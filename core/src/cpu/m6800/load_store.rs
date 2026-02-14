use crate::core::{Bus, BusMaster};
use crate::cpu::m6800::{CcFlag, ExecState, M6800};

impl M6800 {
    // --- 8-bit load immediate (2 cycles: 1 fetch + 1 read operand & execute) ---

    /// LDAA immediate (0x86): Load A with immediate value.
    /// N, Z affected. V cleared.
    pub(crate) fn op_ldaa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_flags_logical(op);
        });
    }

    /// LDAB immediate (0xC6): Load B with immediate value.
    /// N, Z affected. V cleared.
    pub(crate) fn op_ldab_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.b = op;
            cpu.set_flags_logical(op);
        });
    }

    // --- 16-bit immediate ops (3 cycles: 1 fetch + 1 read hi + 1 read lo & execute) ---

    /// CPX immediate (0x8C): Compare X with 16-bit immediate (X - M:M+1).
    /// N, Z, V affected. C not affected (6800-specific: unlike 6809 CMPX).
    /// 3 cycles total: 1 fetch + 2 execute.
    pub(crate) fn op_cpx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
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
                let (result, _borrow) = self.x.overflowing_sub(operand);
                let overflow =
                    (self.x ^ operand) & 0x8000 != 0 && (self.x ^ result) & 0x8000 != 0;
                self.set_flag(CcFlag::N, result & 0x8000 != 0);
                self.set_flag(CcFlag::Z, result == 0);
                self.set_flag(CcFlag::V, overflow);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// LDS immediate (0x8E): Load SP with 16-bit immediate.
    /// N, Z affected. V cleared.
    /// 3 cycles total: 1 fetch + 2 execute.
    pub(crate) fn op_lds_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
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
    /// 3 cycles total: 1 fetch + 2 execute.
    pub(crate) fn op_ldx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
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
}
