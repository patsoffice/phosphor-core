use crate::cpu::m6800::{CcFlag, ExecState, M6800};

impl M6800 {
    // --- Internal Unary Helpers ---

    /// Negate (two's complement): result = 0 - val.
    /// N, Z, V, C affected.
    #[inline]
    pub(crate) fn perform_neg(&mut self, val: u8) -> u8 {
        let (result, borrow) = (0u8).overflowing_sub(val);
        let overflow = val == 0x80;
        self.set_flags_arithmetic(result, overflow, borrow);
        result
    }

    /// Complement (one's complement / bitwise NOT): result = ~val.
    /// N, Z affected. V cleared. C set.
    #[inline]
    pub(crate) fn perform_com(&mut self, val: u8) -> u8 {
        let result = !val;
        self.set_flags_logical(result);
        self.set_flag(CcFlag::C, true);
        result
    }

    /// Clear: result = 0.
    /// N=0, Z=1, V=0, C=0.
    #[inline]
    pub(crate) fn perform_clr(&mut self) -> u8 {
        self.set_flag(CcFlag::N, false);
        self.set_flag(CcFlag::Z, true);
        self.set_flag(CcFlag::V, false);
        self.set_flag(CcFlag::C, false);
        0
    }

    /// Increment: result = val + 1.
    /// N, Z, V affected. C not affected.
    #[inline]
    pub(crate) fn perform_inc(&mut self, val: u8) -> u8 {
        let overflow = val == 0x7F;
        let result = val.wrapping_add(1);
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        result
    }

    /// Decrement: result = val - 1.
    /// N, Z, V affected. C not affected.
    #[inline]
    pub(crate) fn perform_dec(&mut self, val: u8) -> u8 {
        let overflow = val == 0x80;
        let result = val.wrapping_sub(1);
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        result
    }

    /// Test: set flags based on val, no modification.
    /// N, Z affected. V cleared.
    #[inline]
    pub(crate) fn perform_tst(&mut self, val: u8) {
        self.set_flags_logical(val);
    }

    // --- Inherent register ops (2 cycles: 1 fetch + 1 internal) ---

    /// NEGA inherent (0x40): Negate A (A = 0 - A, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80. C set if A was non-zero.
    pub(crate) fn op_nega(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_neg(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// NEGB inherent (0x50): Negate B (B = 0 - B, two's complement).
    /// Same flags as NEGA.
    pub(crate) fn op_negb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_neg(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// COMA inherent (0x43): Complement A (A = ~A).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_coma(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_com(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// COMB inherent (0x53): Complement B (B = ~B).
    /// Same flags as COMA.
    pub(crate) fn op_comb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_com(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRA inherent (0x4F): Clear A (A = 0).
    /// N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// CLRB inherent (0x5F): Clear B (B = 0).
    /// N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// INCA inherent (0x4C): Increment A (A = A + 1).
    /// N, Z, V affected. C not affected.
    pub(crate) fn op_inca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_inc(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// INCB inherent (0x5C): Increment B (B = B + 1).
    /// N, Z, V affected. C not affected.
    pub(crate) fn op_incb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_inc(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// DECA inherent (0x4A): Decrement A (A = A - 1).
    /// N, Z, V affected. C not affected.
    pub(crate) fn op_deca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_dec(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// DECB inherent (0x5A): Decrement B (B = B - 1).
    /// N, Z, V affected. C not affected.
    pub(crate) fn op_decb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_dec(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTA inherent (0x4D): Test A (set flags based on A, no modification).
    /// N, Z affected. V cleared.
    pub(crate) fn op_tsta(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTB inherent (0x5D): Test B (set flags based on B, no modification).
    /// N, Z affected. V cleared.
    pub(crate) fn op_tstb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.b);
            self.state = ExecState::Fetch;
        }
    }
}
