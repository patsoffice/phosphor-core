//! Votrax SC-01 speech synthesizer.
//!
//! 64-phoneme formant synthesis chip used in Gottlieb arcade games (Q*Bert,
//! Reactor, etc.). Parameters for each phoneme are stored in a 512-byte
//! internal ROM. The synthesis pipeline models the analog circuit using
//! a glottal pulse generator, noise source, and 4 cascaded IIR formant
//! filters with switched-capacitor coefficients.
//!
//! # Interface
//!
//! - `write_phoneme(data)`: 6-bit phoneme code (bits 0-5)
//! - `set_inflection(data)`: 2-bit inflection (bits 0-1), modulates pitch
//! - `ar_output()`: A/R (Articulate/Request) pin — `true` = ready
//!
//! # Clock
//!
//! Typical main clock: 720 kHz (Gottlieb boards).
//! - sclock = main / 18 ≈ 40 kHz (audio sample rate)
//! - cclock = main / 36 ≈ 20 kHz (switched-capacitor filter clock)
//!
//! # References
//!
//! - MAME `src/devices/sound/votrax.cpp` (Olivier Galibert, BSD-3-Clause)
//! - US Patent 4,433,210 (switched capacitor filter technology)

use crate::audio::AudioResampler;
use crate::core::debug::{DebugRegister, Debuggable};
use phosphor_macros::Saveable;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Glottal pulse waveform from the SC-01's transistor resistor ladder.
///
/// Index 0 = middle value, index 1 = 0V, indices 2-8 = descending ladder
/// from peak. Indices 9+ map back to the middle value (0.0).
#[allow(dead_code)]
const GLOTTAL_WAVE: [f64; 9] = [
    0.0,
    -4.0 / 7.0,
    1.0,
    6.0 / 7.0,
    5.0 / 7.0,
    4.0 / 7.0,
    3.0 / 7.0,
    2.0 / 7.0,
    1.0 / 7.0,
];

/// Phoneme names for debug display.
#[allow(dead_code)]
const PHONE_NAMES: [&str; 64] = [
    "EH3", "EH2", "EH1", "PA0", "DT", "A1", "A2", "ZH", "AH2", "I3", "I2", "I1", "M", "N", "B",
    "V", "CH", "SH", "Z", "AW1", "NG", "AH1", "OO1", "OO", "L", "K", "J", "H", "G", "F", "D", "S",
    "A", "AY", "Y1", "UH3", "AH", "P", "O", "I", "U", "Y", "T", "R", "E", "W", "AE", "AE1", "AW2",
    "UH2", "UH1", "UH", "O2", "O1", "IU", "U1", "THV", "TH", "ER", "EH", "E1", "AW", "PA1", "STOP",
];

const OUTPUT_SAMPLE_RATE: u64 = 44_100;

// ---------------------------------------------------------------------------
// VotraxSc01
// ---------------------------------------------------------------------------

/// Votrax SC-01 speech synthesizer.
#[derive(Saveable)]
#[save_version(1)]
pub struct VotraxSc01 {
    // --- Inputs ---
    phone: u8,
    inflection: u8,

    // --- Outputs ---
    ar_state: bool,

    // --- ROM-extracted parameters for current phoneme ---
    rom_duration: u8,
    rom_vd: u8,
    rom_cld: u8,
    rom_fa: u8,
    rom_fc: u8,
    rom_va: u8,
    rom_f1: u8,
    rom_f2: u8,
    rom_f2q: u8,
    rom_f3: u8,
    rom_closure: bool,
    rom_pause: bool,

    // --- Interpolation registers (8-bit, exponential approach) ---
    cur_fa: u8,
    cur_fc: u8,
    cur_va: u8,
    cur_f1: u8,
    cur_f2: u8,
    cur_f2q: u8,
    cur_f3: u8,

    // --- Committed filter parameter values ---
    filt_fa: u8,
    filt_fc: u8,
    filt_va: u8,
    filt_f1: u8,
    filt_f2: u8, // 5 bits (cur_f2 >> 3)
    filt_f2q: u8,
    filt_f3: u8,

    // --- Timing counters ---
    phonetick: u16,
    ticks: u8,
    update_counter: u8,
    pitch: u8,
    closure: u8,
    sample_count: u32,
    main_divider: u8,
    cur_closure: bool,

    // --- Noise generator ---
    noise: u16,
    cur_noise: bool,

    // --- Commit/end-of-phone scheduling ---
    commit_pending: bool,
    commit_countdown: u32,
    end_phone_pending: bool,
    end_phone_countdown: u32,

    // --- Filter coefficients (IIR biquads) ---
    f1_a: [f64; 4],
    f1_b: [f64; 4],
    f2v_a: [f64; 4],
    f2v_b: [f64; 4],
    f2n_a: [f64; 2],
    f2n_b: [f64; 2],
    f3_a: [f64; 4],
    f3_b: [f64; 4],
    f4_a: [f64; 4],
    f4_b: [f64; 4],
    fx_a: [f64; 1],
    fx_b: [f64; 2],
    fn_a: [f64; 3],
    fn_b: [f64; 3],

    // --- Signal path histories ---
    voice_1: [f64; 4],
    voice_2: [f64; 4],
    voice_3: [f64; 4],
    noise_1: [f64; 3],
    noise_2: [f64; 3],
    noise_3: [f64; 2],
    noise_4: [f64; 2],
    vn_1: [f64; 4],
    vn_2: [f64; 4],
    vn_3: [f64; 4],
    vn_4: [f64; 4],
    vn_5: [f64; 2],
    vn_6: [f64; 2],

    // --- Non-serialized fields ---
    #[save_skip]
    rom: Vec<u8>,
    #[save_skip]
    #[allow(dead_code)] // Used in Phase 3 filter coefficient calculation
    main_clock_hz: u64,
    #[save_skip]
    #[allow(dead_code)] // Used in Phase 3 filter coefficient calculation
    sclock: f64,
    #[save_skip]
    #[allow(dead_code)] // Used in Phase 3 filter coefficient calculation
    cclock: f64,
    #[save_skip]
    resampler: AudioResampler<f32>,
}

impl VotraxSc01 {
    /// Create a new SC-01 with the given main clock frequency.
    ///
    /// Typical clocks: 720,000 Hz (Gottlieb), 722,534 Hz (others).
    pub fn new(main_clock_hz: u64) -> Self {
        let sclock = main_clock_hz as f64 / 18.0;
        Self {
            phone: 0x3F, // STOP
            inflection: 0,
            ar_state: true,
            rom_duration: 0,
            rom_vd: 0,
            rom_cld: 0,
            rom_fa: 0,
            rom_fc: 0,
            rom_va: 0,
            rom_f1: 0,
            rom_f2: 0,
            rom_f2q: 0,
            rom_f3: 0,
            rom_closure: false,
            rom_pause: false,
            cur_fa: 0,
            cur_fc: 0,
            cur_va: 0,
            cur_f1: 0,
            cur_f2: 0,
            cur_f2q: 0,
            cur_f3: 0,
            filt_fa: 0,
            filt_fc: 0,
            filt_va: 0,
            filt_f1: 0,
            filt_f2: 0,
            filt_f2q: 0,
            filt_f3: 0,
            phonetick: 0,
            ticks: 0,
            update_counter: 0,
            pitch: 0,
            closure: 0,
            sample_count: 0,
            main_divider: 0,
            cur_closure: true,
            noise: 0,
            cur_noise: false,
            commit_pending: false,
            commit_countdown: 0,
            end_phone_pending: false,
            end_phone_countdown: 0,
            f1_a: [0.0; 4],
            f1_b: [0.0; 4],
            f2v_a: [0.0; 4],
            f2v_b: [0.0; 4],
            f2n_a: [0.0; 2],
            f2n_b: [0.0; 2],
            f3_a: [0.0; 4],
            f3_b: [0.0; 4],
            f4_a: [0.0; 4],
            f4_b: [0.0; 4],
            fx_a: [0.0; 1],
            fx_b: [0.0; 2],
            fn_a: [0.0; 3],
            fn_b: [0.0; 3],
            voice_1: [0.0; 4],
            voice_2: [0.0; 4],
            voice_3: [0.0; 4],
            noise_1: [0.0; 3],
            noise_2: [0.0; 3],
            noise_3: [0.0; 2],
            noise_4: [0.0; 2],
            vn_1: [0.0; 4],
            vn_2: [0.0; 4],
            vn_3: [0.0; 4],
            vn_4: [0.0; 4],
            vn_5: [0.0; 2],
            vn_6: [0.0; 2],
            rom: vec![0u8; 512],
            main_clock_hz,
            sclock,
            cclock: main_clock_hz as f64 / 36.0,
            resampler: AudioResampler::new(sclock as u64, OUTPUT_SAMPLE_RATE),
        }
    }

    /// Load the phoneme ROM (512 bytes, 64 entries × 8 bytes, little-endian).
    pub fn load_rom(&mut self, data: &[u8]) {
        let len = data.len().min(512);
        self.rom[..len].copy_from_slice(&data[..len]);
    }

    /// Write a 6-bit phoneme code. Sets A/R low and schedules a commit.
    pub fn write_phoneme(&mut self, data: u8) {
        self.phone = data & 0x3F;
        self.ar_state = false;

        // Schedule commit at 72 main clock ticks (~0.1 ms).
        // Overrides a pending end-of-phone but not an existing commit.
        if !self.commit_pending {
            self.commit_pending = true;
            self.commit_countdown = 72;
            self.end_phone_pending = false;
        }
    }

    /// Set the 2-bit inflection value (modulates pitch).
    pub fn set_inflection(&mut self, data: u8) {
        self.inflection = data & 0x03;
    }

    /// Read the A/R (Articulate/Request) pin.
    ///
    /// `true` = ready for next phoneme, `false` = busy playing.
    pub fn ar_output(&self) -> bool {
        self.ar_state
    }

    /// Take all buffered audio samples (f32, centered around 0).
    pub fn drain_audio(&mut self) -> Vec<f32> {
        self.resampler.drain_audio()
    }

    // -----------------------------------------------------------------------
    // Timing engine
    // -----------------------------------------------------------------------

    /// One step of exponential interpolation (matches MAME `interpolate`).
    ///
    /// `reg = reg - (reg >> 3) + (target << 1)`
    ///
    /// Converges to `target * 16` in approximately 8 steps. The 4-bit ROM
    /// targets are scaled to the 8-bit interpolation register range; the
    /// committed filter values (`filt_*`) are derived by right-shifting
    /// the interpolated result back to 4 or 5 bits.
    fn interpolate(reg: &mut u8, target: u8) {
        *reg = *reg - (*reg >> 3) + (target << 1);
    }

    /// Commit interpolated values to filter parameters.
    ///
    /// Always updates FA, FC, VA (amplitude params). Only updates formant
    /// filter coefficients (F1, F2, F2Q, F3) when they actually change,
    /// unless `force` is true (e.g., on reset).
    ///
    /// Filter coefficient recalculation will be added in Phase 3.
    fn filters_commit(&mut self, force: bool) {
        self.filt_fa = self.cur_fa >> 4;
        self.filt_fc = self.cur_fc >> 4;
        self.filt_va = self.cur_va >> 4;

        let new_f1 = self.cur_f1 >> 4;
        if force || self.filt_f1 != new_f1 {
            self.filt_f1 = new_f1;
            // Phase 3: build_standard_filter for F1
        }

        let new_f2 = self.cur_f2 >> 3; // 5 bits — extra precision
        let new_f2q = self.cur_f2q >> 4;
        if force || self.filt_f2 != new_f2 || self.filt_f2q != new_f2q {
            self.filt_f2 = new_f2;
            self.filt_f2q = new_f2q;
            // Phase 3: build_standard_filter for F2 voice
            // Phase 3: build_injection_filter for F2 noise
        }

        let new_f3 = self.cur_f3 >> 4;
        if force || self.filt_f3 != new_f3 {
            self.filt_f3 = new_f3;
            // Phase 3: build_standard_filter for F3
        }

        if force {
            // Phase 3: build fixed filters (F4, FX, FN)
        }
    }

    /// Main timing engine, called at cclock rate (~20 kHz).
    ///
    /// Manages phonetick/ticks counters, interpolation at 208/625 Hz,
    /// pitch counter, noise LFSR, and closure state.
    fn chip_update(&mut self) {
        // Phone tick counter. Stopped when ticks reach 16.
        if self.ticks != 0x10 {
            self.phonetick += 1;
            // Comparator with duration << 2, one-tick delay in path
            if self.phonetick == ((self.rom_duration as u16) << 2) | 1 {
                self.phonetick = 0;
                self.ticks += 1;
                if self.ticks == self.rom_cld {
                    self.cur_closure = self.rom_closure;
                }
            }
        }

        // Update timing counters: divide by 16 (625 Hz) and by 48 (208 Hz),
        // phased so the 208 Hz tick falls exactly between two 625 Hz ticks.
        self.update_counter += 1;
        if self.update_counter == 0x30 {
            self.update_counter = 0;
        }

        let tick_625 = (self.update_counter & 0x0F) == 0;
        let tick_208 = self.update_counter == 0x28;

        // Formant update at 208 Hz.
        // Die bug: FC is interpolated here instead of VA.
        // Formants frozen during pause unless both voice and noise volumes are zero.
        if tick_208 && (!self.rom_pause || (self.filt_fa == 0 && self.filt_va == 0)) {
            Self::interpolate(&mut self.cur_fc, self.rom_fc);
            Self::interpolate(&mut self.cur_f1, self.rom_f1);
            Self::interpolate(&mut self.cur_f2, self.rom_f2);
            Self::interpolate(&mut self.cur_f2q, self.rom_f2q);
            Self::interpolate(&mut self.cur_f3, self.rom_f3);
        }

        // Non-formant (amplitude) update at 625 Hz.
        // Die bug: VA is interpolated here instead of FC.
        if tick_625 {
            if self.ticks >= self.rom_vd {
                Self::interpolate(&mut self.cur_fa, self.rom_fa);
            }
            if self.ticks >= self.rom_cld {
                Self::interpolate(&mut self.cur_va, self.rom_va);
            }
        }

        // Closure counter: ramps 0→28 when closure active, reset otherwise.
        if !self.cur_closure && (self.filt_fa != 0 || self.filt_va != 0) {
            self.closure = 0;
        } else if self.closure != 7 << 2 {
            self.closure += 1;
        }

        // Pitch counter: 8-bit, wraps at threshold derived from inflection and F1.
        self.pitch = self.pitch.wrapping_add(1);
        let pitch_limit = (0xE0u8 ^ (self.inflection << 5) ^ (self.filt_f1 << 1)).wrapping_add(2);
        if self.pitch == pitch_limit {
            self.pitch = 0;
        }

        // Filters commit when pitch is in index 1 of pitch wave
        // (matches 4 consecutive values where bits [2:1] vary).
        if (self.pitch & 0xF9) == 0x08 {
            self.filters_commit(false);
        }

        // Noise shift register: 15-bit LFSR with NXOR feedback on bits 13-14.
        // The `1 || filt_fa` in MAME is always true (likely intended as `0 ||`).
        let inp = self.cur_noise && (self.noise != 0x7FFF);
        self.noise = ((self.noise << 1) & 0x7FFE) | u16::from(inp);
        self.cur_noise = ((self.noise >> 14) ^ (self.noise >> 13)) & 1 == 0;
    }

    // -----------------------------------------------------------------------
    // ROM parsing
    // -----------------------------------------------------------------------

    /// Extract bits from a 64-bit value at the given positions (MSB first).
    ///
    /// Mirrors MAME's `bitswap()` template: the first position becomes the
    /// MSB of the result, the last becomes the LSB.
    fn extract_bits(val: u64, positions: &[u8]) -> u8 {
        let n = positions.len();
        let mut result = 0u8;
        for (i, &pos) in positions.iter().enumerate() {
            result |= (((val >> pos) & 1) as u8) << (n - 1 - i);
        }
        result
    }

    /// Commit the current phoneme: search the ROM and extract parameters.
    fn phone_commit(&mut self) {
        self.phonetick = 0;
        self.ticks = 0;

        for i in 0..64 {
            let offset = i * 8;
            if offset + 8 > self.rom.len() {
                break;
            }

            let val = u64::from_le_bytes(self.rom[offset..offset + 8].try_into().unwrap());

            if self.phone != ((val >> 56) & 0x3F) as u8 {
                continue;
            }

            // Interleaved 4-bit parameters (MSB-first bit positions)
            self.rom_f1 = Self::extract_bits(val, &[0, 7, 14, 21]);
            self.rom_va = Self::extract_bits(val, &[1, 8, 15, 22]);
            self.rom_f2 = Self::extract_bits(val, &[2, 9, 16, 23]);
            self.rom_fc = Self::extract_bits(val, &[3, 10, 17, 24]);
            self.rom_f2q = Self::extract_bits(val, &[4, 11, 18, 25]);
            self.rom_f3 = Self::extract_bits(val, &[5, 12, 19, 26]);
            self.rom_fa = Self::extract_bits(val, &[6, 13, 20, 27]);

            // CLD and VD have inverted bit orders (prototype miswiring
            // compensated in ROM)
            self.rom_cld = Self::extract_bits(val, &[34, 32, 30, 28]);
            self.rom_vd = Self::extract_bits(val, &[35, 33, 31, 29]);

            self.rom_closure = ((val >> 36) & 1) != 0;
            self.rom_duration = Self::extract_bits(!val, &[37, 38, 39, 40, 41, 42, 43]);

            // Hard-wired pause detection (not part of ROM data)
            self.rom_pause = self.phone == 0x03 || self.phone == 0x3E;

            if self.rom_cld == 0 {
                self.cur_closure = self.rom_closure;
            }

            // Schedule end-of-phone: A/R returns high after full duration
            let duration_ticks = 16 * (self.rom_duration as u32 * 4 + 1) * 4 * 9 + 2;
            self.end_phone_pending = true;
            self.end_phone_countdown = duration_ticks;

            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Device trait
// ---------------------------------------------------------------------------

impl crate::device::Device for VotraxSc01 {
    fn name(&self) -> &'static str {
        "Votrax SC-01"
    }

    fn reset(&mut self) {
        self.phone = 0x3F;
        self.inflection = 0;
        self.ar_state = true;
        self.sample_count = 0;

        // Commit STOP phoneme to initialize ROM parameters
        self.phone_commit();

        // Clear interpolation state
        self.cur_fa = 0;
        self.cur_fc = 0;
        self.cur_va = 0;
        self.cur_f1 = 0;
        self.cur_f2 = 0;
        self.cur_f2q = 0;
        self.cur_f3 = 0;

        // Clear committed filter values
        self.filt_fa = 0;
        self.filt_fc = 0;
        self.filt_va = 0;
        self.filt_f1 = 0;
        self.filt_f2 = 0;
        self.filt_f2q = 0;
        self.filt_f3 = 0;

        // Clear timing state
        self.pitch = 0;
        self.closure = 0;
        self.update_counter = 0;
        self.cur_closure = true;
        self.noise = 0;
        self.cur_noise = false;
        self.main_divider = 0;
        self.commit_pending = false;
        self.commit_countdown = 0;
        self.end_phone_pending = false;
        self.end_phone_countdown = 0;

        // Clear signal histories
        self.voice_1 = [0.0; 4];
        self.voice_2 = [0.0; 4];
        self.voice_3 = [0.0; 4];
        self.noise_1 = [0.0; 3];
        self.noise_2 = [0.0; 3];
        self.noise_3 = [0.0; 2];
        self.noise_4 = [0.0; 2];
        self.vn_1 = [0.0; 4];
        self.vn_2 = [0.0; 4];
        self.vn_3 = [0.0; 4];
        self.vn_4 = [0.0; 4];
        self.vn_5 = [0.0; 2];
        self.vn_6 = [0.0; 2];

        // Rebuild all filter coefficients from zero state
        self.filters_commit(true);

        self.resampler.reset();
    }

    fn tick(&mut self) {
        // Handle pending phoneme commit (72 main clock ticks after write)
        if self.commit_pending {
            if self.commit_countdown == 0 {
                self.commit_pending = false;
                self.phone_commit();
            } else {
                self.commit_countdown -= 1;
            }
        }

        // Handle end-of-phone (A/R goes high when phoneme duration expires)
        if self.end_phone_pending {
            if self.end_phone_countdown == 0 {
                self.end_phone_pending = false;
                self.ar_state = true;
            } else {
                self.end_phone_countdown -= 1;
            }
        }

        // Divide main clock by 18 → sclock (~40 kHz audio sample rate)
        self.main_divider += 1;
        if self.main_divider >= 18 {
            self.main_divider = 0;

            self.sample_count += 1;

            // Every other sclock tick = cclock (~20 kHz): run timing engine
            if self.sample_count & 1 != 0 {
                self.chip_update();
            }

            // Phase 4: analog_calc() and resampler.tick() here
        }
    }
}

// ---------------------------------------------------------------------------
// Debuggable
// ---------------------------------------------------------------------------

impl Debuggable for VotraxSc01 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "PHONE",
                value: self.phone as u64,
                width: 6,
            },
            DebugRegister {
                name: "INFLECT",
                value: self.inflection as u64,
                width: 2,
            },
            DebugRegister {
                name: "AR",
                value: u64::from(self.ar_state),
                width: 1,
            },
            DebugRegister {
                name: "TICKS",
                value: self.ticks as u64,
                width: 5,
            },
            DebugRegister {
                name: "PITCH",
                value: self.pitch as u64,
                width: 8,
            },
            DebugRegister {
                name: "DURATION",
                value: self.rom_duration as u64,
                width: 7,
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Device;

    const TEST_CLOCK: u64 = 720_000;

    /// Build one 8-byte ROM entry encoding the given phoneme parameters.
    ///
    /// The entry is stored in the interleaved bit format expected by
    /// `phone_commit()`, matching the SC-01 ROM layout.
    #[allow(clippy::too_many_arguments)]
    fn build_rom_entry(
        phone: u8,
        f1: u8,
        va: u8,
        f2: u8,
        fc: u8,
        f2q: u8,
        f3: u8,
        fa: u8,
        cld: u8,
        vd: u8,
        closure: bool,
        duration: u8,
    ) -> [u8; 8] {
        let mut val: u64 = 0;

        // Phone code in bits 56-61
        val |= (phone as u64 & 0x3F) << 56;

        // Helper to set a single bit
        let set = |v: &mut u64, pos: u8, bit: u8| {
            if bit & 1 != 0 {
                *v |= 1u64 << pos;
            }
        };

        // Interleaved parameters: position arrays match extract_bits calls.
        // Each parameter is 4 bits (MSB to LSB stored at the given positions).
        let params: [(u8, &[u8]); 7] = [
            (f1, &[0, 7, 14, 21]),
            (va, &[1, 8, 15, 22]),
            (f2, &[2, 9, 16, 23]),
            (fc, &[3, 10, 17, 24]),
            (f2q, &[4, 11, 18, 25]),
            (f3, &[5, 12, 19, 26]),
            (fa, &[6, 13, 20, 27]),
        ];
        for (param, positions) in &params {
            let n = positions.len();
            for (i, &pos) in positions.iter().enumerate() {
                set(&mut val, pos, param >> (n - 1 - i));
            }
        }

        // CLD: bits 34, 32, 30, 28
        for (i, &pos) in [34u8, 32, 30, 28].iter().enumerate() {
            set(&mut val, pos, cld >> (3 - i));
        }
        // VD: bits 35, 33, 31, 29
        for (i, &pos) in [35u8, 33, 31, 29].iter().enumerate() {
            set(&mut val, pos, vd >> (3 - i));
        }

        // Closure: bit 36
        if closure {
            val |= 1u64 << 36;
        }

        // Duration: extracted via extract_bits(!val, [37..43]), so store
        // inverted bits in val.
        for (i, &pos) in [37u8, 38, 39, 40, 41, 42, 43].iter().enumerate() {
            let bit = (duration >> (6 - i)) & 1;
            // Invert: extract_bits uses !val
            if bit == 0 {
                val |= 1u64 << pos;
            }
        }

        val.to_le_bytes()
    }

    /// Build a 512-byte ROM containing one phoneme entry at index 0
    /// (remaining entries zeroed).
    fn build_test_rom(entry: &[u8; 8]) -> Vec<u8> {
        let mut rom = vec![0u8; 512];
        rom[..8].copy_from_slice(entry);
        rom
    }

    #[test]
    fn initial_state() {
        let v = VotraxSc01::new(TEST_CLOCK);
        assert_eq!(v.phone, 0x3F);
        assert_eq!(v.inflection, 0);
        assert!(v.ar_output());
        assert_eq!(v.sclock, TEST_CLOCK as f64 / 18.0);
        assert_eq!(v.cclock, TEST_CLOCK as f64 / 36.0);
    }

    #[test]
    fn write_phoneme_sets_state() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.write_phoneme(0x05); // A1
        assert_eq!(v.phone, 0x05);
        assert!(!v.ar_output()); // busy
        assert!(v.commit_pending);
        assert_eq!(v.commit_countdown, 72);
    }

    #[test]
    fn write_phoneme_masks_to_6_bits() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.write_phoneme(0xFF);
        assert_eq!(v.phone, 0x3F);
    }

    #[test]
    fn set_inflection_masks_to_2_bits() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.set_inflection(0xFF);
        assert_eq!(v.inflection, 0x03);
    }

    #[test]
    fn extract_bits_basic() {
        // bit0=1, bit7=0, bit14=1, bit21=0
        let val: u64 = (1 << 0) | (1 << 14);
        // Positions [0, 7, 14, 21] → MSB-first → result = 1010 = 0xA
        assert_eq!(VotraxSc01::extract_bits(val, &[0, 7, 14, 21]), 0b1010);

        // All bits set
        let val2: u64 = (1 << 0) | (1 << 7) | (1 << 14) | (1 << 21);
        assert_eq!(VotraxSc01::extract_bits(val2, &[0, 7, 14, 21]), 0b1111);
    }

    #[test]
    fn extract_bits_single() {
        let val: u64 = 1 << 36;
        assert_eq!(VotraxSc01::extract_bits(val, &[36]), 1);
        assert_eq!(VotraxSc01::extract_bits(val, &[35]), 0);
    }

    #[test]
    fn rom_round_trip() {
        // Build a ROM entry with known parameters and verify extraction
        let entry = build_rom_entry(
            0x05, // phone = A1
            0x0A, // f1
            0x0C, // va
            0x07, // f2
            0x03, // fc
            0x0F, // f2q
            0x06, // f3
            0x09, // fa
            0x02, // cld
            0x05, // vd
            true, // closure
            0x1A, // duration
        );
        let rom = build_test_rom(&entry);

        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);
        v.phone = 0x05;
        v.phone_commit();

        assert_eq!(v.rom_f1, 0x0A);
        assert_eq!(v.rom_va, 0x0C);
        assert_eq!(v.rom_f2, 0x07);
        assert_eq!(v.rom_fc, 0x03);
        assert_eq!(v.rom_f2q, 0x0F);
        assert_eq!(v.rom_f3, 0x06);
        assert_eq!(v.rom_fa, 0x09);
        assert_eq!(v.rom_cld, 0x02);
        assert_eq!(v.rom_vd, 0x05);
        assert!(v.rom_closure);
        assert_eq!(v.rom_duration, 0x1A);
        assert!(!v.rom_pause); // A1 is not a pause phone
    }

    #[test]
    fn rom_pause_detection() {
        // Phone 0x03 = PA0 (pause)
        let entry = build_rom_entry(0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, false, 0);
        let rom = build_test_rom(&entry);
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);
        v.phone = 0x03;
        v.phone_commit();
        assert!(v.rom_pause);

        // Phone 0x3E = PA1 (pause)
        let entry = build_rom_entry(0x3E, 0, 0, 0, 0, 0, 0, 0, 0, 0, false, 0);
        let rom = build_test_rom(&entry);
        v.load_rom(&rom);
        v.phone = 0x3E;
        v.phone_commit();
        assert!(v.rom_pause);
    }

    #[test]
    fn rom_cld_zero_sets_closure() {
        // When rom_cld == 0, cur_closure is set immediately from rom_closure
        let entry = build_rom_entry(0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, true, 0x20);
        let rom = build_test_rom(&entry);
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);
        v.cur_closure = false;
        v.phone = 0x10;
        v.phone_commit();
        assert!(v.cur_closure); // Set immediately because cld == 0
    }

    #[test]
    fn commit_fires_after_72_ticks() {
        let entry = build_rom_entry(0x05, 0x0A, 0, 0, 0, 0, 0, 0, 0, 0, false, 0x10);
        let rom = build_test_rom(&entry);
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);
        v.write_phoneme(0x05);

        // Tick 72 times — commit should fire on the 73rd tick
        for _ in 0..72 {
            assert!(v.commit_pending);
            v.tick();
        }

        // After 72 ticks the countdown reaches 0, commit fires on next tick
        v.tick();
        assert!(!v.commit_pending);
        assert_eq!(v.rom_f1, 0x0A); // Parameters were extracted
    }

    #[test]
    fn end_of_phone_restores_ar() {
        let duration = 0x01u8;
        let entry = build_rom_entry(0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, false, duration);
        let rom = build_test_rom(&entry);
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);
        v.write_phoneme(0x05);

        // Tick past commit (72 + 1 ticks)
        for _ in 0..73 {
            v.tick();
        }
        assert!(!v.ar_output()); // Still busy

        // End-of-phone countdown: 16 * (duration * 4 + 1) * 4 * 9 + 2
        let end_ticks = 16 * (duration as u32 * 4 + 1) * 4 * 9 + 2;
        for _ in 0..end_ticks {
            v.tick();
        }
        // After the countdown the next tick fires end-of-phone
        v.tick();
        assert!(v.ar_output()); // Ready again
    }

    #[test]
    fn write_during_end_of_phone_cancels_it() {
        let entry = build_rom_entry(0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, false, 0x7F);
        let rom = build_test_rom(&entry);
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.load_rom(&rom);

        // Start first phoneme and tick past commit
        v.write_phoneme(0x05);
        for _ in 0..73 {
            v.tick();
        }
        assert!(v.end_phone_pending);

        // Write a new phoneme — cancels end-of-phone
        v.write_phoneme(0x05);
        assert!(!v.end_phone_pending);
        assert!(v.commit_pending);
    }

    #[test]
    fn reset_clears_state() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.phone = 0x10;
        v.inflection = 3;
        v.ar_state = false;
        v.pitch = 0x42;
        v.noise = 0x1234;

        v.reset();

        assert_eq!(v.phone, 0x3F);
        assert_eq!(v.inflection, 0);
        assert!(v.ar_state);
        assert_eq!(v.pitch, 0);
        assert_eq!(v.noise, 0);
        assert!(v.cur_closure);
    }

    #[test]
    fn debug_registers_populated() {
        let v = VotraxSc01::new(TEST_CLOCK);
        let regs = v.debug_registers();
        assert!(!regs.is_empty());
        assert_eq!(regs[0].name, "PHONE");
        assert_eq!(regs[0].value, 0x3F);
    }

    #[test]
    fn save_load_round_trip() {
        use crate::core::save_state::{StateReader, StateWriter};
        use crate::prelude::Saveable;

        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.phone = 0x15;
        v.inflection = 2;
        v.ar_state = false;
        v.pitch = 0x42;
        v.noise = 0x5678;
        v.f1_a[0] = 1.234;

        let mut w = StateWriter::new();
        v.save_state(&mut w);
        let data = w.into_vec();

        let mut v2 = VotraxSc01::new(TEST_CLOCK);
        let mut r = StateReader::new(&data);
        v2.load_state(&mut r).unwrap();

        assert_eq!(v2.phone, 0x15);
        assert_eq!(v2.inflection, 2);
        assert!(!v2.ar_state);
        assert_eq!(v2.pitch, 0x42);
        assert_eq!(v2.noise, 0x5678);
        assert!((v2.f1_a[0] - 1.234).abs() < f64::EPSILON);
    }

    // -- Phase 2: Timing engine tests --

    #[test]
    fn interpolate_converges_to_target() {
        // Target 15 → steady state = 15 * 16 = 240 = 0xF0
        let mut reg = 0u8;
        for _ in 0..40 {
            VotraxSc01::interpolate(&mut reg, 15);
        }
        // Should converge to 240 (within 1 due to truncation)
        assert!((239..=241).contains(&reg), "expected ~240, got {reg}");
        // After >> 4, should equal the 4-bit target
        assert_eq!(reg >> 4, 15);
    }

    #[test]
    fn interpolate_converges_downward() {
        // Start high, target 0 → should decay toward 0
        let mut reg = 240u8;
        for _ in 0..80 {
            VotraxSc01::interpolate(&mut reg, 0);
        }
        // Should reach a small value (truncation prevents exact 0)
        assert!(reg < 8, "expected near 0, got {reg}");
    }

    #[test]
    fn interpolate_midrange() {
        // Target 8 → steady state ~128
        let mut reg = 0u8;
        for _ in 0..40 {
            VotraxSc01::interpolate(&mut reg, 8);
        }
        assert!((127..=129).contains(&reg), "expected ~128, got {reg}");
        assert_eq!(reg >> 4, 8);
    }

    #[test]
    fn chip_update_phonetick_wraps_at_duration() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_duration = 3; // wrap at (3 << 2) | 1 = 13
        v.ticks = 0;

        // Run 13 chip_update calls — phonetick should wrap once
        for _ in 0..13 {
            v.chip_update();
        }
        assert_eq!(v.phonetick, 0);
        assert_eq!(v.ticks, 1);
    }

    #[test]
    fn chip_update_ticks_stop_at_16() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_duration = 0; // wrap at (0 << 2) | 1 = 1 (every other tick)
        v.ticks = 15;

        // One more wrap should bring ticks to 16
        v.chip_update(); // phonetick = 1 == 1, wraps, ticks = 16
        assert_eq!(v.ticks, 16);

        // Now ticks should stay at 16 — phonetick counter is frozen
        let prev_phonetick = v.phonetick;
        v.chip_update();
        assert_eq!(v.ticks, 16);
        // phonetick doesn't advance when ticks == 0x10
        assert_eq!(v.phonetick, prev_phonetick);
    }

    #[test]
    fn chip_update_closure_at_cld() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_duration = 0; // wrap every other tick
        v.rom_cld = 3;
        v.rom_closure = true;
        v.cur_closure = false;

        // Run until ticks reaches 3 (= rom_cld)
        // Each wrap takes 1 chip_update (phonetick goes 0→1, wraps)
        for _ in 0..3 {
            v.chip_update(); // ticks: 0→1→2→3
        }
        assert_eq!(v.ticks, 3);
        assert!(v.cur_closure);
    }

    #[test]
    fn update_counter_mod_48() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.update_counter = 0x2F; // 47

        v.chip_update();
        assert_eq!(v.update_counter, 0); // wraps at 0x30 (48)
    }

    #[test]
    fn tick_625_fires_every_16_updates() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_vd = 0; // always interpolate FA
        v.rom_cld = 0; // always interpolate VA
        v.rom_fa = 15;
        v.rom_va = 15;
        v.ticks = 1; // past both delays

        // Run 48 chip_updates (one full update_counter cycle)
        let mut tick_625_count = 0;
        for i in 0..48 {
            let before_fa = v.cur_fa;
            v.chip_update();
            if v.cur_fa != before_fa {
                tick_625_count += 1;
            }
            // 625 Hz fires when update_counter (after increment) & 0x0F == 0
            // That's at counter values: 0x10, 0x20, 0x30→0 = 0, so ticks at 16, 32, 0
            let _ = i;
        }
        // Should fire 3 times per 48-cycle period (at 0, 16, 32)
        assert_eq!(tick_625_count, 3, "625 Hz should fire 3× per 48 cycles");
    }

    #[test]
    fn tick_208_fires_at_counter_40() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_f1 = 10;
        v.update_counter = 0x27; // next increment → 0x28 → 208 Hz fires

        let before = v.cur_f1;
        v.chip_update();
        assert_ne!(v.cur_f1, before, "208 Hz should have interpolated F1");
    }

    #[test]
    fn pitch_wraps_at_threshold() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.inflection = 0;
        v.filt_f1 = 0;
        // Threshold = (0xE0 ^ 0 ^ 0) + 2 = 0xE2 = 226
        v.pitch = 225; // next increment = 226

        v.chip_update();
        assert_eq!(v.pitch, 0, "pitch should wrap at 226");
    }

    #[test]
    fn pitch_wrap_with_inflection() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.inflection = 2; // inflection << 5 = 0x40
        v.filt_f1 = 5; // filt_f1 << 1 = 0x0A
        // Threshold = (0xE0 ^ 0x40 ^ 0x0A) + 2 = 0xAA + 2 = 0xAC = 172
        v.pitch = 171;

        v.chip_update();
        assert_eq!(v.pitch, 0, "pitch should wrap at 172");
    }

    #[test]
    fn noise_lfsr_advances() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.noise = 0x0001;
        v.cur_noise = true;

        // After one update, noise should shift
        v.chip_update();
        // inp = true && (0x0001 != 0x7FFF) = true
        // noise = ((0x0001 << 1) & 0x7FFE) | 1 = 0x0002 | 1 = 0x0003
        assert_eq!(v.noise, 0x0003);
    }

    #[test]
    fn noise_lfsr_lockup_prevention() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.noise = 0x7FFF; // all 1s
        v.cur_noise = true;

        v.chip_update();
        // inp = true && (0x7FFF != 0x7FFF) = false (lockup prevention)
        // noise = ((0x7FFF << 1) & 0x7FFE) | 0 = 0x7FFE
        assert_eq!(v.noise, 0x7FFE);
    }

    #[test]
    fn noise_doesnt_lock_up_over_long_run() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.noise = 1;
        v.cur_noise = true;

        // Run many updates and verify noise keeps changing
        let mut seen_values = std::collections::HashSet::new();
        for _ in 0..1000 {
            v.chip_update();
            seen_values.insert(v.noise);
        }
        // Should visit many different states
        assert!(
            seen_values.len() > 100,
            "LFSR should visit many states, got {}",
            seen_values.len()
        );
    }

    #[test]
    fn closure_counter_ramps_to_28() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.cur_closure = true;
        v.filt_fa = 1; // nonzero so closure doesn't reset
        v.filt_va = 0;
        v.closure = 0;

        for _ in 0..30 {
            v.chip_update();
        }
        // Closure counter maxes at 7 << 2 = 28
        assert_eq!(v.closure, 28);
    }

    #[test]
    fn closure_counter_resets_when_not_active() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.cur_closure = false; // not closed
        v.filt_fa = 1; // nonzero
        v.closure = 20;

        v.chip_update();
        assert_eq!(
            v.closure, 0,
            "closure resets when cur_closure=false and volume nonzero"
        );
    }

    #[test]
    fn filters_commit_derives_filt_from_cur() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.cur_fa = 0xF0;
        v.cur_fc = 0x80;
        v.cur_va = 0x50;
        v.cur_f1 = 0xA0;
        v.cur_f2 = 0xC8; // >> 3 = 25
        v.cur_f2q = 0x70;
        v.cur_f3 = 0x30;

        v.filters_commit(true);

        assert_eq!(v.filt_fa, 0x0F); // 0xF0 >> 4
        assert_eq!(v.filt_fc, 0x08); // 0x80 >> 4
        assert_eq!(v.filt_va, 0x05); // 0x50 >> 4
        assert_eq!(v.filt_f1, 0x0A); // 0xA0 >> 4
        assert_eq!(v.filt_f2, 0x19); // 0xC8 >> 3 = 25
        assert_eq!(v.filt_f2q, 0x07); // 0x70 >> 4
        assert_eq!(v.filt_f3, 0x03); // 0x30 >> 4
    }

    #[test]
    fn clock_division_fires_chip_update() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_f1 = 10; // nonzero target so interpolation has effect
        v.update_counter = 0x27; // next chip_update → 0x28 → 208 Hz fires

        // Need 18 main ticks for one sclock, and chip_update fires on odd sclock
        // First sclock tick: sample_count goes from 0 to 1 (odd) → chip_update fires
        for _ in 0..18 {
            v.tick();
        }
        assert_eq!(v.sample_count, 1);
        // 208 Hz should have fired and interpolated F1
        assert_ne!(v.cur_f1, 0);
    }

    #[test]
    fn formants_frozen_during_pause() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_pause = true;
        v.rom_f1 = 10;
        v.filt_fa = 1; // nonzero volume → formants stay frozen
        v.filt_va = 0;
        v.update_counter = 0x27; // next → 208 Hz tick

        v.chip_update();
        assert_eq!(
            v.cur_f1, 0,
            "formants should be frozen during pause with nonzero volume"
        );
    }

    #[test]
    fn formants_unfreeze_during_pause_at_zero_volume() {
        let mut v = VotraxSc01::new(TEST_CLOCK);
        v.rom_pause = true;
        v.rom_f1 = 10;
        v.filt_fa = 0; // zero volume → formants can update
        v.filt_va = 0;
        v.update_counter = 0x27; // next → 208 Hz tick

        v.chip_update();
        assert_ne!(
            v.cur_f1, 0,
            "formants should update during pause when volume is zero"
        );
    }
}
