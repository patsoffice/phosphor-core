use crate::core::{Bus, BusMaster};
use crate::cpu::m6809::{ExecState, M6809};

impl M6809 {
    // --- Internal 16-bit ALU Helpers ---

    #[inline]
    fn perform_addd(&mut self, operand: u16) {
        let d = self.get_d();
        let (result, carry) = d.overflowing_add(operand);
        let overflow = (d ^ operand) & 0x8000 == 0 && (d ^ result) & 0x8000 != 0;
        self.set_d(result);
        self.set_flags_arithmetic16(result, overflow, carry);
    }

    #[inline]
    fn perform_subd(&mut self, operand: u16) {
        let d = self.get_d();
        let (result, borrow) = d.overflowing_sub(operand);
        let overflow = (d ^ operand) & 0x8000 != 0 && (d ^ result) & 0x8000 != 0;
        self.set_d(result);
        self.set_flags_arithmetic16(result, overflow, borrow);
    }

    #[inline]
    fn perform_cmp16(&mut self, reg_val: u16, operand: u16) {
        let (result, borrow) = reg_val.overflowing_sub(operand);
        let overflow = (reg_val ^ operand) & 0x8000 != 0 && (reg_val ^ result) & 0x8000 != 0;
        self.set_flags_arithmetic16(result, overflow, borrow);
    }

    /// ADDD immediate (0xC3): Adds a 16-bit immediate value to the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned carry out of bit 15.
    pub(crate) fn op_addd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Fetch high byte of operand
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Fetch low byte, execute
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let operand = self.temp_addr | low;
                self.perform_addd(operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SUBD immediate (0x83): Subtracts a 16-bit immediate value from the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_subd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Fetch high byte of operand
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Fetch low byte, execute
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let operand = self.temp_addr | low;
                self.perform_subd(operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// CMPX immediate (0x8C): Compare X with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_cmpx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr |= low;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let operand = self.temp_addr;
                self.perform_cmp16(self.x, operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDD immediate (0xCC): Load D with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.set_d(val);
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDX immediate (0x8E): Load X with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.x = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDU immediate (0xCE): Load U with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldu_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.u = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    // --- Direct addressing mode (16-bit) ---

    /// SUBD direct (0x93): Subtracts the 16-bit value at DP:addr from the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_subd_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
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
                self.opcode = high as u8; // reuse opcode field to store high byte temporarily
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let operand = ((self.opcode as u16) << 8) | low;
                self.perform_subd(operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ADDD direct (0xD3): Adds the 16-bit value at DP:addr to the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned carry out of bit 15.
    pub(crate) fn op_addd_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
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
                self.opcode = high as u8;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let operand = ((self.opcode as u16) << 8) | low;
                self.perform_addd(operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// CMPX direct (0x9C): Compare X with 16-bit value at DP:addr.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_cmpx_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
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
                self.opcode = high as u8;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let operand = ((self.opcode as u16) << 8) | low;
                self.perform_cmp16(self.x, operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }
}
