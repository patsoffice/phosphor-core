use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, IndexMode, Z80};

impl Z80 {
    /// LD r, n
    /// Loads an immediate 8-bit value into a register or memory location (HL).
    /// Opcode mask: 00 rrr 110
    pub fn op_ld_r_n<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let r = (opcode >> 3) & 0x07;

        if r == 6 {
            // LD (HL), n
            match cycle {
                0 => {
                    let n = bus.read(master, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.temp_addr = n as u16; // Store immediate temporarily
                    self.state = ExecState::Execute(opcode, 1);
                }
                1 => {
                    if self.index_mode != IndexMode::HL {
                        // TODO: Implement Index modes (IX/IY + d)
                        todo!("LD (IX/IY+d), n");
                    }
                    let addr = self.get_hl();
                    bus.write(master, addr, self.temp_addr as u8);
                    self.state = ExecState::Fetch;
                }
                _ => unreachable!(),
            }
        } else {
            // LD r, n
            if cycle == 0 {
                let n = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.set_reg8(r, n);
                self.state = ExecState::Fetch;
            }
        }
    }

    /// LD r, r'
    /// Loads a value from one register to another.
    /// Opcode mask: 01 dst src
    pub fn op_ld_r_r<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let src = opcode & 0x07;
        let dst = (opcode >> 3) & 0x07;

        if src == 6 {
            // LD r, (HL)
            if cycle == 0 {
                if self.index_mode != IndexMode::HL {
                    todo!("LD r, (IX/IY+d)");
                }
                let addr = self.get_hl();
                let val = bus.read(master, addr);
                self.set_reg8(dst, val);
                self.state = ExecState::Fetch;
            }
        } else if dst == 6 {
            // LD (HL), r
            if cycle == 0 {
                if self.index_mode != IndexMode::HL {
                    todo!("LD (IX/IY+d), r");
                }
                let val = self.get_reg8(src);
                let addr = self.get_hl();
                bus.write(master, addr, val);
                self.state = ExecState::Fetch;
            }
        } else {
            // LD r, r'
            let val = self.get_reg8(src);
            self.set_reg8(dst, val);
            self.state = ExecState::Fetch;
        }
    }
}