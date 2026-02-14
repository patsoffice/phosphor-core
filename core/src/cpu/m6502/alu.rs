use super::{ExecState, M6502, StatusFlag};
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- Flag helpers ----

    /// Set N, Z flags from result (for loads, transfers, logical ops).
    #[inline]
    pub(crate) fn set_nz(&mut self, result: u8) {
        self.set_flag(StatusFlag::N, result & 0x80 != 0);
        self.set_flag(StatusFlag::Z, result == 0);
    }

    /// Set N, Z, C flags for shift/rotate operations.
    #[inline]
    pub(crate) fn set_flags_shift(&mut self, result: u8, carry: bool) {
        self.set_flag(StatusFlag::N, result & 0x80 != 0);
        self.set_flag(StatusFlag::Z, result == 0);
        self.set_flag(StatusFlag::C, carry);
    }

    // ---- ALU operation helpers ----

    /// Perform ADC (Add with Carry). Sets N, Z, C, V. Handles BCD mode.
    /// Binary: A = A + M + C
    /// BCD: A = BCD(A + M + C). N,V from intermediate; Z from binary; C from BCD.
    #[inline]
    pub(crate) fn perform_adc(&mut self, operand: u8) {
        let a = self.a;
        let c: u8 = if self.p & (StatusFlag::C as u8) != 0 {
            1
        } else {
            0
        };

        if self.p & (StatusFlag::D as u8) != 0 {
            // NMOS 6502 decimal mode ADC
            let mut al = (a & 0x0F) as u16 + (operand & 0x0F) as u16 + c as u16;
            if al >= 0x0A {
                al = ((al + 0x06) & 0x0F) + 0x10;
            }
            let mut sum = (a as u16 & 0xF0) + (operand as u16 & 0xF0) + al;

            // N, V from intermediate (before high nibble BCD correction)
            self.set_flag(StatusFlag::N, sum & 0x80 != 0);
            self.set_flag(
                StatusFlag::V,
                (!(a as u16 ^ operand as u16) & (a as u16 ^ sum)) & 0x80 != 0,
            );

            if sum >= 0xA0 {
                sum += 0x60;
            }
            self.set_flag(StatusFlag::C, sum >= 0x100);

            // Z from binary result (NMOS quirk)
            let binary = a as u16 + operand as u16 + c as u16;
            self.set_flag(StatusFlag::Z, (binary & 0xFF) == 0);

            self.a = sum as u8;
        } else {
            // Binary mode ADC
            let sum = a as u16 + operand as u16 + c as u16;
            let result = sum as u8;
            self.set_flag(StatusFlag::C, sum > 0xFF);
            self.set_flag(StatusFlag::V, ((!(a ^ operand)) & (a ^ result)) & 0x80 != 0);
            self.a = result;
            self.set_nz(result);
        }
    }

    /// Perform SBC (Subtract with Carry/Borrow). Sets N, Z, C, V. Handles BCD mode.
    /// Binary: A = A - M - !C (equivalently A + ~M + C)
    /// BCD: All flags from binary result (NMOS quirk); only A gets BCD correction.
    #[inline]
    pub(crate) fn perform_sbc(&mut self, operand: u8) {
        let a = self.a;
        let c: u8 = if self.p & (StatusFlag::C as u8) != 0 {
            1
        } else {
            0
        };

        // Binary subtraction: A + ~M + C
        let diff = a as u16 + (operand ^ 0xFF) as u16 + c as u16;
        let result = diff as u8;

        // Flags always from binary result (even in BCD mode on NMOS)
        self.set_flag(StatusFlag::C, diff > 0xFF);
        self.set_flag(StatusFlag::V, ((a ^ operand) & (a ^ result)) & 0x80 != 0);
        self.set_nz(result);

        if self.p & (StatusFlag::D as u8) != 0 {
            // BCD correction for accumulator only
            let borrow = 1 - c;
            let mut lo = (a & 0x0F) as i16 - (operand & 0x0F) as i16 - borrow as i16;
            let lo_borrow = lo < 0;
            if lo < 0 {
                lo -= 6;
            }
            let mut hi = (a >> 4) as i16 - (operand >> 4) as i16 - if lo_borrow { 1 } else { 0 };
            if hi < 0 {
                hi -= 6;
            }
            self.a = ((hi as u8 & 0x0F) << 4) | (lo as u8 & 0x0F);
        } else {
            self.a = result;
        }
    }

    /// Perform compare (CMP/CPX/CPY). Sets N, Z, C. Does not affect V or any register.
    #[inline]
    pub(crate) fn perform_compare(&mut self, register: u8, operand: u8) {
        let result = register.wrapping_sub(operand);
        self.set_flag(StatusFlag::C, register >= operand);
        self.set_nz(result);
    }

    /// Perform AND. A = A & M, sets N, Z.
    #[inline]
    pub(crate) fn perform_and(&mut self, operand: u8) {
        self.a &= operand;
        self.set_nz(self.a);
    }

    /// Perform ORA. A = A | M, sets N, Z.
    #[inline]
    pub(crate) fn perform_ora(&mut self, operand: u8) {
        self.a |= operand;
        self.set_nz(self.a);
    }

    /// Perform EOR. A = A ^ M, sets N, Z.
    #[inline]
    pub(crate) fn perform_eor(&mut self, operand: u8) {
        self.a ^= operand;
        self.set_nz(self.a);
    }

    /// Perform BIT test. N = M bit 7, V = M bit 6, Z = (A & M) == 0. A is not modified.
    #[inline]
    pub(crate) fn perform_bit(&mut self, operand: u8) {
        self.set_flag(StatusFlag::N, operand & 0x80 != 0);
        self.set_flag(StatusFlag::V, operand & 0x40 != 0);
        self.set_flag(StatusFlag::Z, (self.a & operand) == 0);
    }

    // ---- Read addressing mode helpers ----
    // Each reads an operand and applies a closure, following M6800's alu_imm/alu_direct pattern.

    /// Immediate mode: 1 cycle after fetch. Read operand at PC.
    #[inline]
    pub(crate) fn alu_imm<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 0 {
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            operation(self, operand);
            self.state = ExecState::Fetch;
        }
    }

    /// Zero Page mode: 2 cycles after fetch. Read zp addr, read operand.
    #[inline]
    pub(crate) fn alu_zp<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Zero Page,X mode: 3 cycles after fetch. Read zp addr, add X (wrap in page 0), read operand.
    #[inline]
    pub(crate) fn alu_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal cycle: add X, wrap within zero page
                self.temp_addr = (self.temp_addr.wrapping_add(self.x as u16)) & 0xFF;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Zero Page,Y mode: 3 cycles after fetch. Read zp addr, add Y (wrap in page 0), read operand.
    #[inline]
    pub(crate) fn alu_zp_y<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal cycle: add Y, wrap within zero page
                self.temp_addr = (self.temp_addr.wrapping_add(self.y as u16)) & 0xFF;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Absolute mode: 3 cycles after fetch. Read addr lo, read addr hi, read operand.
    #[inline]
    pub(crate) fn alu_abs<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
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
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Absolute,X mode: 3 or 4 cycles after fetch. +1 cycle if page crossing on reads.
    /// For read operations, if adding X doesn't cross a page boundary, the read
    /// happens on cycle 2 (no penalty). If it crosses, cycle 2 reads the wrong
    /// address and cycle 3 reads the correct one.
    #[inline]
    pub(crate) fn alu_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let hi = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let base = hi << 8 | self.temp_addr;
                self.temp_addr = base.wrapping_add(self.x as u16);
                // Check if page crossed
                if (base ^ self.temp_addr) & 0xFF00 != 0 {
                    // Page crossed — need extra cycle
                    self.state = ExecState::Execute(self.opcode, 2);
                } else {
                    // No page cross — read operand immediately
                    self.state = ExecState::Execute(self.opcode, 3);
                }
            }
            2 => {
                // Extra cycle for page crossing (read from wrong address)
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Absolute,Y mode: 3 or 4 cycles after fetch. +1 cycle if page crossing on reads.
    #[inline]
    pub(crate) fn alu_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let hi = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let base = hi << 8 | self.temp_addr;
                self.temp_addr = base.wrapping_add(self.y as u16);
                if (base ^ self.temp_addr) & 0xFF00 != 0 {
                    self.state = ExecState::Execute(self.opcode, 2);
                } else {
                    self.state = ExecState::Execute(self.opcode, 3);
                }
            }
            2 => {
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// (Indirect,X) mode: 5 cycles after fetch.
    /// Read zp pointer, add X (wrap in zero page), read addr lo, read addr hi, read operand.
    #[inline]
    pub(crate) fn alu_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                // Read zero-page pointer base
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Internal: add X to pointer, wrap in zero page
                self.temp_data = self.temp_data.wrapping_add(self.x);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Read address low byte from (ptr+X) in zero page
                self.temp_addr = bus.read(master, self.temp_data as u16) as u16;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Read address high byte from (ptr+X+1), wrap in zero page
                let hi = bus.read(master, self.temp_data.wrapping_add(1) as u16) as u16;
                self.temp_addr |= hi << 8;
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// (Indirect),Y mode: 4 or 5 cycles after fetch. +1 cycle if page crossing.
    /// Read zp pointer, read addr lo, read addr hi, add Y, read operand.
    #[inline]
    pub(crate) fn alu_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                // Read zero-page pointer
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Read address low byte from zero page
                self.temp_addr = bus.read(master, self.temp_data as u16) as u16;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Read address high byte from (ptr+1), wrap in zero page
                let hi = bus.read(master, self.temp_data.wrapping_add(1) as u16) as u16;
                let base = hi << 8 | self.temp_addr;
                self.temp_addr = base.wrapping_add(self.y as u16);
                if (base ^ self.temp_addr) & 0xFF00 != 0 {
                    // Page crossed — extra cycle
                    self.state = ExecState::Execute(self.opcode, 3);
                } else {
                    // No page cross
                    self.state = ExecState::Execute(self.opcode, 4);
                }
            }
            3 => {
                // Extra cycle for page crossing
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- Store addressing mode helpers ----

    /// Store to Zero Page: 2 cycles after fetch.
    #[inline]
    pub(crate) fn store_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to Zero Page,X: 3 cycles after fetch.
    #[inline]
    pub(crate) fn store_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr = (self.temp_addr.wrapping_add(self.x as u16)) & 0xFF;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to Zero Page,Y: 3 cycles after fetch.
    #[inline]
    pub(crate) fn store_zp_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr = (self.temp_addr.wrapping_add(self.y as u16)) & 0xFF;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to Absolute: 3 cycles after fetch.
    #[inline]
    pub(crate) fn store_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
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
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to Absolute,X: 4 cycles after fetch (always takes penalty cycle).
    #[inline]
    pub(crate) fn store_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let hi = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = (hi << 8 | self.temp_addr).wrapping_add(self.x as u16);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Always takes this cycle (read from potentially wrong address)
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to Absolute,Y: 4 cycles after fetch (always takes penalty cycle).
    #[inline]
    pub(crate) fn store_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let hi = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = (hi << 8 | self.temp_addr).wrapping_add(self.y as u16);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to (Indirect,X): 5 cycles after fetch.
    #[inline]
    pub(crate) fn store_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_data = self.temp_data.wrapping_add(self.x);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.temp_addr = bus.read(master, self.temp_data as u16) as u16;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                let hi = bus.read(master, self.temp_data.wrapping_add(1) as u16) as u16;
                self.temp_addr |= hi << 8;
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store to (Indirect),Y: 5 cycles after fetch (always takes penalty cycle).
    #[inline]
    pub(crate) fn store_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                self.temp_data = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr = bus.read(master, self.temp_data as u16) as u16;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                let hi = bus.read(master, self.temp_data.wrapping_add(1) as u16) as u16;
                self.temp_addr = (hi << 8 | self.temp_addr).wrapping_add(self.y as u16);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Always takes this cycle (write ops don't shortcut)
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- Read-Modify-Write addressing mode helpers ----

    /// RMW Zero Page: 4 cycles after fetch. Read addr, read operand, modify, write back.
    #[inline]
    pub(crate) fn rmw_zp<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Write back unmodified value (6502 RMW quirk), then modify
                self.temp_data = operation(self, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr, self.temp_data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// RMW Zero Page,X: 5 cycles after fetch.
    #[inline]
    pub(crate) fn rmw_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr = (self.temp_addr.wrapping_add(self.x as u16)) & 0xFF;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                self.temp_data = operation(self, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr, self.temp_data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// RMW Absolute: 5 cycles after fetch.
    #[inline]
    pub(crate) fn rmw_abs<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
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
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                self.temp_data = operation(self, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr, self.temp_data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// RMW Absolute,X: 6 cycles after fetch (always takes penalty cycle).
    #[inline]
    pub(crate) fn rmw_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                let hi = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = (hi << 8 | self.temp_addr).wrapping_add(self.x as u16);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Always takes this cycle (RMW never shortcuts page crossing)
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                self.temp_data = operation(self, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                bus.write(master, self.temp_addr, self.temp_data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
