use phosphor_macros::Saveable;

/// Namco 53XX custom chip — DIP switch reader.
///
/// In hardware, this is a Fujitsu MB8843 MCU that reads DIP switch
/// settings and returns them as a sequence of nibbles. We emulate the
/// external behavior directly.
///
/// Returns 4 nibbles per read cycle:
///   [DSWA low, DSWA high, DSWB low, DSWB high]
#[derive(Saveable)]
#[save_version(1)]
pub struct Namco53 {
    /// Nibble sequence counter (0-3).
    pub read_index: u8,
}

impl Namco53 {
    pub fn new() -> Self {
        Self { read_index: 0 }
    }

    /// Read the next DIP switch nibble.
    /// `dswa` and `dswb` are the current DIP switch byte values.
    pub fn read(&mut self, dswa: u8, dswb: u8) -> u8 {
        let idx = self.read_index;
        self.read_index = (self.read_index + 1) % 4;

        let nibble = match idx {
            0 => dswa & 0x0F,
            1 => (dswa >> 4) & 0x0F,
            2 => dswb & 0x0F,
            3 => (dswb >> 4) & 0x0F,
            _ => unreachable!(),
        };

        // The 53xx MCU firmware (mode 7, used by Dig Dug) encodes the port
        // index in the upper nibble: O = (port_index << 4) | data_nibble.
        // The game code uses bit 5 of the output as a "data valid" indicator
        // (set for ports 2-3, i.e. DSWB reads).
        (idx << 4) | nibble
    }

    pub fn reset(&mut self) {
        self.read_index = 0;
    }
}

impl Default for Namco53 {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Device for Namco53 {
    fn name(&self) -> &'static str {
        "Namco 53XX"
    }
    fn reset(&mut self) {
        self.reset();
    }
}

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for Namco53 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![DebugRegister {
            name: "READ_IDX",
            value: self.read_index as u64,
            width: 2,
        }]
    }
}
