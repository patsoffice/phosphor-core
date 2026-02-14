use super::{ExecState, M6502, StatusFlag};
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- Stack instructions ----

    /// PHA (0x48) - 3 cycles. Push A to stack.
    pub(crate) fn op_pha<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Dummy read from PC (next byte, discarded)
                let _ = bus.read(master, self.pc);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                bus.write(master, 0x0100 | self.sp as u16, self.a);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PLA (0x68) - 4 cycles. Pull A from stack. Sets N, Z.
    pub(crate) fn op_pla<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Dummy read from PC
                let _ = bus.read(master, self.pc);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Dummy read from stack[SP], then increment SP
                let _ = bus.read(master, 0x0100 | self.sp as u16);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Pull A from stack, set N/Z
                self.a = bus.read(master, 0x0100 | self.sp as u16);
                self.set_nz(self.a);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PHP (0x08) - 3 cycles. Push P with B=1 and U=1 to stack.
    pub(crate) fn op_php<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Dummy read from PC
                let _ = bus.read(master, self.pc);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Push P with B and U bits always set
                let p_push = self.p | StatusFlag::B as u8 | StatusFlag::U as u8;
                bus.write(master, 0x0100 | self.sp as u16, p_push);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PLP (0x28) - 4 cycles. Pull P from stack. B is always clear, U is always set.
    pub(crate) fn op_plp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Dummy read from PC
                let _ = bus.read(master, self.pc);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Dummy read from stack[SP], then increment SP
                let _ = bus.read(master, 0x0100 | self.sp as u16);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Pull P from stack (B always clear, U always set)
                let pulled = bus.read(master, 0x0100 | self.sp as u16);
                self.p = (pulled | StatusFlag::U as u8) & !(StatusFlag::B as u8);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- BRK ----

    /// BRK (0x00) - 7 cycles. Software interrupt.
    /// 2-byte instruction: pushes PC+2 (past opcode + padding byte).
    /// Pushes P with B=1. Vectors through $FFFE/$FFFF. Sets I flag.
    pub(crate) fn op_brk<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Read padding byte, increment PC
                let _ = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Push PCH
                bus.write(master, 0x0100 | self.sp as u16, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Push PCL
                bus.write(master, 0x0100 | self.sp as u16, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Push P with B=1, U=1
                let p_push = self.p | StatusFlag::B as u8 | StatusFlag::U as u8;
                bus.write(master, 0x0100 | self.sp as u16, p_push);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                // Read vector low from $FFFE
                self.pc = bus.read(master, 0xFFFE) as u16;
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                // Read vector high from $FFFF, set I flag
                self.pc |= (bus.read(master, 0xFFFF) as u16) << 8;
                self.set_flag(StatusFlag::I, true);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
