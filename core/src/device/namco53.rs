use phosphor_macros::Saveable;

/// Namco 53XX custom chip — DIP switch reader.
///
/// In hardware, this is a Fujitsu MB8843 MCU that reads DIP switch
/// settings and returns them as a sequence of nibbles. We emulate the
/// external behavior directly.
///
/// Returns 2 bytes per read cycle:
///   [DSWA, DSWB]
///
/// The real MB8843 firmware reads R0-R3 (DIP switch nibbles) and packs
/// pairs into full bytes via the O port. Each Z80 read returns one
/// complete DIP switch byte, cycling between DSWA and DSWB.
#[derive(Saveable)]
#[save_version(1)]
pub struct Namco53 {
    /// Byte sequence counter (0-1).
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
        self.read_index = (self.read_index + 1) % 2;

        // The real MB8843 firmware packs two R-port nibbles per IRQ:
        //   IRQ 0: R0 (low) | R1 (high) << 4 = DSWA
        //   IRQ 1: R2 (low) | R3 (high) << 4 = DSWB
        match idx {
            0 => dswa,
            1 => dswb,
            _ => unreachable!(),
        }
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
