use crate::core::{Bus, BusMaster};
use crate::cpu::m6809::{CcFlag, ExecState, M6809};

impl M6809 {
    // --- Internal Shift/Rotate Helpers ---

    #[inline]
    fn perform_asl(&mut self, val: u8) -> u8 {
        let carry = val & 0x80 != 0;
        let result = val << 1;
        self.set_flags_shift(result, carry);
        result
    }

    #[inline]
    fn perform_asr(&mut self, val: u8) -> u8 {
        let carry = val & 0x01 != 0;
        let result = ((val as i8) >> 1) as u8;
        self.set_flags_shift(result, carry);
        result
    }

    #[inline]
    fn perform_lsr(&mut self, val: u8) -> u8 {
        let carry = val & 0x01 != 0;
        let result = val >> 1;
        self.set_flags_shift(result, carry);
        result
    }

    #[inline]
    fn perform_rol(&mut self, val: u8) -> u8 {
        let old_carry = self.cc & (CcFlag::C as u8) != 0;
        let new_carry = val & 0x80 != 0;
        let result = (val << 1) | (old_carry as u8);
        self.set_flags_shift(result, new_carry);
        result
    }

    #[inline]
    fn perform_ror(&mut self, val: u8) -> u8 {
        let old_carry = self.cc & (CcFlag::C as u8) != 0;
        let new_carry = val & 0x01 != 0;
        let result = (val >> 1) | ((old_carry as u8) << 7);
        self.set_flags_shift(result, new_carry);
        result
    }

    /// ASLA/LSLA inherent (0x48): Arithmetic/Logical Shift Left A.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_asla(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_asl(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ASLB/LSLB inherent (0x58): Arithmetic/Logical Shift Left B.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_aslb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_asl(self.b);
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
            self.a = self.perform_asr(self.a);
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
            self.b = self.perform_asr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRA inherent (0x44): Logical Shift Right A.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_lsr(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRB inherent (0x54): Logical Shift Right B.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_lsr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLA inherent (0x49): Rotate Left A through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rola(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_rol(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLB inherent (0x59): Rotate Left B through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rolb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_rol(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// RORA inherent (0x46): Rotate Right A through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rora(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_ror(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// RORB inherent (0x56): Rotate Right B through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rorb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_ror(self.b);
            self.state = ExecState::Fetch;
        }
    }

    // --- Indexed addressing mode (memory shift ops, 0x64-0x69) ---

    /// LSR indexed (0x64): Logical Shift Right memory byte at indexed EA.
    pub(crate) fn op_lsr_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(opcode, cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// ROR indexed (0x66): Rotate Right memory byte at indexed EA through Carry.
    pub(crate) fn op_ror_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(opcode, cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }

    /// ASR indexed (0x67): Arithmetic Shift Right memory byte at indexed EA.
    pub(crate) fn op_asr_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(opcode, cycle, bus, master, |cpu, val| cpu.perform_asr(val));
    }

    /// ASL indexed (0x68): Arithmetic Shift Left memory byte at indexed EA.
    pub(crate) fn op_asl_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(opcode, cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ROL indexed (0x69): Rotate Left memory byte at indexed EA through Carry.
    pub(crate) fn op_rol_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(opcode, cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }
}
