use crate::core::{Bus, BusMaster};
use crate::cpu::m68xx::M68xxAlu;
use crate::cpu::m6800::{ExecState, M6800};

impl M6800 {
    // --- Inherent register ops (2 cycles: 1 fetch + 1 internal) ---

    /// ASLA inherent (0x48): Arithmetic Shift Left A.
    pub(crate) fn op_asla(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_asl(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ASLB inherent (0x58): Arithmetic Shift Left B.
    pub(crate) fn op_aslb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_asl(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRA inherent (0x47): Arithmetic Shift Right A.
    pub(crate) fn op_asra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_asr(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRB inherent (0x57): Arithmetic Shift Right B.
    pub(crate) fn op_asrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_asr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRA inherent (0x44): Logical Shift Right A.
    pub(crate) fn op_lsra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_lsr(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRB inherent (0x54): Logical Shift Right B.
    pub(crate) fn op_lsrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_lsr(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLA inherent (0x49): Rotate Left A through Carry.
    pub(crate) fn op_rola(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_rol(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLB inherent (0x59): Rotate Left B through Carry.
    pub(crate) fn op_rolb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_rol(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// RORA inherent (0x46): Rotate Right A through Carry.
    pub(crate) fn op_rora(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_ror(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// RORB inherent (0x56): Rotate Right B through Carry.
    pub(crate) fn op_rorb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_ror(self.b);
            self.state = ExecState::Fetch;
        }
    }

    // --- Memory shift/rotate ops: indexed (7 cycles) and extended (6 cycles) ---

    /// ASL indexed (0x68).
    pub(crate) fn op_asl_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ASL extended (0x78).
    pub(crate) fn op_asl_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ASR indexed (0x67).
    pub(crate) fn op_asr_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_asr(val));
    }

    /// ASR extended (0x77).
    pub(crate) fn op_asr_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_asr(val));
    }

    /// LSR indexed (0x64).
    pub(crate) fn op_lsr_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// LSR extended (0x74).
    pub(crate) fn op_lsr_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// ROL indexed (0x69).
    pub(crate) fn op_rol_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    /// ROL extended (0x79).
    pub(crate) fn op_rol_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    /// ROR indexed (0x66).
    pub(crate) fn op_ror_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }

    /// ROR extended (0x76).
    pub(crate) fn op_ror_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }
}
