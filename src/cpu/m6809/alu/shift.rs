use crate::cpu::m6809::{CcFlag, ExecState, M6809};

impl M6809 {
    /// ASLA/LSLA inherent (0x48): Arithmetic/Logical Shift Left A.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_asla(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x80 != 0;
            self.a = self.a << 1;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASLB/LSLB inherent (0x58): Arithmetic/Logical Shift Left B.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_aslb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x80 != 0;
            self.b = self.b << 1;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRA inherent (0x47): Arithmetic Shift Right A.
    /// Shifts all bits right one position. Bit 7 is preserved (sign extension).
    /// Bit 0 goes to C.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 0.
    pub(crate) fn op_asra(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x01 != 0;
            self.a = ((self.a as i8) >> 1) as u8;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRB inherent (0x57): Arithmetic Shift Right B.
    /// Shifts all bits right one position. Bit 7 is preserved (sign extension).
    /// Bit 0 goes to C.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 0.
    pub(crate) fn op_asrb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x01 != 0;
            self.b = ((self.b as i8) >> 1) as u8;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRA inherent (0x44): Logical Shift Right A.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsra(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x01 != 0;
            self.a = self.a >> 1;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRB inherent (0x54): Logical Shift Right B.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsrb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x01 != 0;
            self.b = self.b >> 1;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLA inherent (0x49): Rotate Left A through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rola(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.a & 0x80 != 0;
            self.a = (self.a << 1) | (old_carry as u8);
            self.set_flags_shift(self.a, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLB inherent (0x59): Rotate Left B through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rolb(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.b & 0x80 != 0;
            self.b = (self.b << 1) | (old_carry as u8);
            self.set_flags_shift(self.b, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// RORA inherent (0x46): Rotate Right A through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rora(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.a & 0x01 != 0;
            self.a = (self.a >> 1) | ((old_carry as u8) << 7);
            self.set_flags_shift(self.a, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// RORB inherent (0x56): Rotate Right B through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rorb(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.b & 0x01 != 0;
            self.b = (self.b >> 1) | ((old_carry as u8) << 7);
            self.set_flags_shift(self.b, new_carry);
            self.state = ExecState::Fetch;
        }
    }
}
