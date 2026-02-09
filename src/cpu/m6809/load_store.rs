use super::{CcFlag, ExecState, M6809};
use crate::core::{Bus, BusMaster};

impl M6809 {
    /// LDA immediate (0x86)
    pub(crate) fn op_lda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.a = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.set_flag(CcFlag::N, self.a & 0x80 != 0);
                self.set_flag(CcFlag::Z, self.a == 0);
                self.set_flag(CcFlag::V, false);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDB immediate (0xC6)
    pub(crate) fn op_ldb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.b = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.set_flag(CcFlag::N, self.b & 0x80 != 0);
                self.set_flag(CcFlag::Z, self.b == 0);
                self.set_flag(CcFlag::V, false);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDA direct (0x96): Load A from memory at DP:addr.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_lda_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                self.a = bus.read(master, self.temp_addr);
                self.set_flags_logical(self.a);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STA direct (0x97): Store A to memory at DP:addr.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_sta_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, self.a);
                self.set_flag(CcFlag::N, self.a & 0x80 != 0);
                self.set_flag(CcFlag::Z, self.a == 0);
                self.set_flag(CcFlag::V, false);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDB direct (0xD6): Load B from memory at DP:addr.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldb_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                self.b = bus.read(master, self.temp_addr);
                self.set_flags_logical(self.b);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STB direct (0xD7): Store B to memory at DP:addr.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_stb_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, self.b);
                self.set_flag(CcFlag::N, self.b & 0x80 != 0);
                self.set_flag(CcFlag::Z, self.b == 0);
                self.set_flag(CcFlag::V, false);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDD direct (0xDC): Load D (A:B) from memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldd_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let high = bus.read(master, self.temp_addr) as u16;
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.a = high as u8;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                self.b = bus.read(master, self.temp_addr);
                let val = self.get_d();
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STD direct (0xDD): Store D (A:B) to memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_std_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, self.a);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, self.b);
                let val = self.get_d();
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDX direct (0x9E): Load X from memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldx_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let high = bus.read(master, self.temp_addr) as u16;
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.x = high << 8;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let low = bus.read(master, self.temp_addr) as u16;
                self.x |= low;
                self.set_flags_logical16(self.x);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STX direct (0x9F): Store X to memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_stx_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, (self.x >> 8) as u8);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, self.x as u8);
                self.set_flags_logical16(self.x);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDU direct (0xDE): Load U from memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldu_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let high = bus.read(master, self.temp_addr) as u16;
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.u = high << 8;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let low = bus.read(master, self.temp_addr) as u16;
                self.u |= low;
                self.set_flags_logical16(self.u);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STU direct (0xDF): Store U to memory at DP:addr (16-bit).
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_stu_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, (self.u >> 8) as u8);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, self.u as u8);
                self.set_flags_logical16(self.u);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }
}
