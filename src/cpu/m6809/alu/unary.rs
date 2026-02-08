use crate::cpu::m6809::{CcFlag, ExecState, M6809};

impl M6809 {
    /// NEGA inherent (0x40): Negate A (A = 0 - A, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if A was non-zero (borrow occurred from 0).
    pub(crate) fn op_nega(&mut self, cycle: u8) {
        if cycle == 0 {
            let (result, borrow) = (0u8).overflowing_sub(self.a);
            // Overflow occurs if A was 0x80 (-128), because -(-128) = +128 is not representable in i8
            let overflow = self.a == 0x80;
            self.a = result;
            self.set_flags_arithmetic(result, overflow, borrow);
            self.state = ExecState::Fetch;
        }
    }

    /// NEGB inherent (0x50): Negate B (B = 0 - B, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if B was non-zero (borrow occurred from 0).
    pub(crate) fn op_negb(&mut self, cycle: u8) {
        if cycle == 0 {
            let (result, borrow) = (0u8).overflowing_sub(self.b);
            let overflow = self.b == 0x80;
            self.b = result;
            self.set_flags_arithmetic(result, overflow, borrow);
            self.state = ExecState::Fetch;
        }
    }

    /// COMA inherent (0x43): Complement A (A = ~A, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_coma(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = !self.a;
            // COM sets N, Z, clears V, and sets C=1
            self.set_flags_logical(self.a);
            self.set_flag(CcFlag::C, true);
            self.state = ExecState::Fetch;
        }
    }

    /// COMB inherent (0x53): Complement B (B = ~B, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_comb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = !self.b;
            self.set_flags_logical(self.b);
            self.set_flag(CcFlag::C, true);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRA inherent (0x4F): Clear A (A = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = 0;
            self.set_flag(CcFlag::N, false);
            self.set_flag(CcFlag::Z, true);
            self.set_flag(CcFlag::V, false);
            self.set_flag(CcFlag::C, false);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRB inherent (0x5F): Clear B (B = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = 0;
            self.set_flag(CcFlag::N, false);
            self.set_flag(CcFlag::Z, true);
            self.set_flag(CcFlag::V, false);
            self.set_flag(CcFlag::C, false);
            self.state = ExecState::Fetch;
        }
    }

    /// INCA inherent (0x4C): Increment A (A = A + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_inca(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.a == 0x7F;
            self.a = self.a.wrapping_add(1);
            self.set_flag(CcFlag::N, self.a & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.a == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// INCB inherent (0x5C): Increment B (B = B + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_incb(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.b == 0x7F;
            self.b = self.b.wrapping_add(1);
            self.set_flag(CcFlag::N, self.b & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.b == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// DECA inherent (0x4A): Decrement A (A = A - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_deca(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.a == 0x80;
            self.a = self.a.wrapping_sub(1);
            self.set_flag(CcFlag::N, self.a & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.a == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// DECB inherent (0x5A): Decrement B (B = B - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_decb(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.b == 0x80;
            self.b = self.b.wrapping_sub(1);
            self.set_flag(CcFlag::N, self.b & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.b == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTA inherent (0x4D): Test A (set flags based on A, no modification).
    /// N set if A bit 7 is set. Z set if A is zero. V always cleared.
    pub(crate) fn op_tsta(&mut self, cycle: u8) {
        if cycle == 0 {
            self.set_flags_logical(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTB inherent (0x5D): Test B (set flags based on B, no modification).
    /// N set if B bit 7 is set. Z set if B is zero. V always cleared.
    pub(crate) fn op_tstb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.set_flags_logical(self.b);
            self.state = ExecState::Fetch;
        }
    }
}
