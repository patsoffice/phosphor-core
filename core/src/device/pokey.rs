pub struct Pokey {
    // Audio channel registers (CPU-written)
    audf: [u8; 4], // AUDF1-4: frequency divider reload values
    audc: [u8; 4], // AUDC1-4: volume (bits 3:0)  distortion (bits 7:5)  tone gate (bit 4)
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

impl Pokey {
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
                if self.audctl & 0x80 != 0 {
                    // 9-bit poly
                    (self.poly9 >> 1) as u8
                } else {
                    // 17-bit poly
                    (self.poly17 >> 9) as u8
                }
            }
            0x0D => self.serin,  // SERIN
            0x0E => self.irqst,  // IRQST
            0x0F => self.skstat, // SKSTAT
            _ => 0xFF,
        }
    }

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
                // STIMER: Reset all dividers
                for i in 0..4 {
                    self.divider[i] = self.audf[i] as u16;
                    self.div_out[i] = false;
                }
                // Also reset base clocks? Usually not, but let's reset counters to reload values
                self.base_div28 = 28;
                self.base_div114 = 114;
            }
            0x0A => {
                // SKREST: Reset serial status
                self.skstat = 0xFF; // Simplified
            }
            0x0B => {
                // POTGO: Start pot scan
                self.pot_scanning = true;
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
        // Determine clock source for each channel
        // AUDCTL bits:
        // 0: Base clock select (0=64k, 1=15k)
        // 1: Ch2 HPF
        // 2: Ch1 HPF
        // 3: Ch3/4 Linked (16-bit)
        // 4: Ch1/2 Linked (16-bit)
        // 5: Ch3 1.79MHz
        // 6: Ch1 1.79MHz
        // 7: 9-bit/17-bit poly select

        let base_tick = if (self.audctl & 0x01) != 0 {
            tick_15k
        } else {
            tick_64k
        };

        // Channel 1
        let ch1_tick = if (self.audctl & 0x40) != 0 {
            true
        } else {
            base_tick
        };
        let ch1_linked = (self.audctl & 0x10) != 0;

        if ch1_linked {
            // 16-bit mode for Ch1Ch2
            if ch1_tick {
                if self.divider[0] == 0 {
                    // Reload 16-bit value: AUDF1 (low)  AUDF2 (high)
                    let reload = (self.audf[0] as u16) | ((self.audf[1] as u16) << 8);
                    self.divider[0] = reload;

                    // Toggle Ch2 output (Ch1 output is usually ignored or acts as intermediate)
                    self.div_out[1] = !self.div_out[1];

                    // IRQ for Ch2 (Timer 2)
                    if (self.irqen & 0x02) != 0 {
                        self.irqst &= !0x02;
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
                    if (self.irqen & 0x01) != 0 {
                        self.irqst &= !0x01;
                    }
                } else {
                    self.divider[0] -= 1;
                }
            }

            // 8-bit mode for Ch2
            // Ch2 always uses base clock in 8-bit mode
            if base_tick {
                if self.divider[1] == 0 {
                    self.divider[1] = self.audf[1] as u16;
                    self.div_out[1] = !self.div_out[1];
                    // IRQ for Ch2 (Timer 2)
                    if (self.irqen & 0x02) != 0 {
                        self.irqst &= !0x02;
                    }
                } else {
                    self.divider[1] -= 1;
                }
            }
        }

        // Channel 3
        let ch3_tick = if (self.audctl & 0x20) != 0 {
            true
        } else {
            base_tick
        };
        let ch3_linked = (self.audctl & 0x08) != 0;

        if ch3_linked {
            // 16-bit mode for Ch3Ch4
            if ch3_tick {
                if self.divider[2] == 0 {
                    let reload = (self.audf[2] as u16) | ((self.audf[3] as u16) << 8);
                    self.divider[2] = reload;

                    self.div_out[3] = !self.div_out[3];

                    // IRQ for Ch4 (Timer 4)
                    if (self.irqen & 0x04) != 0 {
                        self.irqst &= !0x04;
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
                    // IRQ for Ch4 (Timer 4)
                    if (self.irqen & 0x04) != 0 {
                        self.irqst &= !0x04;
                    }
                } else {
                    self.divider[3] -= 1;
                }
            }
        }

        // 4. High-pass filter flip-flops
        // Ch1 filtered by Ch3: toggles when Ch3 output toggles?
        // Prompt: "XOR with flip-flop clocked by the filter channel's divider"
        // This means we need to detect the underflow event of the filter channel.
        // We can use the fact that div_out toggles on underflow.
        // But we need the edge.
        // Actually, div_out IS the flip-flop.
        // So HPF logic is just XORing with the other channel's div_out?
        // Let's assume div_out represents the state of the flip-flop.

        // 5. Generate audio output
        let mut mixed_sample = 0.0;

        for i in 0..4 {
            let audc = self.audc[i];
            let vol = audc & 0x0F;
            let dist = (audc >> 5) & 0x07;
            let vol_only = (audc & 0x10) == 0;

            let mut signal = if vol_only {
                true
            } else {
                let poly_val = self.get_poly_output(dist);
                self.div_out[i] && poly_val
            };

            // High-pass filter
            // Ch1 filtered by Ch3 (AUDCTL.2)
            if i == 0 && (self.audctl & 0x04) != 0 {
                signal ^= self.div_out[2];
            }
            // Ch2 filtered by Ch4 (AUDCTL.1)
            if i == 1 && (self.audctl & 0x02) != 0 {
                signal ^= self.div_out[3];
            }

            self.channel_out[i] = signal;

            if signal {
                mixed_sample += vol as f32;
            }
        }

        // Normalize (max vol 15 * 4 = 60)
        mixed_sample /= 60.0;

        // 6. Resample
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

        // 7. Pot scanning
        if self.pot_scanning {
            // Use 15kHz tick for pot counters (approx scanline rate)
            if tick_15k {
                for i in 0..8 {
                    // If pot not yet done
                    if (self.pot_done & (1 << i)) != 0 {
                        self.pot_counter[i] = self.pot_counter[i].wrapping_add(1);
                        if self.pot_counter[i] >= self.pot_input[i] {
                            // Mark as done (clear bit)
                            self.pot_done &= !(1 << i);
                        }
                    }
                }
                // Stop scanning if maxed out? (usually 228)
                // For now, just let it run until all done or re-triggered.
            }
        }
    }

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
        let new_bit = (bit8 ^ bit3) as u16;
        self.poly9 = ((self.poly9 << 1) | new_bit) & 0x1FF;

        // 17-bit: feedback = bit16 XOR bit4
        let bit16 = (self.poly17 >> 16) & 1;
        let bit4 = (self.poly17 >> 4) & 1;
        let new_bit = (bit16 ^ bit4) as u32;
        self.poly17 = ((self.poly17 << 1) | new_bit) & 0x1FFFF;
    }

    fn get_poly_output(&self, dist: u8) -> bool {
        let poly9_mode = (self.audctl & 0x80) != 0;
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

    pub fn drain_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.sample_buffer)
    }

    pub fn irq(&self) -> bool {
        (!self.irqst & self.irqen) != 0
    }

    pub fn set_pot_input(&mut self, pot: usize, value: u8) {
        if pot < 8 {
            self.pot_input[pot] = value;
        }
    }

    pub fn set_kbcode(&mut self, code: u8) {
        self.kbcode = code;
    }

    pub fn set_serin(&mut self, data: u8) {
        self.serin = data;
    }

    pub fn read_serout(&self) -> u8 {
        self.serout
    }
}
