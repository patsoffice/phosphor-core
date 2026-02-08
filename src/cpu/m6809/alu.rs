use super::{CcFlag, ExecState, M6809};
use crate::core::{Bus, BusMaster};

mod binary;
mod shift;
mod unary;
mod word;

impl M6809 {
    /// Helper to set N, Z, V (cleared) flags for logical operations
    #[inline]
    pub(crate) fn set_flags_logical(&mut self, result: u8) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// Helper to set N, Z, V, C flags for arithmetic operations
    #[inline]
    pub(crate) fn set_flags_arithmetic(&mut self, result: u8, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V, C flags for 16-bit arithmetic
    #[inline]
    pub(crate) fn set_flags_arithmetic16(&mut self, result: u16, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V (cleared) flags for 16-bit logical operations
    #[inline]
    pub(crate) fn set_flags_logical16(&mut self, result: u16) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// The alu_imm function is a generic helper method designed to reduce code duplication for Immediate Addressing Mode ALU instructions (like ADDA #$10, ANDB #$FF, etc.).
    ///
    /// In the Motorola 6809, immediate mode instructions always follow a specific pattern.
    #[inline]
    pub(crate) fn alu_imm<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 0 {
            // 1. Fetch the operand from memory at PC
            let operand = bus.read(master, self.pc);
            // 2. Advance PC to the next instruction
            self.pc = self.pc.wrapping_add(1);
            // 3. Run the specific ALU logic provided by the caller
            operation(self, operand);
            // 4. Return to Fetch state for the next instruction
            self.state = ExecState::Fetch;
        }
    }

    // --- Shift and Rotate instructions ---

    /// Helper to set N, Z, V, C flags for shift/rotate operations.
    /// V is always set to N XOR C (post-operation) per 6809 datasheet.
    #[inline]
    pub(crate) fn set_flags_shift(&mut self, result: u8, carry: bool) {
        let n = result & 0x80 != 0;
        self.set_flag(CcFlag::N, n);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, n ^ carry);
        self.set_flag(CcFlag::C, carry);
    }
}
