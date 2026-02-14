use crate::cpu::m6800::{CcFlag, ExecState, M6800};

impl M6800 {
    // --- Internal Shift/Rotate Helpers ---

    /// ASL (Arithmetic Shift Left): bit 7 → C, bits shift left, 0 → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    #[inline]
    pub(crate) fn perform_asl(&mut self, val: u8) -> u8 {
        let carry = val & 0x80 != 0;
        let result = val << 1;
        self.set_flags_shift(result, carry);
        result
    }

    /// ASR (Arithmetic Shift Right): bit 7 preserved, bits shift right, bit 0 → C.
    /// N, Z, V, C affected. V = N XOR C.
    #[inline]
    pub(crate) fn perform_asr(&mut self, val: u8) -> u8 {
        let carry = val & 0x01 != 0;
        let result = ((val as i8) >> 1) as u8;
        self.set_flags_shift(result, carry);
        result
    }

    /// LSR (Logical Shift Right): 0 → bit 7, bits shift right, bit 0 → C.
    /// N cleared, Z, V, C affected. V = N XOR C = C (since N=0).
    #[inline]
    pub(crate) fn perform_lsr(&mut self, val: u8) -> u8 {
        let carry = val & 0x01 != 0;
        let result = val >> 1;
        self.set_flags_shift(result, carry);
        result
    }

    /// ROL (Rotate Left through Carry): bit 7 → C, bits shift left, old C → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    #[inline]
    pub(crate) fn perform_rol(&mut self, val: u8) -> u8 {
        let old_carry = self.cc & (CcFlag::C as u8) != 0;
        let new_carry = val & 0x80 != 0;
        let result = (val << 1) | (old_carry as u8);
        self.set_flags_shift(result, new_carry);
        result
    }

    /// ROR (Rotate Right through Carry): bit 0 → C, bits shift right, old C → bit 7.
    /// N, Z, V, C affected. V = N XOR C.
    #[inline]
    pub(crate) fn perform_ror(&mut self, val: u8) -> u8 {
        let old_carry = self.cc & (CcFlag::C as u8) != 0;
        let new_carry = val & 0x01 != 0;
        let result = (val >> 1) | ((old_carry as u8) << 7);
        self.set_flags_shift(result, new_carry);
        result
    }

    // --- Inherent register ops (2 cycles: 1 fetch + 1 internal) ---

    /// ASLA inherent (0x48): Arithmetic Shift Left A.
    /// Bit 7 → C, bits shift left, 0 → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_asla(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_asl(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ASLB inherent (0x58): Arithmetic Shift Left B.
    /// Bit 7 → C, bits shift left, 0 → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_aslb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_asl(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRA inherent (0x47): Arithmetic Shift Right A.
    /// Bit 7 preserved, bits shift right, bit 0 → C.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_asra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_asr(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRB inherent (0x57): Arithmetic Shift Right B.
    /// Bit 7 preserved, bits shift right, bit 0 → C.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_asrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_asr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRA inherent (0x44): Logical Shift Right A.
    /// 0 → bit 7, bits shift right, bit 0 → C.
    /// N cleared, Z, V, C affected. V = C (since N=0).
    pub(crate) fn op_lsra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_lsr(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRB inherent (0x54): Logical Shift Right B.
    /// 0 → bit 7, bits shift right, bit 0 → C.
    /// N cleared, Z, V, C affected. V = C (since N=0).
    pub(crate) fn op_lsrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_lsr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLA inherent (0x49): Rotate Left A through Carry.
    /// Bit 7 → C, bits shift left, old C → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_rola(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_rol(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLB inherent (0x59): Rotate Left B through Carry.
    /// Bit 7 → C, bits shift left, old C → bit 0.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_rolb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_rol(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// RORA inherent (0x46): Rotate Right A through Carry.
    /// Bit 0 → C, bits shift right, old C → bit 7.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_rora(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_ror(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// RORB inherent (0x56): Rotate Right B through Carry.
    /// Bit 0 → C, bits shift right, old C → bit 7.
    /// N, Z, V, C affected. V = N XOR C.
    pub(crate) fn op_rorb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_ror(self.b);
            self.state = ExecState::Fetch;
        }
    }
}
