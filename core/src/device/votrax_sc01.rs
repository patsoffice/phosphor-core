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

        // Clear filter coefficients
        self.f1_a = [0.0; 4];
        self.f1_b = [0.0; 4];
        self.f2v_a = [0.0; 4];
        self.f2v_b = [0.0; 4];
        self.f2n_a = [0.0; 2];
        self.f2n_b = [0.0; 2];
        self.f3_a = [0.0; 4];
        self.f3_b = [0.0; 4];
        self.f4_a = [0.0; 4];
        self.f4_b = [0.0; 4];
        self.fx_a = [0.0; 1];
        self.fx_b = [0.0; 2];
        self.fn_a = [0.0; 3];
        self.fn_b = [0.0; 3];

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

        // Phases 2-4 will add: clock division → chip_update → analog_calc
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
}
