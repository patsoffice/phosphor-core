use super::{CcFlag, ExecState, M6800};
use crate::core::{Bus, BusMaster};

impl M6800 {
    // --- Generic branch helper ---

    /// Branch helper: 4 cycles total (1 fetch + 3 execute).
    /// On 6800, branches always take 4 cycles whether taken or not.
    #[inline]
    fn branch<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        condition: bool,
    ) {
        match cycle {
            0 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                if condition {
                    let offset = self.temp_data as i8 as i16 as u16;
                    self.pc = self.pc.wrapping_add(offset);
                }
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- Conditional branches (4 cycles each) ---

    /// BRA (0x20): Branch always.
    pub(crate) fn op_bra<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, true);
    }

    /// BHI (0x22): Branch if higher (C=0 AND Z=0).
    pub(crate) fn op_bhi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::C as u8)) == 0 && (self.cc & (CcFlag::Z as u8)) == 0;
        self.branch(cycle, bus, master, cond);
    }

    /// BLS (0x23): Branch if lower or same (C=1 OR Z=1).
    pub(crate) fn op_bls<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::C as u8)) != 0 || (self.cc & (CcFlag::Z as u8)) != 0;
        self.branch(cycle, bus, master, cond);
    }

    /// BCC (0x24): Branch if carry clear (C=0).
    pub(crate) fn op_bcc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::C as u8)) == 0);
    }

    /// BCS (0x25): Branch if carry set (C=1).
    pub(crate) fn op_bcs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::C as u8)) != 0);
    }

    /// BNE (0x26): Branch if not equal (Z=0).
    pub(crate) fn op_bne<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::Z as u8)) == 0);
    }

    /// BEQ (0x27): Branch if equal (Z=1).
    pub(crate) fn op_beq<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::Z as u8)) != 0);
    }

    /// BVC (0x28): Branch if overflow clear (V=0).
    pub(crate) fn op_bvc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::V as u8)) == 0);
    }

    /// BVS (0x29): Branch if overflow set (V=1).
    pub(crate) fn op_bvs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::V as u8)) != 0);
    }

    /// BPL (0x2A): Branch if plus (N=0).
    pub(crate) fn op_bpl<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::N as u8)) == 0);
    }

    /// BMI (0x2B): Branch if minus (N=1).
    pub(crate) fn op_bmi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch(cycle, bus, master, (self.cc & (CcFlag::N as u8)) != 0);
    }

    /// BGE (0x2C): Branch if greater or equal signed (N XOR V = 0).
    pub(crate) fn op_bge<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch(cycle, bus, master, n == v);
    }

    /// BLT (0x2D): Branch if less than signed (N XOR V = 1).
    pub(crate) fn op_blt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch(cycle, bus, master, n != v);
    }

    /// BGT (0x2E): Branch if greater than signed (Z=0 AND N XOR V = 0).
    pub(crate) fn op_bgt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let z = (self.cc & (CcFlag::Z as u8)) != 0;
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch(cycle, bus, master, !z && n == v);
    }

    /// BLE (0x2F): Branch if less or equal signed (Z=1 OR N XOR V = 1).
    pub(crate) fn op_ble<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let z = (self.cc & (CcFlag::Z as u8)) != 0;
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch(cycle, bus, master, z || n != v);
    }

    // --- BSR (0x8D): Branch to subroutine ---
    // 8 cycles: 1 fetch + 7 execute

    /// BSR (0x8D): Branch to subroutine.
    /// Pushes return address then branches. No flags affected.
    pub(crate) fn op_bsr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1..=3 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            4 => {
                bus.write(master, self.sp, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 6);
            }
            6 => {
                let offset = self.temp_data as i8 as i16 as u16;
                self.pc = self.pc.wrapping_add(offset);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- JMP ---

    /// JMP indexed (0x6E): Jump to X + offset.
    /// 4 cycles: 1 fetch + 3 execute. No flags affected.
    pub(crate) fn op_jmp_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let offset = bus.read(master, self.pc) as u16;
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.pc = self.temp_addr;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// JMP extended (0x7E): Jump to 16-bit address.
    /// 3 cycles: 1 fetch + 2 execute. No flags affected.
    pub(crate) fn op_jmp_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_addr = (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr |= bus.read(master, self.pc) as u16;
                self.pc = self.temp_addr;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- JSR ---

    /// JSR indexed (0xAD): Jump to subroutine at X + offset.
    /// 8 cycles: 1 fetch + 7 execute. No flags affected.
    pub(crate) fn op_jsr_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let offset = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 | 2 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            3 => {
                bus.write(master, self.sp, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 | 6 => {
                if cycle == 6 {
                    self.pc = self.temp_addr;
                    self.state = ExecState::Fetch;
                } else {
                    self.state = ExecState::Execute(self.opcode, cycle + 1);
                }
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// JSR extended (0xBD): Jump to subroutine at 16-bit address.
    /// 9 cycles: 1 fetch + 8 execute. No flags affected.
    pub(crate) fn op_jsr_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_addr = (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr |= bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 | 3 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            4 => {
                bus.write(master, self.sp, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 6);
            }
            6 | 7 => {
                if cycle == 7 {
                    self.pc = self.temp_addr;
                    self.state = ExecState::Fetch;
                } else {
                    self.state = ExecState::Execute(self.opcode, cycle + 1);
                }
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- RTS ---

    /// RTS (0x39): Return from subroutine.
    /// 5 cycles: 1 fetch + 4 execute. No flags affected.
    pub(crate) fn op_rts<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr = (bus.read(master, self.sp) as u16) << 8;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.temp_addr |= bus.read(master, self.sp) as u16;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                self.pc = self.temp_addr;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
