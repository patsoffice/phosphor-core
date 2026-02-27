/// Bresenham-style fractional clock divider.
///
/// Ticks a secondary clock at a fractional rate relative to the primary clock.
/// Each call to `tick()` returns `true` when the secondary clock should advance.
///
/// # Example
///
/// ```
/// use phosphor_core::core::ClockDivider;
///
/// // I8035 at 400 kHz from 3.072 MHz main clock = 25/192
/// let mut clk = ClockDivider::new(25, 192);
/// let mut fires = 0;
/// for _ in 0..192 {
///     if clk.tick() { fires += 1; }
/// }
/// assert_eq!(fires, 25);
/// ```
pub struct ClockDivider {
    phase_accum: u32,
    numerator: u32,
    denominator: u32,
}

impl ClockDivider {
    /// Create a new clock divider with the given ratio.
    ///
    /// The secondary clock fires `numerator` times per `denominator` primary ticks.
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            phase_accum: 0,
            numerator,
            denominator,
        }
    }

    /// Advance one primary clock tick. Returns `true` if the secondary
    /// clock should tick this cycle.
    #[inline]
    pub fn tick(&mut self) -> bool {
        self.phase_accum += self.numerator;
        if self.phase_accum >= self.denominator {
            self.phase_accum -= self.denominator;
            true
        } else {
            false
        }
    }

    /// Return the current phase accumulator value.
    pub fn phase(&self) -> u32 {
        self.phase_accum
    }

    /// Set the phase accumulator (for testing).
    pub fn set_phase(&mut self, phase: u32) {
        self.phase_accum = phase;
    }

    /// Reset the phase accumulator to zero.
    pub fn reset(&mut self) {
        self.phase_accum = 0;
    }
}

use super::save_state::{SaveError, Saveable, StateReader, StateWriter};

impl Saveable for ClockDivider {
    fn save_state(&self, w: &mut StateWriter) {
        w.write_version(1);
        w.write_u32_le(self.phase_accum);
    }

    fn load_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        r.read_version(1)?;
        self.phase_accum = r.read_u32_le()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_25_192_fires_exactly_25_times() {
        let mut clk = ClockDivider::new(25, 192);
        let mut fires = 0;
        for _ in 0..192 {
            if clk.tick() {
                fires += 1;
            }
        }
        assert_eq!(fires, 25);
    }

    #[test]
    fn ratio_1_1_fires_every_tick() {
        let mut clk = ClockDivider::new(1, 1);
        for _ in 0..100 {
            assert!(clk.tick());
        }
    }

    #[test]
    fn ratio_1_3_fires_once_per_three() {
        let mut clk = ClockDivider::new(1, 3);
        let mut fires = 0;
        for _ in 0..300 {
            if clk.tick() {
                fires += 1;
            }
        }
        assert_eq!(fires, 100);
    }

    #[test]
    fn reset_clears_accumulator() {
        let mut clk = ClockDivider::new(25, 192);
        // Advance partway
        for _ in 0..50 {
            clk.tick();
        }
        clk.reset();
        assert_eq!(clk.phase_accum, 0);
    }

    #[test]
    fn save_load_round_trip() {
        let mut clk = ClockDivider::new(25, 192);
        // Advance to a non-zero state
        for _ in 0..37 {
            clk.tick();
        }

        let mut w = StateWriter::new();
        clk.save_state(&mut w);
        let data = w.into_vec();

        let mut clk2 = ClockDivider::new(25, 192);
        let mut r = StateReader::new(&data);
        clk2.load_state(&mut r).unwrap();

        assert_eq!(clk2.phase_accum, clk.phase_accum);
    }
}
