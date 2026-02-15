use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, IndexMode, Z80};

impl Z80 {
    /// LD r, n — 7 T: M1(4) + MR(3)
    /// Loads an immediate 8-bit value into a register or memory location (HL).
    /// Opcode mask: 00 rrr 110
    /// Handler cycles: 1=T4, 2-4=MR (bus read on cycle 2)
    pub fn op_ld_r_n<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let r = (opcode >> 3) & 0x07;

        if r == 6 {
            // LD (HL), n — 10 T: M1(4) + MR(3) + MW(3)
            // cycles 1=T4, 2=MR read imm, 3-4=MR pad, 5=MW write, 6-7=MW pad
            match cycle {
                1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    let n = bus.read(master, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.temp_data = n;
                    self.state = ExecState::Execute(opcode, 3);
                }
                5 => {
                    if self.index_mode != IndexMode::HL {
                        todo!("LD (IX/IY+d), n");
                    }
                    let addr = self.get_hl();
                    bus.write(master, addr, self.temp_data);
                    self.state = ExecState::Execute(opcode, 6);
                }
                7 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        } else {
            // LD r, n — 7 T: M1(4) + MR(3)
            // cycles 1=T4, 2=MR read imm, 3=MR pad, 4=done
            match cycle {
                1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    let n = bus.read(master, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.set_reg8_ix(r, n);
                    self.state = ExecState::Execute(opcode, 3);
                }
                4 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        }
    }

    /// LD r, r' — 4 T: M1 only (register-register)
    /// LD r, (HL) — 7 T: M1(4) + MR(3)
    /// LD (HL), r — 7 T: M1(4) + MW(3)
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
            // LD r, (HL) — 7 T: M1(4) + MR(3)
            // cycles 1=T4, 2=MR read, 3=MR pad, 4=done
            match cycle {
                1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    if self.index_mode != IndexMode::HL {
                        todo!("LD r, (IX/IY+d)");
                    }
                    let addr = self.get_hl();
                    let val = bus.read(master, addr);
                    self.set_reg8(dst, val);
                    self.state = ExecState::Execute(opcode, 3);
                }
                4 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        } else if dst == 6 {
            // LD (HL), r — 7 T: M1(4) + MW(3)
            // cycles 1=T4, 2=MW write, 3=MW pad, 4=done
            match cycle {
                1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    if self.index_mode != IndexMode::HL {
                        todo!("LD (IX/IY+d), r");
                    }
                    let val = self.get_reg8(src);
                    let addr = self.get_hl();
                    bus.write(master, addr, val);
                    self.state = ExecState::Execute(opcode, 3);
                }
                4 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        } else {
            // LD r, r' — 4 T: M1 only
            let val = self.get_reg8_ix(src);
            self.set_reg8_ix(dst, val);
            self.state = ExecState::Fetch;
        }
    }
}
