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
