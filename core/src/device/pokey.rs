/// Atari POKEY (C012294) — Programmable sound, I/O, and timer chip
///
/// The POKEY provides four independently programmable audio channels,
/// polynomial counter-based noise/tone generation, potentiometer (paddle)
/// input scanning, keyboard scanning, serial I/O, and an interrupt
/// controller. It was used in Atari 400/800 home computers and numerous
/// Atari coin-op arcade boards (Missile Command, Centipede, etc.).
///
/// This implementation covers the audio, timer/IRQ, pot scanning, and
/// random number subsystems. Keyboard and serial I/O are stubbed for
/// arcade use (directly settable via helper methods).
///
/// References:
/// - Atari C012294 datasheet (the definitive POKEY reference)
/// - De Re Atari, Chapter 7: "Sound"
/// - Altirra Hardware Reference Manual, POKEY section
/// - MAME: `mamedev/mame` `src/devices/sound/pokey.cpp` / `.h`
///
/// # Write registers (offsets 0x00-0x0F)
///
/// | Offset | Name   | Description                                      |
/// |--------|--------|--------------------------------------------------|
/// | 0x00   | AUDF1  | Channel 1 frequency divider (period = N+1)       |
/// | 0x01   | AUDC1  | Channel 1 control: volume, distortion, tone gate |
/// | 0x02   | AUDF2  | Channel 2 frequency divider                      |
/// | 0x03   | AUDC2  | Channel 2 control                                |
/// | 0x04   | AUDF3  | Channel 3 frequency divider                      |
/// | 0x05   | AUDC3  | Channel 3 control                                |
/// | 0x06   | AUDF4  | Channel 4 frequency divider                      |
/// | 0x07   | AUDC4  | Channel 4 control                                |
/// | 0x08   | AUDCTL | Master audio control                             |
/// | 0x09   | STIMER | Reset audio timers (write any value)             |
/// | 0x0A   | SKREST | Reset serial port status bits                    |
/// | 0x0B   | POTGO  | Start potentiometer scan                         |
/// | 0x0D   | SEROUT | Serial output data                               |
/// | 0x0E   | IRQEN  | Interrupt enable mask                            |
/// | 0x0F   | SKCTL  | Serial port control                              |
///
/// # Read registers (offsets 0x00-0x0F)
///
/// | Offset | Name   | Description                                      |
/// |--------|--------|--------------------------------------------------|
/// | 0x00-7 | POT0-7 | Potentiometer counter values                     |
/// | 0x08   | ALLPOT | Pot scan completion bitmap (1 = still scanning)  |
/// | 0x09   | KBCODE | Keyboard code                                    |
/// | 0x0A   | RANDOM | Random number (from polynomial counter)          |
/// | 0x0D   | SERIN  | Serial input data                                |
/// | 0x0E   | IRQST  | Interrupt status (active-low: 0 = pending)       |
/// | 0x0F   | SKSTAT | Serial/keyboard status                           |
///
/// # AUDCTL bit assignments
///
/// | Bit | Constant          | Description                                   |
/// |-----|-------------------|-----------------------------------------------|
/// | 7   | `AUDCTL_POLY9`    | 0 = 17-bit polynomial, 1 = 9-bit polynomial   |
/// | 6   | `AUDCTL_CH1_179MHZ` | 0 = base clock, 1 = 1.79 MHz for Ch1        |
/// | 5   | `AUDCTL_CH3_179MHZ` | 0 = base clock, 1 = 1.79 MHz for Ch3        |
/// | 4   | `AUDCTL_CH12_LINKED` | 1 = Ch1+Ch2 form 16-bit counter            |
/// | 3   | `AUDCTL_CH34_LINKED` | 1 = Ch3+Ch4 form 16-bit counter            |
/// | 2   | `AUDCTL_HPF_CH1`  | 1 = High-pass filter Ch1 (clocked by Ch3)     |
/// | 1   | `AUDCTL_HPF_CH2`  | 1 = High-pass filter Ch2 (clocked by Ch4)     |
/// | 0   | `AUDCTL_CLOCK_15KHZ` | 0 = 64 kHz base, 1 = 15 kHz base           |
///
/// # Audio pipeline (per tick at 1.79 MHz master clock)
///
/// 1. **Polynomial counters** step: 4-bit, 5-bit, 9-bit, and 17-bit LFSRs
///    advance one position every tick, producing pseudo-random bit streams.
/// 2. **Base clock dividers** count down: divide-by-28 produces the 64 kHz
///    tick, divide-by-114 produces the 15 kHz tick.
/// 3. **Channel dividers** count down on their selected clock edge. On
///    underflow the divider reloads and the channel's square-wave output
///    (`div_out`) toggles. In 16-bit linked mode, AUDF of the paired
///    channels forms one 16-bit reload value.
/// 4. **High-pass filters** (optional per AUDCTL): the source channel's
///    output is captured into a flip-flop on the modulating channel's
///    underflow edge, then XORed with the source to produce the filtered
///    signal.
/// 5. **Distortion gating**: the channel's square wave is ANDed with
///    selected polynomial counter output. The 3-bit distortion field in
///    AUDC selects which combination of 4-bit, 5-bit, and 17/9-bit
///    polynomials to use.
/// 6. **Volume scaling**: the gated signal (0 or 1) selects between 0 and
///    the 4-bit volume level from AUDC. If AUDC bit 4 is set, the output
///    is forced to the volume level regardless of tone/poly state ("volume
///    only" mode, used for DAC-style sample playback).
/// 7. **Mixing**: all four channels are summed and normalized.
/// 8. **Resampling**: a Bresenham accumulator downsamples the 1.79 MHz
///    mixed output to the host audio sample rate using box-filter averaging.
pub struct Pokey {
    // Audio channel registers (CPU-written)
    audf: [u8; 4], // AUDF1-4: frequency divider reload values
    audc: [u8; 4], // AUDC1-4: volume (bits 3:0), distortion (bits 7:5), tone gate (bit 4)
    audctl: u8,    // Master audio control

    // Audio channel runtime state
    divider: [u16; 4],  // Current divider countdown (u16 for 16-bit linked mode)
    div_out: [bool; 4], // Divider output toggle (flips on underflow -> square wave)
    channel_out: [bool; 4], // Final channel output after distortion gating
    hp_ff: [bool; 2],   // High-pass filter flip-flop [ch1, ch2]

    // Polynomial counters (free-running LFSRs, clocked at 1.79 MHz)
    poly4: u8,   // 4-bit LFSR, period 15
    poly5: u8,   // 5-bit LFSR, period 31
    poly9: u16,  // 9-bit LFSR, period 511
    poly17: u32, // 17-bit LFSR, period 131071

    // Base clock dividers (derived from 1.79 MHz master)
    base_div28: u8,  // Counter for 64 kHz (1.79M / 28)
    base_div114: u8, // Counter for 15 kHz (1.79M / 114)

    // Potentiometer inputs
    pot_input: [u8; 8],   // External pot values (set by board logic)
    pot_counter: [u8; 8], // Scan counter per pot
    pot_done: u8,         // ALLPOT completion bitmap
    pot_scanning: bool,
    pot_scan_count: u8, // Global scan tick counter (stops at POT_SCAN_MAX)

    // Keyboard / serial (stubbed for arcade use)
    kbcode: u8,
    serin: u8,
    serout: u8,
    skctl: u8,
    skstat: u8,

    // Interrupt system
    irqen: u8, // IRQEN: enable mask
    irqst: u8, // IRQST: status (active-low: 0 = pending)

    // Audio output buffer (resampled from 1.79 MHz to host sample rate)
    sample_buffer: Vec<f32>,
    sample_accum: f32, // Running sum for box-filter downsampling
    sample_count: u32, // Ticks accumulated in current sample
    sample_phase: u64, // Bresenham-style fractional accumulator
    output_sample_rate: u32,
    master_clock_hz: u32, // 1_789_773 (NTSC)
}

// AUDCTL bit positions (from Atari C012294 datasheet)
const AUDCTL_POLY9: u8 = 0x80; // Bit 7: 0 = 17-bit poly, 1 = 9-bit poly
const AUDCTL_CH1_179MHZ: u8 = 0x40; // Bit 6: 0 = base clock, 1 = 1.79 MHz for Ch1
const AUDCTL_CH3_179MHZ: u8 = 0x20; // Bit 5: 0 = base clock, 1 = 1.79 MHz for Ch3
const AUDCTL_CH12_LINKED: u8 = 0x10; // Bit 4: 0 = independent, 1 = Ch1+2 16-bit
const AUDCTL_CH34_LINKED: u8 = 0x08; // Bit 3: 0 = independent, 1 = Ch3+4 16-bit
const AUDCTL_HPF_CH1: u8 = 0x04; // Bit 2: High-pass filter Ch1 (clocked by Ch3)
const AUDCTL_HPF_CH2: u8 = 0x02; // Bit 1: High-pass filter Ch2 (clocked by Ch4)
const AUDCTL_CLOCK_15KHZ: u8 = 0x01; // Bit 0: 0 = 64 kHz base, 1 = 15 kHz base

// AUDC (per-channel control) bit positions
const AUDC_DIST_MASK: u8 = 0xE0; // Bits 7:5: distortion (polynomial select)
const AUDC_DIST_SHIFT: u8 = 5; // Right-shift to extract distortion field
const AUDC_VOLUME_ONLY: u8 = 0x10; // Bit 4: 1 = force volume level (DAC mode), 0 = use poly/tone
const AUDC_VOL_MASK: u8 = 0x0F; // Bits 3:0: volume (0-15)

// IRQEN / IRQST bit positions (active-low in IRQST: 0 = pending)
const IRQ_TIMER1: u8 = 0x01; // Bit 0: Ch1 timer underflow
const IRQ_TIMER2: u8 = 0x02; // Bit 1: Ch2 timer underflow
const IRQ_TIMER4: u8 = 0x04; // Bit 2: Ch4 timer underflow

// SKSTAT bits cleared by SKREST (write to 0x0A).
// Only resets serial error flags, not keyboard status bits.
const SKSTAT_FRAME_ERR: u8 = 0x80; // Bit 7: Serial frame error
const SKSTAT_OVERRUN: u8 = 0x40; // Bit 6: Serial data overrun
const SKSTAT_DATA_READY: u8 = 0x08; // Bit 3: Serial data ready
const SKSTAT_RESET_MASK: u8 = SKSTAT_FRAME_ERR | SKSTAT_OVERRUN | SKSTAT_DATA_READY;

/// Maximum pot scan count. Hardware stops scanning after 228 clocks
/// (one NTSC frame's worth of scanlines at the 15 kHz rate).
const POT_SCAN_MAX: u8 = 228;

impl Pokey {
    /// Create a new POKEY with all registers cleared and polynomial counters
    /// seeded to their maximum values. The `output_sample_rate` determines
    /// the Bresenham resampling ratio (e.g. 44100 or 48000 Hz).
    pub fn new(output_sample_rate: u32) -> Self {
        Self {
            audf: [0; 4],
            audc: [0; 4],
            audctl: 0,
            divider: [0; 4],
            div_out: [false; 4],
            channel_out: [false; 4],
            hp_ff: [false; 2],
            poly4: 0x0F,
            poly5: 0x1F,
            poly9: 0x1FF,
            poly17: 0x1FFFF,
            base_div28: 28,
            base_div114: 114,
            pot_input: [0; 8],
            pot_counter: [0; 8],
            pot_done: 0xFF,
            pot_scanning: false,
            pot_scan_count: 0,
            kbcode: 0xFF,
            serin: 0,
            serout: 0,
            skctl: 0,
            skstat: 0xFF,
            irqen: 0,
            irqst: 0xFF,
            sample_buffer: Vec::new(),
            sample_accum: 0.0,
            sample_count: 0,
            sample_phase: 0,
            output_sample_rate,
            master_clock_hz: 1_789_773,
        }
    }

    /// Create a POKEY with a custom master clock rate.
    /// Missile Command uses 1.25 MHz vs the standard 1.79 MHz NTSC clock.
    pub fn with_clock(master_clock_hz: u32, output_sample_rate: u32) -> Self {
        let mut pokey = Self::new(output_sample_rate);
        pokey.master_clock_hz = master_clock_hz;
        pokey
    }

    /// Read from a POKEY register. `offset` is masked to 4 bits (0x00-0x0F).
    ///
    /// | Offset | Register | Returns                                     |
    /// |--------|----------|---------------------------------------------|
    /// | 0x00-7 | POTn     | Potentiometer counter value for pot n       |
    /// | 0x08   | ALLPOT   | Pot scan status bitmap (1=still scanning)   |
    /// | 0x09   | KBCODE   | Last keyboard scan code                     |
    /// | 0x0A   | RANDOM   | Bits from polynomial counter (8 bits)       |
    /// | 0x0D   | SERIN    | Serial input data byte                      |
    /// | 0x0E   | IRQST    | Interrupt status (active-low: 0=pending)    |
    /// | 0x0F   | SKSTAT   | Serial/keyboard status                      |
    ///
    /// Reading RANDOM returns the upper bits of either the 9-bit or 17-bit
    /// polynomial counter, selected by AUDCTL bit 7.
    pub fn read(&mut self, offset: u8) -> u8 {
        match offset & 0x0F {
            0x00..=0x07 => {
                // POT0-POT7: Read pot counter value
                let idx = (offset & 0x07) as usize;
                self.pot_counter[idx]
            }
            0x08 => self.pot_done, // ALLPOT
            0x09 => self.kbcode,   // KBCODE
            0x0A => {
                // RANDOM: High bits of poly counter
                if self.audctl & AUDCTL_POLY9 != 0 {
                    (self.poly9 >> 1) as u8
                } else {
                    (self.poly17 >> 9) as u8
                }
            }
            0x0D => self.serin,  // SERIN
            0x0E => self.irqst,  // IRQST
            0x0F => self.skstat, // SKSTAT
            _ => 0xFF,
        }
    }

    /// Write to a POKEY register. `offset` is masked to 4 bits (0x00-0x0F).
    ///
    /// | Offset | Register | Effect                                         |
    /// |--------|----------|------------------------------------------------|
    /// | 0x00/02/04/06 | AUDFn | Set frequency divider reload for channel n |
    /// | 0x01/03/05/07 | AUDCn | Set volume, distortion, and tone gate      |
    /// | 0x08   | AUDCTL   | Set master audio control flags                 |
    /// | 0x09   | STIMER   | Reset all channel dividers to reload values    |
    /// | 0x0A   | SKREST   | Reset serial status error bits                 |
    /// | 0x0B   | POTGO    | Start potentiometer scan cycle                 |
    /// | 0x0D   | SEROUT   | Write serial output data byte                  |
    /// | 0x0E   | IRQEN    | Set interrupt enable mask                      |
    /// | 0x0F   | SKCTL    | Set serial port control                        |
    ///
    /// Writing IRQEN also clears any pending interrupts for newly-disabled
    /// sources (sets the corresponding IRQST bits to 1).
    pub fn write(&mut self, offset: u8, data: u8) {
        let masked_offset = offset & 0x0F;
        match masked_offset {
            0x00 | 0x02 | 0x04 | 0x06 => {
                // AUDF1, AUDF2, AUDF3, AUDF4
                let idx = (masked_offset / 2) as usize;
                self.audf[idx] = data;
            }
            0x01 | 0x03 | 0x05 | 0x07 => {
                // AUDC1, AUDC2, AUDC3, AUDC4
                let idx = (masked_offset / 2) as usize;
                self.audc[idx] = data;
            }
            0x08 => self.audctl = data, // AUDCTL
            0x09 => {
                // STIMER: Reset all channel dividers to their reload values.
                // Only resets channel counters and output flip-flops; the
                // base clock dividers (28/114) are free-running and unaffected.
                for i in 0..4 {
                    self.divider[i] = self.audf[i] as u16;
                    self.div_out[i] = false;
                }
            }
            0x0A => {
                // SKREST: Reset serial status error bits only.
                // Clears frame error (bit 7), overrun (bit 6), and data ready (bit 3).
                // Does NOT affect keyboard-related bits (5, 4) or other status.
                self.skstat |= SKSTAT_RESET_MASK;
            }
            0x0B => {
                // POTGO: Start pot scan
                self.pot_scanning = true;
                self.pot_scan_count = 0;
                self.pot_counter = [0; 8];
                self.pot_done = 0xFF;
            }
            0x0D => self.serout = data, // SEROUT
            0x0E => {
                // IRQEN: Enable mask
                self.irqen = data;
                // Writing clears disabled interrupts (sets IRQST bit to 1)
                self.irqst |= !data;
            }
            0x0F => self.skctl = data, // SKCTL
            _ => {}
        }
    }

    /// Advance the POKEY by one master clock cycle (1.79 MHz).
    ///
    /// This executes the full audio pipeline: polynomial counter step,
    /// base clock division, channel divider clocking, high-pass filtering,
    /// distortion gating, volume mixing, resampling, and pot scanning.
    /// Call this once per CPU clock cycle.
    pub fn tick(&mut self) {
        // 1. Advance polynomial counters
        self.step_polys();

        // 2. Advance base clocks
        let mut tick_64k = false;
        let mut tick_15k = false;

        self.base_div28 -= 1;
        if self.base_div28 == 0 {
            self.base_div28 = 28;
            tick_64k = true;
        }

        self.base_div114 -= 1;
        if self.base_div114 == 0 {
            self.base_div114 = 114;
            tick_15k = true;
        }

        // 3. Clock channels
        let base_tick = if (self.audctl & AUDCTL_CLOCK_15KHZ) != 0 {
            tick_15k
        } else {
            tick_64k
        };

        // Channel 1
        let ch1_tick = if (self.audctl & AUDCTL_CH1_179MHZ) != 0 {
            true
        } else {
            base_tick
        };
        let ch1_linked = (self.audctl & AUDCTL_CH12_LINKED) != 0;

        if ch1_linked {
            // 16-bit mode for Ch1+Ch2
            if ch1_tick {
                if self.divider[0] == 0 {
                    // Reload 16-bit value: AUDF1 (low) + AUDF2 (high)
                    let reload = (self.audf[0] as u16) | ((self.audf[1] as u16) << 8);
                    self.divider[0] = reload;

                    // Toggle Ch2 output (Ch1 output is ignored in linked mode)
                    self.div_out[1] = !self.div_out[1];

                    // IRQ for Ch2 (Timer 2)
                    if (self.irqen & IRQ_TIMER2) != 0 {
                        self.irqst &= !IRQ_TIMER2;
                    }
                } else {
                    self.divider[0] -= 1;
                }
            }
        } else {
            // 8-bit mode for Ch1
            if ch1_tick {
                if self.divider[0] == 0 {
                    self.divider[0] = self.audf[0] as u16;
                    self.div_out[0] = !self.div_out[0];
                    // IRQ for Ch1 (Timer 1)
                    if (self.irqen & IRQ_TIMER1) != 0 {
                        self.irqst &= !IRQ_TIMER1;
                    }
                } else {
                    self.divider[0] -= 1;
                }
            }

            // 8-bit mode for Ch2 (always uses base clock in 8-bit mode)
            if base_tick {
                if self.divider[1] == 0 {
                    self.divider[1] = self.audf[1] as u16;
                    self.div_out[1] = !self.div_out[1];
                    // IRQ for Ch2 (Timer 2)
                    if (self.irqen & IRQ_TIMER2) != 0 {
                        self.irqst &= !IRQ_TIMER2;
                    }
                } else {
                    self.divider[1] -= 1;
                }
            }
        }

        // Channel 3
        let ch3_tick = if (self.audctl & AUDCTL_CH3_179MHZ) != 0 {
            true
        } else {
            base_tick
        };
        let ch3_linked = (self.audctl & AUDCTL_CH34_LINKED) != 0;

        if ch3_linked {
            // 16-bit mode for Ch3+Ch4
            if ch3_tick {
                if self.divider[2] == 0 {
                    let reload = (self.audf[2] as u16) | ((self.audf[3] as u16) << 8);
                    self.divider[2] = reload;

                    self.div_out[3] = !self.div_out[3];

                    // Capture Ch2 output into HPF flip-flop on Ch4 underflow edge
                    if (self.audctl & AUDCTL_HPF_CH2) != 0 {
                        self.hp_ff[1] = self.div_out[1];
                    }

                    // IRQ for Ch4 (Timer 4)
                    if (self.irqen & IRQ_TIMER4) != 0 {
                        self.irqst &= !IRQ_TIMER4;
                    }
                } else {
                    self.divider[2] -= 1;
                }
            }
        } else {
            // 8-bit mode for Ch3
            if ch3_tick {
                if self.divider[2] == 0 {
                    self.divider[2] = self.audf[2] as u16;
                    self.div_out[2] = !self.div_out[2];

                    // Capture Ch1 output into HPF flip-flop on Ch3 underflow edge
                    if (self.audctl & AUDCTL_HPF_CH1) != 0 {
                        self.hp_ff[0] = self.div_out[0];
                    }
                    // Ch3 has no IRQ
                } else {
                    self.divider[2] -= 1;
                }
            }

            // 8-bit mode for Ch4
            if base_tick {
                if self.divider[3] == 0 {
                    self.divider[3] = self.audf[3] as u16;
                    self.div_out[3] = !self.div_out[3];

                    // Capture Ch2 output into HPF flip-flop on Ch4 underflow edge
                    if (self.audctl & AUDCTL_HPF_CH2) != 0 {
                        self.hp_ff[1] = self.div_out[1];
                    }

                    // IRQ for Ch4 (Timer 4)
                    if (self.irqen & IRQ_TIMER4) != 0 {
                        self.irqst &= !IRQ_TIMER4;
                    }
                } else {
                    self.divider[3] -= 1;
                }
            }
        }

        // 4. Generate audio output
        let mut mixed_sample = 0.0;

        for i in 0..4 {
            let audc = self.audc[i];
            let vol = audc & AUDC_VOL_MASK;
            let dist = (audc & AUDC_DIST_MASK) >> AUDC_DIST_SHIFT;

            // When AUDC bit 4 is set, the channel output is forced to the
            // volume level (bypassing tone/polynomial gating). This is
            // "volume only" mode, used for DAC-style sample playback.
            let volume_only = (audc & AUDC_VOLUME_ONLY) != 0;

            let mut signal = if volume_only {
                true
            } else {
                let poly_val = self.get_poly_output(dist);
                self.div_out[i] && poly_val
            };

            // High-pass filter: XOR with captured flip-flop value.
            // The flip-flop captures the source channel's output on the
            // modulating channel's divider underflow edge (see step 3 above).
            if i == 0 && (self.audctl & AUDCTL_HPF_CH1) != 0 {
                signal ^= self.hp_ff[0];
            }
            if i == 1 && (self.audctl & AUDCTL_HPF_CH2) != 0 {
                signal ^= self.hp_ff[1];
            }

            self.channel_out[i] = signal;

            if signal {
                mixed_sample += vol as f32;
            }
        }

        // Normalize (max vol 15 * 4 = 60)
        mixed_sample /= 60.0;

        // 5. Resample
        self.sample_accum += mixed_sample;
        self.sample_count += 1;
        self.sample_phase += self.output_sample_rate as u64;

        if self.sample_phase >= self.master_clock_hz as u64 {
            self.sample_phase -= self.master_clock_hz as u64;
            let sample = self.sample_accum / self.sample_count as f32;
            self.sample_buffer.push(sample);
            self.sample_accum = 0.0;
            self.sample_count = 0;
        }

        // 6. Pot scanning (runs at 15 kHz, stops after POT_SCAN_MAX ticks)
        if self.pot_scanning && tick_15k {
            self.pot_scan_count = self.pot_scan_count.saturating_add(1);
            for i in 0..8 {
                if (self.pot_done & (1 << i)) != 0 {
                    self.pot_counter[i] = self.pot_counter[i].wrapping_add(1);
                    if self.pot_counter[i] >= self.pot_input[i] {
                        self.pot_done &= !(1 << i);
                    }
                }
            }
            if self.pot_scan_count >= POT_SCAN_MAX {
                self.pot_scanning = false;
            }
        }
    }

    /// Advance all four polynomial counters (LFSRs) by one step.
    ///
    /// - 4-bit:  taps at bits 3,2; period 15
    /// - 5-bit:  taps at bits 4,2; period 31
    /// - 9-bit:  taps at bits 8,3; period 511
    /// - 17-bit: taps at bits 16,4; period 131071
    fn step_polys(&mut self) {
        // 4-bit: feedback = bit3 XOR bit2, shift left
        let bit3 = (self.poly4 >> 3) & 1;
        let bit2 = (self.poly4 >> 2) & 1;
        let new_bit = bit3 ^ bit2;
        self.poly4 = ((self.poly4 << 1) | new_bit) & 0x0F;

        // 5-bit: feedback = bit4 XOR bit2
        let bit4 = (self.poly5 >> 4) & 1;
        let bit2 = (self.poly5 >> 2) & 1;
        let new_bit = bit4 ^ bit2;
        self.poly5 = ((self.poly5 << 1) | new_bit) & 0x1F;

        // 9-bit: feedback = bit8 XOR bit3
        let bit8 = (self.poly9 >> 8) & 1;
        let bit3 = (self.poly9 >> 3) & 1;
        let new_bit = bit8 ^ bit3;
        self.poly9 = ((self.poly9 << 1) | new_bit) & 0x1FF;

        // 17-bit: feedback = bit16 XOR bit4
        let bit16 = (self.poly17 >> 16) & 1;
        let bit4 = (self.poly17 >> 4) & 1;
        let new_bit = bit16 ^ bit4;
        self.poly17 = ((self.poly17 << 1) | new_bit) & 0x1FFFF;
    }

    /// Return the current output bit for the given distortion mode.
    ///
    /// The 3-bit `dist` field from AUDC bits 7:5 selects which polynomial
    /// counter combination gates the channel's square wave:
    ///
    /// | dist | Polynomials      | Sound character            |
    /// |------|------------------|----------------------------|
    /// | 0    | 5-bit AND 17-bit | Harsh noise                |
    /// | 1,3  | 5-bit only       | Buzzy tone                 |
    /// | 2    | 5-bit AND 4-bit  | Gritty buzz                |
    /// | 4    | 17-bit only      | White noise                |
    /// | 5,7  | None (pure tone) | Clean square wave          |
    /// | 6    | 4-bit only       | "Metallic" 15-cycle noise  |
    ///
    /// When AUDCTL bit 7 is set, the 17-bit counter is replaced by the
    /// 9-bit counter (shorter period = coarser noise).
    fn get_poly_output(&self, dist: u8) -> bool {
        let poly9_mode = (self.audctl & AUDCTL_POLY9) != 0;
        let p4 = (self.poly4 & 1) != 0;
        let p5 = (self.poly5 & 1) != 0;
        let p17 = if poly9_mode {
            (self.poly9 & 1) != 0
        } else {
            (self.poly17 & 1) != 0
        };

        match dist {
            0 => p5 && p17, // 5-bit AND 17-bit
            1 | 3 => p5,    // 5-bit only
            2 => p5 && p4,  // 5-bit AND 4-bit
            4 => p17,       // 17-bit only
            6 => p4,        // 4-bit only
            _ => true,      // Pure tone (covers 5, 7)
        }
    }

    /// Take the accumulated resampled audio buffer and return it.
    ///
    /// Returns a `Vec<f32>` of mono samples in the range \[0.0, 1.0\],
    /// resampled from 1.79 MHz to the configured output sample rate.
    /// The buffer is emptied after this call.
    pub fn drain_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.sample_buffer)
    }

    /// Check if the POKEY's IRQ output line is asserted.
    ///
    /// Returns `true` if any enabled interrupt source is pending:
    /// `(NOT IRQST) AND IRQEN != 0`.
    pub fn irq(&self) -> bool {
        (!self.irqst & self.irqen) != 0
    }

    /// Set the external potentiometer input value for a given pot (0-7).
    ///
    /// Called by board logic to provide the target value that the pot scan
    /// counter will count up to. When the counter reaches this value, the
    /// corresponding ALLPOT bit clears.
    pub fn set_pot_input(&mut self, pot: usize, value: u8) {
        if pot < 8 {
            self.pot_input[pot] = value;
        }
    }

    /// Set the keyboard code register (called by board logic).
    pub fn set_kbcode(&mut self, code: u8) {
        self.kbcode = code;
    }

    /// Set the serial input data register (called by board logic).
    pub fn set_serin(&mut self, data: u8) {
        self.serin = data;
    }

    /// Read the serial output data register (called by board logic).
    pub fn read_serout(&self) -> u8 {
        self.serout
    }
}

impl Default for Pokey {
    fn default() -> Self {
        Self::new(44100)
    }
}
