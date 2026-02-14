use crate::core::{Bus, BusMaster};
use crate::cpu::m6800::{CcFlag, ExecState, M6800};

impl M6800 {
    // ---- 8-bit Load immediate ----

    /// LDAA immediate (0x86): Load A with immediate value.
    /// N, Z affected. V cleared.
    pub(crate) fn op_ldaa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAB immediate (0xC6): Load B with immediate value.
    /// N, Z affected. V cleared.
    pub(crate) fn op_ldab_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.b = op;
            cpu.set_flags_logical(op);
        });
    }

    // ---- 8-bit Load direct/indexed/extended ----

    /// LDAA direct (0x96). N, Z affected. V cleared.
    pub(crate) fn op_ldaa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAA indexed (0xA6). N, Z affected. V cleared.
    pub(crate) fn op_ldaa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAA extended (0xB6). N, Z affected. V cleared.
    pub(crate) fn op_ldaa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAB direct (0xD6). N, Z affected. V cleared.
    pub(crate) fn op_ldab_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| {
            cpu.b = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAB indexed (0xE6). N, Z affected. V cleared.
    pub(crate) fn op_ldab_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| {
            cpu.b = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAB extended (0xF6). N, Z affected. V cleared.
    pub(crate) fn op_ldab_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| {
            cpu.b = op;
            cpu.set_flags_logical(op);
        });
    }

    // ---- 8-bit Store direct/indexed/extended ----

    /// STAA direct (0x97). N, Z affected. V cleared.
    pub(crate) fn op_staa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.a); }
        self.store_direct(cycle, bus, master, self.a);
    }

    /// STAA indexed (0xA7). N, Z affected. V cleared.
    pub(crate) fn op_staa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.a); }
        self.store_indexed(cycle, bus, master, self.a);
    }

    /// STAA extended (0xB7). N, Z affected. V cleared.
    pub(crate) fn op_staa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.a); }
        self.store_extended(cycle, bus, master, self.a);
    }

    /// STAB direct (0xD7). N, Z affected. V cleared.
    pub(crate) fn op_stab_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.b); }
        self.store_direct(cycle, bus, master, self.b);
    }

    /// STAB indexed (0xE7). N, Z affected. V cleared.
    pub(crate) fn op_stab_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.b); }
        self.store_indexed(cycle, bus, master, self.b);
    }

    /// STAB extended (0xF7). N, Z affected. V cleared.
    pub(crate) fn op_stab_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical(self.b); }
        self.store_extended(cycle, bus, master, self.b);
    }

    // ---- 16-bit immediate ops ----

    /// CPX immediate (0x8C): Compare X with 16-bit immediate (X - M:M+1).
    /// N, Z, V affected. C not affected (6800-specific: unlike 6809 CMPX).
    pub(crate) fn op_cpx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let operand = self.temp_addr | low;
                self.perform_cpx(operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// LDS immediate (0x8E): Load SP with 16-bit immediate.
    /// N, Z affected. V cleared.
    pub(crate) fn op_lds_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.sp = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// LDX immediate (0xCE): Load X with 16-bit immediate.
    /// N, Z affected. V cleared.
    pub(crate) fn op_ldx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.x = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- 16-bit Load direct/indexed/extended ----

    /// LDS direct (0x9E). N, Z affected. V cleared.
    pub(crate) fn op_lds_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_direct(cycle, bus, master, |cpu, val| {
            cpu.sp = val;
            cpu.set_flags_logical16(val);
        });
    }

    /// LDS indexed (0xAE). N, Z affected. V cleared.
    pub(crate) fn op_lds_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_indexed(cycle, bus, master, |cpu, val| {
            cpu.sp = val;
            cpu.set_flags_logical16(val);
        });
    }

    /// LDS extended (0xBE). N, Z affected. V cleared.
    pub(crate) fn op_lds_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_extended(cycle, bus, master, |cpu, val| {
            cpu.sp = val;
            cpu.set_flags_logical16(val);
        });
    }

    /// LDX direct (0xDE). N, Z affected. V cleared.
    pub(crate) fn op_ldx_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_direct(cycle, bus, master, |cpu, val| {
            cpu.x = val;
            cpu.set_flags_logical16(val);
        });
    }

    /// LDX indexed (0xEE). N, Z affected. V cleared.
    pub(crate) fn op_ldx_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_indexed(cycle, bus, master, |cpu, val| {
            cpu.x = val;
            cpu.set_flags_logical16(val);
        });
    }

    /// LDX extended (0xFE). N, Z affected. V cleared.
    pub(crate) fn op_ldx_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_extended(cycle, bus, master, |cpu, val| {
            cpu.x = val;
            cpu.set_flags_logical16(val);
        });
    }

    // ---- 16-bit Store direct/indexed/extended ----

    /// STS direct (0x9F). N, Z affected. V cleared.
    pub(crate) fn op_sts_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.sp); }
        self.store16_direct(cycle, bus, master, self.sp);
    }

    /// STS indexed (0xAF). N, Z affected. V cleared.
    pub(crate) fn op_sts_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.sp); }
        self.store16_indexed(cycle, bus, master, self.sp);
    }

    /// STS extended (0xBF). N, Z affected. V cleared.
    pub(crate) fn op_sts_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.sp); }
        self.store16_extended(cycle, bus, master, self.sp);
    }

    /// STX direct (0xDF). N, Z affected. V cleared.
    pub(crate) fn op_stx_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.x); }
        self.store16_direct(cycle, bus, master, self.x);
    }

    /// STX indexed (0xEF). N, Z affected. V cleared.
    pub(crate) fn op_stx_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.x); }
        self.store16_indexed(cycle, bus, master, self.x);
    }

    /// STX extended (0xFF). N, Z affected. V cleared.
    pub(crate) fn op_stx_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        if cycle == 0 { self.set_flags_logical16(self.x); }
        self.store16_extended(cycle, bus, master, self.x);
    }

    // ---- CPX direct/indexed/extended ----

    /// CPX direct (0x9C). N, Z, V affected. C not affected.
    pub(crate) fn op_cpx_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_direct(cycle, bus, master, |cpu, val| cpu.perform_cpx(val));
    }

    /// CPX indexed (0xAC). N, Z, V affected. C not affected.
    pub(crate) fn op_cpx_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_indexed(cycle, bus, master, |cpu, val| cpu.perform_cpx(val));
    }

    /// CPX extended (0xBC). N, Z, V affected. C not affected.
    pub(crate) fn op_cpx_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        self.alu16_extended(cycle, bus, master, |cpu, val| cpu.perform_cpx(val));
    }

    // ---- CPX helper ----

    /// CPX: Compare X register with 16-bit operand (X - operand).
    /// Sets N, Z, V. C not affected (6800-specific).
    #[inline]
    pub(crate) fn perform_cpx(&mut self, operand: u16) {
        let (result, _borrow) = self.x.overflowing_sub(operand);
        let overflow =
            (self.x ^ operand) & 0x8000 != 0 && (self.x ^ result) & 0x8000 != 0;
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
    }
}
