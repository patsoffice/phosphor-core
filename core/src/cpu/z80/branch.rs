use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, Flag, Z80};

impl Z80 {
    /// Evaluate a condition code (3 bits from opcode bits 5-3).
    /// 0=NZ, 1=Z, 2=NC, 3=C, 4=PO, 5=PE, 6=P, 7=M
    pub(crate) fn eval_condition(&self, cc: u8) -> bool {
        match cc {
            0 => (self.f & Flag::Z as u8) == 0,  // NZ
            1 => (self.f & Flag::Z as u8) != 0,  // Z
            2 => (self.f & Flag::C as u8) == 0,  // NC
            3 => (self.f & Flag::C as u8) != 0,  // C
            4 => (self.f & Flag::PV as u8) == 0, // PO (parity odd)
            5 => (self.f & Flag::PV as u8) != 0, // PE (parity even)
            6 => (self.f & Flag::S as u8) == 0,  // P (positive)
            7 => (self.f & Flag::S as u8) != 0,  // M (minus)
            _ => unreachable!(),
        }
    }

    /// JP nn — 10 T: M1(4) + MR(3) + MR(3)
    pub fn op_jp_nn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // cycles: 1=pad, 2=read low, 3=pad, 4=pad, 5=read high, 6=pad, 7=done
        match cycle {
            1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let addr = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = addr;
                self.pc = addr;
                self.state = ExecState::Execute(opcode, 6);
            }
            7 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// JP cc,nn — 10 T: M1(4) + MR(3) + MR(3). Always 10T whether taken or not.
    pub fn op_jp_cc_nn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cc = (opcode >> 3) & 0x07;
        match cycle {
            1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let addr = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = addr;
                if self.eval_condition(cc) {
                    self.pc = addr;
                }
                self.state = ExecState::Execute(opcode, 6);
            }
            7 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// JR e — 12 T: M1(4) + MR(3) + internal(5)
    pub fn op_jr_e<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // cycles: 1=pad, 2=read disp, 3=pad, 4-8=internal, 9=done
        match cycle {
            1 | 3 | 4 | 5 | 6 | 7 | 8 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                let disp = bus.read(master, self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                self.pc = self.pc.wrapping_add(disp as i16 as u16);
                self.memptr = self.pc;
                self.state = ExecState::Execute(opcode, 3);
            }
            9 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// JR cc,e — 12 T taken / 7 T not taken
    /// M1(4) + MR(3) + internal(5) when taken; M1(4) + MR(3) when not taken.
    pub fn op_jr_cc_e<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cc = (opcode >> 3) & 0x03; // Only NZ/Z/NC/C for JR cc
        match cycle {
            1 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                let disp = bus.read(master, self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if self.eval_condition(cc) {
                    self.pc = self.pc.wrapping_add(disp as i16 as u16);
                    self.memptr = self.pc;
                    self.temp_data = 1; // taken
                } else {
                    self.temp_data = 0; // not taken
                }
                self.state = ExecState::Execute(opcode, 3);
            }
            4 => {
                if self.temp_data == 0 {
                    self.state = ExecState::Fetch; // Not taken: 7T
                } else {
                    self.state = ExecState::Execute(opcode, 5);
                }
            }
            5..=8 => self.state = ExecState::Execute(opcode, cycle + 1),
            9 => self.state = ExecState::Fetch, // Taken: 12T
            _ => unreachable!(),
        }
    }

    /// JP (HL) — 4 T: M1 only. Really "JP HL" (load PC from HL/IX/IY).
    pub fn op_jp_hl(&mut self) {
        self.pc = self.get_rp(2); // respects index_mode
        self.state = ExecState::Fetch;
    }

    /// DJNZ e — 13 T taken / 8 T not taken
    /// M1(5) + MR(3) + internal(5) when taken; M1(5) + MR(3) when not taken.
    pub fn op_djnz<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            1 => {
                // Extended M1: decrement B
                self.b = self.b.wrapping_sub(1);
                self.state = ExecState::Execute(opcode, 2);
            }
            2 | 3 => self.state = ExecState::Execute(opcode, cycle + 1),
            4 => {
                // MR: read displacement
                let disp = bus.read(master, self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_add(disp as i16 as u16);
                    self.memptr = self.pc;
                    self.temp_data = 1; // taken
                } else {
                    self.temp_data = 0; // not taken
                }
                self.state = ExecState::Execute(opcode, 5);
            }
            5 => {
                if self.temp_data == 0 {
                    self.state = ExecState::Fetch; // Not taken: 8T
                } else {
                    self.state = ExecState::Execute(opcode, 6);
                }
            }
            6..=9 => self.state = ExecState::Execute(opcode, cycle + 1),
            10 => self.state = ExecState::Fetch, // Taken: 13T
            _ => unreachable!(),
        }
    }

    /// CALL nn — 17 T: M1(4) + MR(3) + MR(3) + internal(1) + MW(3) + MW(3)
    pub fn op_call_nn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // cycles: 1=pad, 2=read low, 3=pad, 4=pad, 5=read high, 6=pad,
        //         7=internal, 8=write PC_high, 9-10=pad, 11=write PC_low, 12-13=pad, 14=done
        match cycle {
            1 | 3 | 4 | 6 | 7 | 9 | 10 | 12 | 13 => {
                self.state = ExecState::Execute(opcode, cycle + 1);
            }
            2 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = self.temp_addr;
                self.state = ExecState::Execute(opcode, 6);
            }
            8 => {
                // MW1: push PC high byte
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.state = ExecState::Execute(opcode, 9);
            }
            11 => {
                // MW2: push PC low byte, then jump
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, self.pc as u8);
                self.pc = self.temp_addr;
                self.state = ExecState::Execute(opcode, 12);
            }
            14 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// CALL cc,nn — 17 T taken / 10 T not taken
    /// When not taken: reads both address bytes but doesn't push or jump (same as JP cc,nn).
    pub fn op_call_cc_nn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cc = (opcode >> 3) & 0x07;
        match cycle {
            1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = self.temp_addr;
                self.state = ExecState::Execute(opcode, 6);
            }
            7 => {
                if self.eval_condition(cc) {
                    // Taken: continue to push + jump
                    self.state = ExecState::Execute(opcode, 8);
                } else {
                    // Not taken: 10T
                    self.state = ExecState::Fetch;
                }
            }
            // Taken path: push and jump (cycles 8-14)
            8 | 10 | 12 | 13 => self.state = ExecState::Execute(opcode, cycle + 1),
            9 => {
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.state = ExecState::Execute(opcode, 10);
            }
            11 => {
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, self.pc as u8);
                self.pc = self.temp_addr;
                self.state = ExecState::Execute(opcode, 12);
            }
            14 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// RET — 10 T: M1(4) + MR(3) + MR(3)
    pub fn op_ret<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // cycles: 1=pad, 2=read low, 3=pad, 4=pad, 5=read high, 6=pad, 7=done
        match cycle {
            1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.pc = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = self.pc;
                self.state = ExecState::Execute(opcode, 6);
            }
            7 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// RET cc — 11 T taken / 5 T not taken
    /// M1(5) + MR(3) + MR(3) when taken; M1(5) when not taken.
    pub fn op_ret_cc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cc = (opcode >> 3) & 0x07;
        match cycle {
            1 => {
                // Extended M1: condition evaluation
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                if self.eval_condition(cc) {
                    self.state = ExecState::Execute(opcode, 3); // Taken
                } else {
                    self.state = ExecState::Fetch; // Not taken: 5T
                }
            }
            // Taken path: same structure as RET (shifted by 2)
            3 | 5 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            4 => {
                self.temp_data = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 5);
            }
            7 => {
                let high = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.pc = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = self.pc;
                self.state = ExecState::Execute(opcode, 8);
            }
            8 => self.state = ExecState::Fetch, // Taken: 11T
            _ => unreachable!(),
        }
    }

    /// RST p — 11 T: M1(5) + MW(3) + MW(3)
    /// Target address = opcode & 0x38 (0x00, 0x08, ..., 0x38).
    pub fn op_rst<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let target = (opcode & 0x38) as u16;
        match cycle {
            1 => {
                // Extended M1: internal cycle
                self.state = ExecState::Execute(opcode, 2);
            }
            2 | 4 | 5 | 7 => self.state = ExecState::Execute(opcode, cycle + 1),
            3 => {
                // MW1: push PC high byte
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.state = ExecState::Execute(opcode, 4);
            }
            6 => {
                // MW2: push PC low byte
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, self.pc as u8);
                self.pc = target;
                self.memptr = self.pc;
                self.state = ExecState::Execute(opcode, 7);
            }
            8 => self.state = ExecState::Fetch, // 11T
            _ => unreachable!(),
        }
    }

    /// DI — 4 T: M1 only. Disable interrupts.
    pub fn op_di(&mut self) {
        self.iff1 = false;
        self.iff2 = false;
        self.state = ExecState::Fetch;
    }

    /// EI — 4 T: M1 only. Enable interrupts (with 1-instruction delay).
    pub fn op_ei(&mut self) {
        self.iff1 = true;
        self.iff2 = true;
        self.ei_delay = true;
        self.state = ExecState::Fetch;
    }

    // --- ED Control Flow ---

    /// RETN/RETI — 14T (ED prefix): pop PC, copy IFF2 → IFF1.
    /// 7 handler cycles: 0=IFF2→IFF1+pad, 1=read low, 2=pad, 3=pad, 4=read high, 5=pad, 6=done.
    pub fn op_retn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.iff1 = self.iff2;
                self.state = ExecState::ExecuteED(opcode, 1);
            }
            2 | 3 | 5 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            1 => {
                self.temp_data = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::ExecuteED(opcode, 2);
            }
            4 => {
                let high = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.pc = ((high as u16) << 8) | self.temp_data as u16;
                self.memptr = self.pc;
                self.state = ExecState::ExecuteED(opcode, 5);
            }
            6 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// IM 0/1/2 — 8T (ED prefix): set interrupt mode.
    /// Bits 4-3: 00/01→IM 0, 10→IM 1, 11→IM 2.
    pub fn op_im(&mut self, opcode: u8) {
        self.im = match (opcode >> 3) & 0x03 {
            0 | 1 => 0,
            2 => 1,
            3 => 2,
            _ => unreachable!(),
        };
        self.state = ExecState::Fetch;
    }
}
