use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, Flag, IndexMode, Z80};

impl Z80 {
    // --- Flag Helpers ---

    fn get_parity(val: u8) -> bool {
        val.count_ones() % 2 == 0
    }

    fn update_flags_logic(&mut self, result: u8, is_and: bool) {
        let mut f = 0;
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if Self::get_parity(result) { f |= Flag::PV as u8; }
        if is_and { f |= Flag::H as u8; } // AND sets H, others clear it
        // N is 0, C is 0

        // Undocumented X/Y
        f |= result & (Flag::X as u8 | Flag::Y as u8);
        self.f = f;
    }

    fn do_add(&mut self, val: u8, carry_in: bool) {
        let a = self.a;
        let c_val = if carry_in && (self.f & Flag::C as u8) != 0 { 1 } else { 0 };
        let result_u16 = (a as u16) + (val as u16) + (c_val as u16);
        let result = result_u16 as u8;

        let mut f = 0;
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if ((a & 0xF) + (val & 0xF) + (c_val as u8)) > 0xF { f |= Flag::H as u8; }
        if ((a ^ result) & (val ^ result) & 0x80) != 0 { f |= Flag::PV as u8; }
        if result_u16 > 0xFF { f |= Flag::C as u8; }

        f |= result & (Flag::X as u8 | Flag::Y as u8);
        self.a = result;
        self.f = f;
    }

    fn do_sub(&mut self, val: u8, carry_in: bool) {
        let a = self.a;
        let c_val = if carry_in && (self.f & Flag::C as u8) != 0 { 1 } else { 0 };
        let result_u16 = (a as u16).wrapping_sub(val as u16).wrapping_sub(c_val as u16);
        let result = result_u16 as u8;

        let mut f = Flag::N as u8;
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if (a & 0xF) < ((val & 0xF) + (c_val as u8)) { f |= Flag::H as u8; }
        if ((a ^ val) & (a ^ result) & 0x80) != 0 { f |= Flag::PV as u8; }
        if result_u16 > 0xFF { f |= Flag::C as u8; }

        f |= result & (Flag::X as u8 | Flag::Y as u8);
        self.a = result;
        self.f = f;
    }

    fn do_cp(&mut self, val: u8) {
        let a = self.a;
        let result_u16 = (a as u16).wrapping_sub(val as u16);
        let result = result_u16 as u8;

        let mut f = Flag::N as u8;
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if (a & 0xF) < (val & 0xF) { f |= Flag::H as u8; }
        if ((a ^ val) & (a ^ result) & 0x80) != 0 { f |= Flag::PV as u8; }
        if result_u16 > 0xFF { f |= Flag::C as u8; }

        // X/Y come from the operand for CP, not the result
        f |= val & (Flag::X as u8 | Flag::Y as u8);
        self.f = f;
    }

    fn perform_alu_op(&mut self, op: u8, val: u8) {
        match op {
            0 => self.do_add(val, false), // ADD
            1 => self.do_add(val, true),  // ADC
            2 => self.do_sub(val, false), // SUB
            3 => self.do_sub(val, true),  // SBC
            4 => { self.a &= val; self.update_flags_logic(self.a, true); }, // AND
            5 => { self.a ^= val; self.update_flags_logic(self.a, false); }, // XOR
            6 => { self.a |= val; self.update_flags_logic(self.a, false); }, // OR
            7 => self.do_cp(val),         // CP
            _ => unreachable!(),
        }
    }

    // --- Instructions ---

    /// ALU A, r — 4 T (reg) or 7 T ((HL))
    /// ADD, ADC, SUB, SBC, AND, XOR, OR, CP
    /// Opcode mask: 10 xxx zzz
    pub fn op_alu_r<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let alu_op = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;

        if r == 6 {
            // ALU A, (HL) — 7 T: M1(4) + MR(3)
            // cycles 1=T4, 2=MR read, 3=MR pad, 4=done
            match cycle {
                1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    if self.index_mode != IndexMode::HL {
                        todo!("ALU A, (IX/IY+d)");
                    }
                    let addr = self.get_hl();
                    let val = bus.read(master, addr);
                    self.perform_alu_op(alu_op, val);
                    self.state = ExecState::Execute(opcode, 3);
                }
                4 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        } else {
            // ALU A, r — 4 T: M1 only
            let val = self.get_reg8_ix(r);
            self.perform_alu_op(alu_op, val);
            self.state = ExecState::Fetch;
        }
    }

    /// ALU A, n — 7 T: M1(4) + MR(3)
    /// Opcode mask: 11 xxx 110
    pub fn op_alu_n<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let alu_op = (opcode >> 3) & 0x07;

        // cycles 1=T4, 2=MR read imm, 3=MR pad, 4=done
        match cycle {
            1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                let val = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.perform_alu_op(alu_op, val);
                self.state = ExecState::Execute(opcode, 3);
            }
            4 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// INC/DEC r — 4 T (reg) or 11 T ((HL))
    /// Opcode mask: 00 rrr 10x
    pub fn op_inc_dec_r<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let r = (opcode >> 3) & 0x07;
        let is_dec = (opcode & 0x01) != 0;

        if r == 6 {
            // INC/DEC (HL) — 11 T: M1(4) + MR(3) + internal(1) + MW(3)
            // cycles 1=T4, 2=MR read, 3-4=MR pad, 5=internal compute,
            //         6=MW write, 7-8=MW pad
            match cycle {
                1 | 3 | 4 | 7 => self.state = ExecState::Execute(opcode, cycle + 1),
                2 => {
                    if self.index_mode != IndexMode::HL {
                        todo!("INC/DEC (IX/IY+d)");
                    }
                    let addr = self.get_hl();
                    self.temp_data = bus.read(master, addr);
                    self.temp_addr = addr;
                    self.state = ExecState::Execute(opcode, 3);
                }
                5 => {
                    // Internal cycle: compute result
                    self.temp_data = if is_dec {
                        self.calc_dec_flags(self.temp_data)
                    } else {
                        self.calc_inc_flags(self.temp_data)
                    };
                    self.state = ExecState::Execute(opcode, 6);
                }
                6 => {
                    bus.write(master, self.temp_addr, self.temp_data);
                    self.state = ExecState::Execute(opcode, 7);
                }
                8 => self.state = ExecState::Fetch,
                _ => unreachable!(),
            }
        } else {
            // INC/DEC r — 4 T: M1 only
            let val = self.get_reg8_ix(r);
            let result = if is_dec {
                self.calc_dec_flags(val)
            } else {
                self.calc_inc_flags(val)
            };
            self.set_reg8_ix(r, result);
            self.state = ExecState::Fetch;
        }
    }

    fn calc_inc_flags(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        let mut f = self.f & Flag::C as u8; // Preserve C
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if (val & 0xF) == 0xF { f |= Flag::H as u8; }
        if val == 0x7F { f |= Flag::PV as u8; } // Overflow 7F -> 80
        // N is 0
        f |= result & (Flag::X as u8 | Flag::Y as u8);
        self.f = f;
        result
    }

    fn calc_dec_flags(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        let mut f = (self.f & Flag::C as u8) | Flag::N as u8; // Preserve C, Set N
        if result == 0 { f |= Flag::Z as u8; }
        if (result & 0x80) != 0 { f |= Flag::S as u8; }
        if (val & 0xF) == 0x0 { f |= Flag::H as u8; } // Borrow from bit 4
        if val == 0x80 { f |= Flag::PV as u8; } // Overflow 80 -> 7F
        f |= result & (Flag::X as u8 | Flag::Y as u8);
        self.f = f;
        result
    }
}
