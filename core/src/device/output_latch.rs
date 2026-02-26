/// 74LS259 8-bit addressable latch.
///
/// Address lines A0-A2 select which output bit to set/clear.
/// One data line (typically D0) provides the value. The caller extracts
/// the relevant data bit before calling [`write()`](OutputLatch::write).
#[derive(Default)]
pub struct OutputLatch {
    value: u8,
}

impl OutputLatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the full 8-bit latch state.
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Set or clear output `bit` (0-7). Returns the previous latch state
    /// (useful for edge detection on specific bits).
    pub fn write(&mut self, bit: u8, data: bool) -> u8 {
        let old = self.value;
        if data {
            self.value |= 1 << bit;
        } else {
            self.value &= !(1 << bit);
        }
        old
    }

    /// Test whether output `bit` (0-7) is set.
    pub fn bit(&self, n: u8) -> bool {
        self.value & (1 << n) != 0
    }

    /// Reset all outputs to zero (active-low clear).
    pub fn reset(&mut self) {
        self.value = 0;
    }
}

impl super::Device for OutputLatch {
    fn name(&self) -> &'static str {
        "74LS259"
    }
    fn reset(&mut self) {
        self.reset();
    }
}

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for OutputLatch {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![DebugRegister {
            name: "VALUE",
            value: self.value as u64,
            width: 8,
        }]
    }
}

use crate::core::save_state::{SaveError, Saveable, StateReader, StateWriter};

impl Saveable for OutputLatch {
    fn save_state(&self, w: &mut StateWriter) {
        w.write_u8(self.value);
    }

    fn load_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        self.value = r.read_u8()?;
        Ok(())
    }
}
