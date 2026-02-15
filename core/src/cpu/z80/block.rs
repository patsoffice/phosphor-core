use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, Flag, Z80};

impl Z80 {
    // --- Block Transfer ---

    /// LDI/LDD — 16T: Main M1(4) + ED M1(4) + MR(3) + MW(3) + internal(2)
    /// LDI (0xA0): (DE)←(HL), HL++, DE++, BC--
    /// LDD (0xA8): (DE)←(HL), HL--, DE--, BC--
    /// 9 handler cycles: 0=pad, 1=read(HL), 2=pad, 3=write(DE), 4-5=pad, 6-7=internal, 8=done
    pub fn op_ldi_ldd<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 | 2 | 4 | 5 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            1 => {
                self.temp_data = bus.read(master, self.get_hl());
                self.state = ExecState::ExecuteED(opcode, 2);
            }
            3 => {
                bus.write(master, self.get_de(), self.temp_data);
                self.state = ExecState::ExecuteED(opcode, 4);
            }
            6 => {
                // Internal: update registers and flags
                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));
                self.set_de(self.get_de().wrapping_add(delta));
                self.set_bc(self.get_bc().wrapping_sub(1));

                let n = self.temp_data.wrapping_add(self.a);
                let mut f = self.f & (Flag::S as u8 | Flag::Z as u8 | Flag::C as u8);
                if self.get_bc() != 0 { f |= Flag::PV as u8; }
                // Undocumented: X = bit 3 of (val+A), Y = bit 1 of (val+A)
                if (n & 0x08) != 0 { f |= Flag::X as u8; }
                if (n & 0x02) != 0 { f |= Flag::Y as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 7);
            }
            8 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// LDIR/LDDR — 21T repeating / 16T when done
    /// Like LDI/LDD but repeats while BC != 0.
    /// 9 handler cycles when done, 14 when repeating (extra 5T for PC -= 2).
    pub fn op_ldir_lddr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 | 2 | 4 | 5 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            1 => {
                self.temp_data = bus.read(master, self.get_hl());
                self.state = ExecState::ExecuteED(opcode, 2);
            }
            3 => {
                bus.write(master, self.get_de(), self.temp_data);
                self.state = ExecState::ExecuteED(opcode, 4);
            }
            6 => {
                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));
                self.set_de(self.get_de().wrapping_add(delta));
                self.set_bc(self.get_bc().wrapping_sub(1));

                let n = self.temp_data.wrapping_add(self.a);
                let mut f = self.f & (Flag::S as u8 | Flag::Z as u8 | Flag::C as u8);
                if self.get_bc() != 0 { f |= Flag::PV as u8; }
                if (n & 0x08) != 0 { f |= Flag::X as u8; }
                if (n & 0x02) != 0 { f |= Flag::Y as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 7);
            }
            8 => {
                if self.get_bc() == 0 {
                    self.state = ExecState::Fetch; // Done: 16T
                } else {
                    self.pc = self.pc.wrapping_sub(2);
                    self.memptr = self.pc.wrapping_add(1);
                    self.state = ExecState::ExecuteED(opcode, 9);
                }
            }
            9..=12 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            13 => self.state = ExecState::Fetch, // Repeating: 21T
            _ => unreachable!(),
        }
    }

    // --- Block Compare ---

    /// CPI/CPD — 16T: Main M1(4) + ED M1(4) + MR(3) + internal(5)
    /// CPI (0xA1): compare A-(HL), HL++, BC--
    /// CPD (0xA9): compare A-(HL), HL--, BC--
    /// 9 handler cycles: 0=pad, 1=read(HL), 2=pad, 3-7=internal, 8=done
    pub fn op_cpi_cpd<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 | 2 | 4 | 5 | 6 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            1 => {
                self.temp_data = bus.read(master, self.get_hl());
                self.state = ExecState::ExecuteED(opcode, 2);
            }
            3 => {
                // Internal: compare and update
                let val = self.temp_data;
                let result = self.a.wrapping_sub(val);
                let h = (self.a & 0xF) < (val & 0xF);

                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));
                self.set_bc(self.get_bc().wrapping_sub(1));
                if dec {
                    self.memptr = self.memptr.wrapping_sub(1);
                } else {
                    self.memptr = self.memptr.wrapping_add(1);
                }

                let mut f = self.f & Flag::C as u8; // preserve C
                f |= Flag::N as u8;
                if result == 0 { f |= Flag::Z as u8; }
                if (result & 0x80) != 0 { f |= Flag::S as u8; }
                if h { f |= Flag::H as u8; }
                if self.get_bc() != 0 { f |= Flag::PV as u8; }
                // Undocumented X/Y: n = result - H_flag
                let n = result.wrapping_sub(if h { 1 } else { 0 });
                if (n & 0x08) != 0 { f |= Flag::X as u8; }
                if (n & 0x02) != 0 { f |= Flag::Y as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 4);
            }
            8 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// CPIR/CPDR — 21T repeating / 16T when done
    /// Repeats while BC != 0 and Z = 0 (not found).
    pub fn op_cpir_cpdr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 | 2 | 4 | 5 | 6 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            1 => {
                self.temp_data = bus.read(master, self.get_hl());
                self.state = ExecState::ExecuteED(opcode, 2);
            }
            3 => {
                let val = self.temp_data;
                let result = self.a.wrapping_sub(val);
                let h = (self.a & 0xF) < (val & 0xF);

                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));
                self.set_bc(self.get_bc().wrapping_sub(1));
                if dec {
                    self.memptr = self.memptr.wrapping_sub(1);
                } else {
                    self.memptr = self.memptr.wrapping_add(1);
                }

                let mut f = self.f & Flag::C as u8;
                f |= Flag::N as u8;
                if result == 0 { f |= Flag::Z as u8; }
                if (result & 0x80) != 0 { f |= Flag::S as u8; }
                if h { f |= Flag::H as u8; }
                if self.get_bc() != 0 { f |= Flag::PV as u8; }
                let n = result.wrapping_sub(if h { 1 } else { 0 });
                if (n & 0x08) != 0 { f |= Flag::X as u8; }
                if (n & 0x02) != 0 { f |= Flag::Y as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 4);
            }
            8 => {
                let z = (self.f & Flag::Z as u8) != 0;
                if self.get_bc() == 0 || z {
                    self.state = ExecState::Fetch; // Done: 16T
                } else {
                    self.pc = self.pc.wrapping_sub(2);
                    self.memptr = self.pc.wrapping_add(1);
                    self.state = ExecState::ExecuteED(opcode, 9);
                }
            }
            9..=12 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            13 => self.state = ExecState::Fetch, // Repeating: 21T
            _ => unreachable!(),
        }
    }

    // --- Block I/O (stubbed: IN reads 0xFF, OUT discards) ---

    /// INI/IND — 16T: Main M1(4) + ED M1(5) + IO(4) + MW(3)
    /// B--, IN port C → (HL), HL±±
    /// 9 handler cycles: 0=B--, 1-4=IO(stub), 5=write(HL), 6-7=pad, 8=done
    pub fn op_ini_ind<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 => {
                self.b = self.b.wrapping_sub(1);
                self.state = ExecState::ExecuteED(opcode, 1);
            }
            1..=3 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            4 => {
                // IO read (stubbed as 0xFF)
                self.temp_data = 0xFF;
                self.state = ExecState::ExecuteED(opcode, 5);
            }
            5 => {
                bus.write(master, self.get_hl(), self.temp_data);
                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));

                let mut f = self.f & Flag::C as u8;
                f |= Flag::N as u8;
                if self.b == 0 { f |= Flag::Z as u8; }
                f |= self.b & (Flag::X as u8 | Flag::Y as u8);
                if (self.b & 0x80) != 0 { f |= Flag::S as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 6);
            }
            6 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            8 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// INIR/INDR — 21T repeating / 16T when done
    pub fn op_inir_indr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // Cycles 0-7: same as INI/IND
        if cycle <= 7 {
            self.op_ini_ind(opcode, cycle, bus, master);
            // Override: don't go to Fetch at cycle 8, instead check repeat
            if cycle == 7 {
                self.state = ExecState::ExecuteED(opcode, 8);
            }
            return;
        }
        match cycle {
            8 => {
                if self.b == 0 {
                    self.state = ExecState::Fetch;
                } else {
                    self.pc = self.pc.wrapping_sub(2);
                    self.state = ExecState::ExecuteED(opcode, 9);
                }
            }
            9..=12 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            13 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// OUTI/OUTD — 16T: Main M1(4) + ED M1(5) + MR(3) + IO(4)
    /// B--, (HL) → OUT port C, HL±±
    /// 9 handler cycles: 0=B--, 1=pad, 2=read(HL), 3=pad, 4-7=IO(discard), 8=done
    pub fn op_outi_outd<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let dec = (opcode & 0x08) != 0;
        match cycle {
            0 => {
                self.b = self.b.wrapping_sub(1);
                self.state = ExecState::ExecuteED(opcode, 1);
            }
            1 | 3 | 5 | 6 | 7 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.get_hl());
                let delta: u16 = if dec { 0xFFFF } else { 1 };
                self.set_hl(self.get_hl().wrapping_add(delta));
                self.state = ExecState::ExecuteED(opcode, 3);
            }
            4 => {
                // IO write (discarded)
                let mut f = self.f & Flag::C as u8;
                f |= Flag::N as u8;
                if self.b == 0 { f |= Flag::Z as u8; }
                f |= self.b & (Flag::X as u8 | Flag::Y as u8);
                if (self.b & 0x80) != 0 { f |= Flag::S as u8; }
                self.f = f;
                self.state = ExecState::ExecuteED(opcode, 5);
            }
            8 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// OTIR/OTDR — 21T repeating / 16T when done
    pub fn op_otir_otdr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        if cycle <= 7 {
            self.op_outi_outd(opcode, cycle, bus, master);
            if cycle == 7 {
                self.state = ExecState::ExecuteED(opcode, 8);
            }
            return;
        }
        match cycle {
            8 => {
                if self.b == 0 {
                    self.state = ExecState::Fetch;
                } else {
                    self.pc = self.pc.wrapping_sub(2);
                    self.state = ExecState::ExecuteED(opcode, 9);
                }
            }
            9..=12 => self.state = ExecState::ExecuteED(opcode, cycle + 1),
            13 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }
}
