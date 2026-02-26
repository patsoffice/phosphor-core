//! Shared audio resampling utilities.
//!
//! Provides Bresenham box-filter downsamplers for converting high-frequency
//! audio (at CPU clock rates) to standard output sample rates (e.g., 44.1 kHz).
//! Two variants are provided: `AudioResampler` (i64/i16) for most devices,
//! and `AudioResamplerF32` (f32) for POKEY and other float-based pipelines.

use crate::core::save_state::{SaveError, Saveable, StateReader, StateWriter};

// ---------------------------------------------------------------------------
// AudioResampler (i64 accumulator, i16 output)
// ---------------------------------------------------------------------------

/// Bresenham box-filter audio resampler.
///
/// Downsamples from an input clock rate to an output sample rate using
/// box-filter averaging. Each `tick()` accumulates a sample; when the
/// Bresenham phase crosses the threshold, the averaged sample is pushed
/// to the internal buffer.
pub struct AudioResampler {
    input_rate: u64,
    output_rate: u64,
    sample_accum: i64,
    sample_count: u32,
    sample_phase: u64,
    buffer: Vec<i16>,
}

impl AudioResampler {
    /// Create a new resampler.
    ///
    /// - `input_rate`: source clock rate in Hz (e.g., 3_072_000 for 3.072 MHz CPU)
    /// - `output_rate`: target sample rate in Hz (e.g., 44_100)
    pub fn new(input_rate: u64, output_rate: u64) -> Self {
        Self {
            input_rate,
            output_rate,
            sample_accum: 0,
            sample_count: 0,
            sample_phase: 0,
            buffer: Vec::with_capacity(2048),
        }
    }

    /// Accumulate one input sample. If this tick completes an output sample,
    /// the box-filtered average is automatically pushed to the internal buffer.
    #[inline]
    pub fn tick(&mut self, sample: i16) {
        if let Some(avg) = self.tick_sample(sample) {
            self.buffer.push(avg);
        }
    }

    /// Accumulate one input sample. If this tick completes an output sample,
    /// returns the box-filtered average without pushing it to the buffer.
    ///
    /// Use this when you need to post-process (e.g., mix with another source)
    /// before calling [`push_sample`].
    #[inline]
    pub fn tick_sample(&mut self, sample: i16) -> Option<i16> {
        self.sample_accum += sample as i64;
        self.sample_count += 1;
        self.sample_phase += self.output_rate;

        if self.sample_phase >= self.input_rate {
            self.sample_phase -= self.input_rate;
            let avg = if self.sample_count > 0 {
                (self.sample_accum / self.sample_count as i64) as i16
            } else {
                0
            };
            self.sample_accum = 0;
            self.sample_count = 0;
            Some(avg)
        } else {
            None
        }
    }

    /// Manually push a sample to the output buffer.
    ///
    /// Use after [`tick_sample`] returns `Some` and you've mixed or
    /// post-processed the averaged sample.
    #[inline]
    pub fn push_sample(&mut self, sample: i16) {
        self.buffer.push(sample);
    }

    /// Drain audio samples into the provided buffer. Returns the number
    /// of samples written.
    pub fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        let n = buffer.len().min(self.buffer.len());
        buffer[..n].copy_from_slice(&self.buffer[..n]);
        self.buffer.drain(..n);
        n
    }

    /// Clear all runtime state (phase, accumulator, buffer).
    pub fn reset(&mut self) {
        self.sample_accum = 0;
        self.sample_count = 0;
        self.sample_phase = 0;
        self.buffer.clear();
    }
}

impl Saveable for AudioResampler {
    fn save_state(&self, w: &mut StateWriter) {
        w.write_i64_le(self.sample_accum);
        w.write_u32_le(self.sample_count);
        w.write_u64_le(self.sample_phase);
    }

    fn load_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        self.sample_accum = r.read_i64_le()?;
        self.sample_count = r.read_u32_le()?;
        self.sample_phase = r.read_u64_le()?;
        self.buffer.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AudioResamplerF32 (f32 accumulator, f32 output)
// ---------------------------------------------------------------------------

/// Bresenham box-filter audio resampler (f32 variant).
///
/// Identical algorithm to [`AudioResampler`] but uses `f32` accumulator
/// and buffer. Used by POKEY and other float-based audio pipelines.
pub struct AudioResamplerF32 {
    input_rate: u64,
    output_rate: u64,
    sample_accum: f32,
    sample_count: u32,
    sample_phase: u64,
    buffer: Vec<f32>,
}

impl AudioResamplerF32 {
    /// Create a new f32 resampler.
    pub fn new(input_rate: u64, output_rate: u64) -> Self {
        Self {
            input_rate,
            output_rate,
            sample_accum: 0.0,
            sample_count: 0,
            sample_phase: 0,
            buffer: Vec::with_capacity(2048),
        }
    }

    /// Accumulate one input sample. If this tick completes an output sample,
    /// the box-filtered average is pushed to the internal buffer.
    #[inline]
    pub fn tick(&mut self, sample: f32) {
        self.sample_accum += sample;
        self.sample_count += 1;
        self.sample_phase += self.output_rate;

        if self.sample_phase >= self.input_rate {
            self.sample_phase -= self.input_rate;
            let avg = if self.sample_count > 0 {
                self.sample_accum / self.sample_count as f32
            } else {
                0.0
            };
            self.buffer.push(avg);
            self.sample_accum = 0.0;
            self.sample_count = 0;
        }
    }

    /// Take all buffered samples, leaving the buffer empty.
    pub fn drain_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.buffer)
    }

    /// Clear all runtime state (phase, accumulator, buffer).
    pub fn reset(&mut self) {
        self.sample_accum = 0.0;
        self.sample_count = 0;
        self.sample_phase = 0;
        self.buffer.clear();
    }
}

impl Saveable for AudioResamplerF32 {
    fn save_state(&self, w: &mut StateWriter) {
        w.write_f32_le(self.sample_accum);
        w.write_u32_le(self.sample_count);
        w.write_u64_le(self.sample_phase);
    }

    fn load_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        self.sample_accum = r.read_f32_le()?;
        self.sample_count = r.read_u32_le()?;
        self.sample_phase = r.read_u64_le()?;
        self.buffer.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_produces_correct_sample_count() {
        let mut r = AudioResampler::new(1_000_000, 44_100);
        for _ in 0..1_000_000 {
            r.tick(1000);
        }
        let mut buf = vec![0i16; 50_000];
        let n = r.fill_audio(&mut buf);
        // Should produce approximately 44100 samples (± 1 for rounding)
        assert!(
            (44_099..=44_101).contains(&n),
            "expected ~44100 samples, got {n}"
        );
    }

    #[test]
    fn resampler_averages_correctly() {
        // Input rate 4, output rate 1: averages 4 samples into 1
        let mut r = AudioResampler::new(4, 1);
        r.tick(100);
        r.tick(200);
        r.tick(300);
        r.tick(400); // average = 250

        let mut buf = [0i16; 4];
        let n = r.fill_audio(&mut buf);
        assert_eq!(n, 1);
        assert_eq!(buf[0], 250);
    }

    #[test]
    fn tick_sample_returns_average_without_pushing() {
        let mut r = AudioResampler::new(4, 1);
        assert_eq!(r.tick_sample(100), None);
        assert_eq!(r.tick_sample(200), None);
        assert_eq!(r.tick_sample(300), None);
        assert_eq!(r.tick_sample(400), Some(250));

        // Buffer should be empty since tick_sample doesn't push
        let mut buf = [0i16; 4];
        assert_eq!(r.fill_audio(&mut buf), 0);
    }

    #[test]
    fn push_sample_adds_to_buffer() {
        let mut r = AudioResampler::new(1_000_000, 44_100);
        r.push_sample(42);
        r.push_sample(-100);

        let mut buf = [0i16; 4];
        let n = r.fill_audio(&mut buf);
        assert_eq!(n, 2);
        assert_eq!(buf[0], 42);
        assert_eq!(buf[1], -100);
    }

    #[test]
    fn fill_audio_drains_buffer() {
        let mut r = AudioResampler::new(2, 1);
        r.tick(100);
        r.tick(200); // produces one sample: avg 150

        let mut buf = [0i16; 1];
        assert_eq!(r.fill_audio(&mut buf), 1);
        assert_eq!(buf[0], 150);

        // Second call should return 0 (drained)
        assert_eq!(r.fill_audio(&mut buf), 0);
    }

    #[test]
    fn reset_clears_all_state() {
        let mut r = AudioResampler::new(2, 1);
        r.tick(100);
        r.tick(200);
        r.reset();

        assert_eq!(r.sample_accum, 0);
        assert_eq!(r.sample_count, 0);
        assert_eq!(r.sample_phase, 0);
        assert!(r.buffer.is_empty());
    }

    #[test]
    fn save_load_round_trip() {
        let mut r = AudioResampler::new(1_000_000, 44_100);
        for _ in 0..500 {
            r.tick(1234);
        }

        let mut w = StateWriter::new();
        r.save_state(&mut w);
        let data = w.into_vec();

        let mut r2 = AudioResampler::new(1_000_000, 44_100);
        let mut reader = StateReader::new(&data);
        r2.load_state(&mut reader).unwrap();

        assert_eq!(r2.sample_accum, r.sample_accum);
        assert_eq!(r2.sample_count, r.sample_count);
        assert_eq!(r2.sample_phase, r.sample_phase);
    }

    // -- AudioResamplerF32 tests --

    #[test]
    fn f32_resampler_produces_correct_count() {
        let mut r = AudioResamplerF32::new(1_000_000, 44_100);
        for _ in 0..1_000_000 {
            r.tick(0.5);
        }
        let samples = r.drain_audio();
        assert!(
            (44_099..=44_101).contains(&samples.len()),
            "expected ~44100 samples, got {}",
            samples.len()
        );
    }

    #[test]
    fn f32_resampler_averages_correctly() {
        let mut r = AudioResamplerF32::new(4, 1);
        r.tick(0.1);
        r.tick(0.2);
        r.tick(0.3);
        r.tick(0.4);
        let samples = r.drain_audio();
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn f32_save_load_round_trip() {
        let mut r = AudioResamplerF32::new(1_000_000, 44_100);
        for _ in 0..500 {
            r.tick(0.42);
        }

        let mut w = StateWriter::new();
        r.save_state(&mut w);
        let data = w.into_vec();

        let mut r2 = AudioResamplerF32::new(1_000_000, 44_100);
        let mut reader = StateReader::new(&data);
        r2.load_state(&mut reader).unwrap();

        assert_eq!(r2.sample_count, r.sample_count);
        assert_eq!(r2.sample_phase, r.sample_phase);
        assert!((r2.sample_accum - r.sample_accum).abs() < 1e-6);
    }
}
