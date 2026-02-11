use crate::cpu::m6809::{CcFlag, ExecState, M6809};

impl M6809 {
    // --- Internal Unary Helpers ---

    #[inline]
    fn perform_neg(&mut self, val: u8) -> u8 {
        let (result, borrow) = (0u8).overflowing_sub(val);
        let overflow = val == 0x80;
        self.set_flags_arithmetic(result, overflow, borrow);
        result
    }

    #[inline]
    fn perform_com(&mut self, val: u8) -> u8 {
        let result = !val;
        self.set_flags_logical(result);
        self.set_flag(CcFlag::C, true);
        result
    }

    #[inline]
    fn perform_clr(&mut self) -> u8 {
        self.set_flag(CcFlag::N, false);
        self.set_flag(CcFlag::Z, true);
        self.set_flag(CcFlag::V, false);
        self.set_flag(CcFlag::C, false);
        0
    }

    #[inline]
    fn perform_inc(&mut self, val: u8) -> u8 {
        let overflow = val == 0x7F;
        let result = val.wrapping_add(1);
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        result
    }

    #[inline]
    fn perform_dec(&mut self, val: u8) -> u8 {
        let overflow = val == 0x80;
        let result = val.wrapping_sub(1);
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        result
    }

    #[inline]
    fn perform_tst(&mut self, val: u8) {
        self.set_flags_logical(val);
    }

    /// NEGA inherent (0x40): Negate A (A = 0 - A, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if A was non-zero (borrow occurred from 0).
    pub(crate) fn op_nega(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_neg(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// NEGB inherent (0x50): Negate B (B = 0 - B, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if B was non-zero (borrow occurred from 0).
    pub(crate) fn op_negb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_neg(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// COMA inherent (0x43): Complement A (A = ~A, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_coma(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_com(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// COMB inherent (0x53): Complement B (B = ~B, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_comb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_com(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRA inherent (0x4F): Clear A (A = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// CLRB inherent (0x5F): Clear B (B = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// INCA inherent (0x4C): Increment A (A = A + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_inca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_inc(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// INCB inherent (0x5C): Increment B (B = B + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_incb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_inc(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// DECA inherent (0x4A): Decrement A (A = A - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_deca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_dec(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// DECB inherent (0x5A): Decrement B (B = B - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_decb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_dec(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTA inherent (0x4D): Test A (set flags based on A, no modification).
    /// N set if A bit 7 is set. Z set if A is zero. V always cleared.
    pub(crate) fn op_tsta(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTB inherent (0x5D): Test B (set flags based on B, no modification).
    /// N set if B bit 7 is set. Z set if B is zero. V always cleared.
    pub(crate) fn op_tstb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.b);
            self.state = ExecState::Fetch;
        }
    }
}
