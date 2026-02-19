/// Donkey Kong discrete analog sound effects.
///
/// Models three circuits from the DK sound board, driven by the 74LS259
/// control latch at 0x7D00-0x7D07 (bits 0-2):
///
/// - **Walk** (bit 0): CD4049B inverter oscillator (~1 Hz LFO) modulates a
///   555 VCO (R1=47kΩ, R2=27kΩ, C=33nF, base ~430 Hz). Active while bit set.
/// - **Jump** (bit 1): One-shot 555 VCO sweep from ~220 Hz to ~700 Hz with
///   exponential RC decay (τ ≈ 360ms). Triggered on rising edge.
/// - **Stomp** (bit 2): 24-bit LFSR noise (4 kHz clock = CLOCK_2VF) with
///   exponential amplitude decay (τ ≈ 50ms). Triggered on rising edge.
///
/// Call [`generate_sample`] at 44.1 kHz to produce output samples.
pub struct DkongDiscrete {
    // Walk: LFO-modulated VCO
    walk_lfo_phase: f64,
    walk_vco_phase: f64,

    // Jump: frequency sweep + envelope
    jump_active: bool,
    jump_timer: f64,
    jump_vco_phase: f64,

    // Stomp: noise burst + envelope
    stomp_active: bool,
    stomp_timer: f64,
    stomp_lfsr: u32,
    stomp_lfsr_clock: f64,

    // Control latch state (bits 0-2)
    latch: u8,
}

impl Default for DkongDiscrete {
    fn default() -> Self {
        Self {
            walk_lfo_phase: 0.0,
            walk_vco_phase: 0.0,
            jump_active: false,
            jump_timer: 0.0,
            jump_vco_phase: 0.0,
            stomp_active: false,
            stomp_timer: 0.0,
            stomp_lfsr: 0x1A_CFFC,
            stomp_lfsr_clock: 0.0,
            latch: 0,
        }
    }
}

// 555 astable frequency with external control voltage.
// R1=47kΩ (charge), R2=27kΩ (discharge), Vcc=5V.
// f = 1 / (t_charge + t_discharge)
// t_charge = (R1+R2)*C * ln((Vcc - CV/2) / (Vcc - CV))
// t_discharge = R2*C * ln(2)
fn vco_freq(cap_nf: f64, cv: f64) -> f64 {
    const VCC: f64 = 5.0;
    const R1: f64 = 47_000.0;
    const R2: f64 = 27_000.0;
    let c = cap_nf * 1e-9;
    let t_charge = (R1 + R2) * c * ((VCC - cv * 0.5) / (VCC - cv)).ln();
    let t_discharge = R2 * c * 2.0_f64.ln();
    1.0 / (t_charge + t_discharge)
}

impl DkongDiscrete {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a control latch bit (0-2). Detects rising edges for one-shot sounds.
    pub fn write_latch(&mut self, bit: u8, value: bool) {
        let old = self.latch;
        if value {
            self.latch |= 1 << bit;
        } else {
            self.latch &= !(1 << bit);
        }
        let rising = self.latch & !old;
        if rising & 0x02 != 0 {
            self.jump_active = true;
            self.jump_timer = 0.0;
            self.jump_vco_phase = 0.0;
        }
        if rising & 0x04 != 0 {
            self.stomp_active = true;
            self.stomp_timer = 0.0;
        }
    }

    /// Generate one output sample. Call at 44.1 kHz.
    pub fn generate_sample(&mut self) -> i16 {
        const DT: f64 = 1.0 / 44100.0;
        let mut output = 0.0f64;

        // Walk: VCO modulated by ~1 Hz LFO while bit 0 is set.
        // Inverter oscillator: TYPE2, R48=43kΩ, C30=10µF → ~1.0 Hz.
        // 555 VCO: C=33nF, CV sweeps ~2.5-3.8V → freq ~350-550 Hz.
        if self.latch & 0x01 != 0 {
            self.walk_lfo_phase += 1.0 * DT;
            if self.walk_lfo_phase >= 1.0 {
                self.walk_lfo_phase -= 1.0;
            }
            let lfo = (self.walk_lfo_phase * std::f64::consts::TAU).sin();
            // CV oscillates between ~2.5V and ~3.8V
            let cv = 3.15 + 0.65 * lfo;
            let freq = vco_freq(33.0, cv);

            self.walk_vco_phase += freq * DT;
            if self.walk_vco_phase >= 1.0 {
                self.walk_vco_phase -= 1.0;
            }
            let wave = if self.walk_vco_phase < 0.5 { 1.0 } else { -1.0 };
            output += wave * 0.12;
        }

        // Jump: one-shot sweep triggered on bit 1 rising edge.
        // 555 VCO: C=47nF. CV starts at ~4V (low freq) and decays
        // exponentially with τ=360ms (R8+R7=110kΩ, C20=3.3µF).
        // Freq sweeps from ~220 Hz to ~700 Hz.
        if self.jump_active {
            self.jump_timer += DT;
            if self.jump_timer > 0.5 {
                self.jump_active = false;
            } else {
                let t = self.jump_timer;
                // CV decays from 4.0V toward 1.0V with τ=0.36s
                let cv = 1.0 + 3.0 * (-t / 0.36).exp();
                let freq = vco_freq(47.0, cv);
                // Amplitude envelope: fast attack (10ms), slow decay (360ms)
                let amp = (-t / 0.36).exp();

                self.jump_vco_phase += freq * DT;
                if self.jump_vco_phase >= 1.0 {
                    self.jump_vco_phase -= 1.0;
                }
                let wave = if self.jump_vco_phase < 0.5 { 1.0 } else { -1.0 };
                output += wave * amp * 0.15;
            }
        }

        // Stomp: noise burst triggered on bit 2 rising edge.
        // 24-bit LFSR (XOR taps at bits 10,23) clocked at 4 kHz (CLOCK_2VF).
        // Exponential decay τ ≈ 50ms (R9=10kΩ, C21=1µF).
        if self.stomp_active {
            self.stomp_timer += DT;
            if self.stomp_timer > 0.25 {
                self.stomp_active = false;
            } else {
                self.stomp_lfsr_clock += 4000.0 * DT;
                while self.stomp_lfsr_clock >= 1.0 {
                    self.stomp_lfsr_clock -= 1.0;
                    let bit = ((self.stomp_lfsr >> 10) ^ (self.stomp_lfsr >> 23)) & 1;
                    self.stomp_lfsr = (self.stomp_lfsr >> 1) | (bit << 23);
                }
                let noise = if self.stomp_lfsr & 1 != 0 { 1.0 } else { -1.0 };
                let amp = (-self.stomp_timer / 0.05).exp();
                output += noise * amp * 0.12;
            }
        }

        (output * 32767.0).clamp(-32767.0, 32767.0) as i16
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
