pub struct Mc1408Dac {
    /// Most recent value written by the CPU (0-255 unsigned).
    value: u8,
}

impl Default for Mc1408Dac {
    fn default() -> Self {
        Self { value: 0x80 }
    }
}

impl Mc1408Dac {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called when the sound PIA Port A is written.
    pub fn write(&mut self, data: u8) {
        self.value = data;
    }

    /// Return current output as a signed 16-bit PCM sample.
    /// Maps 0x00 → -32768, 0x80 → 0, 0xFF → +32512.
    pub fn sample_i16(&self) -> i16 {
        ((self.value as i16) - 128) * 256
    }
}
