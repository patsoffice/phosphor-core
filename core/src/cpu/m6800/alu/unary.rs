use crate::core::{Bus, BusMaster};
use crate::cpu::m68xx::M68xxAlu;
use crate::cpu::m6800::{ExecState, M6800};

impl M6800 {
    // --- Inherent register ops (2 cycles: 1 fetch + 1 internal) ---

    /// NEGA inherent (0x40): Negate A (A = 0 - A, two's complement).
    pub(crate) fn op_nega(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_neg(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// NEGB inherent (0x50): Negate B (B = 0 - B, two's complement).
    pub(crate) fn op_negb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_neg(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// COMA inherent (0x43): Complement A (A = ~A).
    pub(crate) fn op_coma(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_com(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// COMB inherent (0x53): Complement B (B = ~B).
    pub(crate) fn op_comb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_com(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRA inherent (0x4F): Clear A (A = 0).
    pub(crate) fn op_clra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// CLRB inherent (0x5F): Clear B (B = 0).
    pub(crate) fn op_clrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_clr();
            self.state = ExecState::Fetch;
        }
    }

    /// INCA inherent (0x4C): Increment A (A = A + 1).
    pub(crate) fn op_inca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_inc(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// INCB inherent (0x5C): Increment B (B = B + 1).
    pub(crate) fn op_incb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_inc(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// DECA inherent (0x4A): Decrement A (A = A - 1).
    pub(crate) fn op_deca(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = self.perform_dec(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// DECB inherent (0x5A): Decrement B (B = B - 1).
    pub(crate) fn op_decb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = self.perform_dec(self.b);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTA inherent (0x4D): Test A (set flags based on A, no modification).
    pub(crate) fn op_tsta(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTB inherent (0x5D): Test B (set flags based on B, no modification).
    pub(crate) fn op_tstb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.perform_tst(self.b);
            self.state = ExecState::Fetch;
        }
    }

    // --- Memory unary ops: indexed (7 cycles) and extended (6 cycles) ---

    /// NEG indexed (0x60).
    pub(crate) fn op_neg_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_neg(val));
    }

    /// NEG extended (0x70).
    pub(crate) fn op_neg_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_neg(val));
    }

    /// COM indexed (0x63).
    pub(crate) fn op_com_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_com(val));
    }

    /// COM extended (0x73).
    pub(crate) fn op_com_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_com(val));
    }

    /// INC indexed (0x6C).
    pub(crate) fn op_inc_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_inc(val));
    }

    /// INC extended (0x7C).
    pub(crate) fn op_inc_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_inc(val));
    }

    /// DEC indexed (0x6A).
    pub(crate) fn op_dec_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| cpu.perform_dec(val));
    }

    /// DEC extended (0x7A).
    pub(crate) fn op_dec_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| cpu.perform_dec(val));
    }

    /// TST indexed (0x6D).
    pub(crate) fn op_tst_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, val| {
            cpu.perform_tst(val);
            val
        });
    }

    /// TST extended (0x7D).
    pub(crate) fn op_tst_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, val| {
            cpu.perform_tst(val);
            val
        });
    }

    /// CLR indexed (0x6F).
    pub(crate) fn op_clr_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_indexed(cycle, bus, master, |cpu, _val| cpu.perform_clr());
    }

    /// CLR extended (0x7F).
    pub(crate) fn op_clr_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_extended(cycle, bus, master, |cpu, _val| cpu.perform_clr());
    }
}
