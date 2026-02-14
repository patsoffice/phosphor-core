use super::{ExecState, M6502, StatusFlag};
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- Branch helper ----

    /// Generic conditional branch. Timing:
    /// - Not taken: 2 cycles (fetch + cycle 0)
    /// - Taken, no page cross: 3 cycles
    /// - Taken, page cross: 4 cycles
    fn branch<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        condition: bool,
    ) {
        match cycle {
            0 => {
                let offset = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                if !condition {
                    // Not taken — 2 cycles total
                    self.state = ExecState::Fetch;
                } else {
                    // Taken — compute target, check page cross on next cycle
                    self.temp_addr = self.pc.wrapping_add(offset as i8 as u16);
                    self.state = ExecState::Execute(self.opcode, 1);
                }
            }
            1 => {
                if (self.pc ^ self.temp_addr) & 0xFF00 != 0 {
                    // Page cross — need extra cycle
                    self.pc = self.temp_addr;
                    self.state = ExecState::Execute(self.opcode, 2);
                } else {
                    // No page cross — done
                    self.pc = self.temp_addr;
                    self.state = ExecState::Fetch;
                }
            }
            2 => {
                // Extra cycle for page crossing fix-up
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- Branch instructions ----

    /// BPL (0x10) - Branch if Plus (N=0)
    pub(crate) fn op_bpl<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::N as u8) == 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BMI (0x30) - Branch if Minus (N=1)
    pub(crate) fn op_bmi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::N as u8) != 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BVC (0x50) - Branch if Overflow Clear (V=0)
    pub(crate) fn op_bvc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::V as u8) == 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BVS (0x70) - Branch if Overflow Set (V=1)
    pub(crate) fn op_bvs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::V as u8) != 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BCC (0x90) - Branch if Carry Clear (C=0)
    pub(crate) fn op_bcc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::C as u8) == 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BCS (0xB0) - Branch if Carry Set (C=1)
    pub(crate) fn op_bcs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::C as u8) != 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BNE (0xD0) - Branch if Not Equal (Z=0)
    pub(crate) fn op_bne<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::Z as u8) == 0;
        self.branch(cycle, bus, master, condition);
    }

    /// BEQ (0xF0) - Branch if Equal (Z=1)
    pub(crate) fn op_beq<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let condition = self.p & (StatusFlag::Z as u8) != 0;
        self.branch(cycle, bus, master, condition);
    }

    // ---- Jump instructions ----

    /// JMP Absolute (0x4C) - 3 cycles
    pub(crate) fn op_jmp_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr |= (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.temp_addr;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// JMP Indirect (0x6C) - 5 cycles
    /// NMOS bug: if pointer is at $xxFF, high byte is fetched from $xx00 (not $xx00+$100).
    pub(crate) fn op_jmp_ind<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr |= (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Read target low byte from pointer address
                self.pc = bus.read(master, self.temp_addr) as u16;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // NMOS page-wrap bug: high byte wraps within same page
                let hi_addr = (self.temp_addr & 0xFF00) | (self.temp_addr.wrapping_add(1) & 0x00FF);
                self.pc |= (bus.read(master, hi_addr) as u16) << 8;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// JSR (0x20) - 6 cycles
    /// Pushes address of last byte of JSR instruction (return addr - 1). RTS adds 1.
    pub(crate) fn op_jsr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Read target address low byte
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal cycle
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Push PCH (PC points to last byte of JSR instruction)
                bus.write(master, 0x0100 | self.sp as u16, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Push PCL
                bus.write(master, 0x0100 | self.sp as u16, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                // Read target address high byte, set PC
                self.temp_addr |= (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.temp_addr;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// RTS (0x60) - 6 cycles
    /// Pulls address from stack and adds 1.
    pub(crate) fn op_rts<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Internal: read and discard
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal: increment SP
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Pull PCL from stack
                self.pc = bus.read(master, 0x0100 | self.sp as u16) as u16;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Pull PCH from stack
                self.pc |= (bus.read(master, 0x0100 | self.sp as u16) as u16) << 8;
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                // Increment PC (+1 to get past the last byte of JSR)
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// RTI (0x40) - 6 cycles
    /// Pulls P then PC from stack. No +1 adjustment (unlike RTS).
    pub(crate) fn op_rti<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Internal: read and discard
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal: increment SP
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Pull P from stack (B always clear, U always set)
                let pulled = bus.read(master, 0x0100 | self.sp as u16);
                self.p = (pulled | StatusFlag::U as u8) & !(StatusFlag::B as u8);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Pull PCL from stack
                self.pc = bus.read(master, 0x0100 | self.sp as u16) as u16;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                // Pull PCH from stack
                self.pc |= (bus.read(master, 0x0100 | self.sp as u16) as u16) << 8;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
