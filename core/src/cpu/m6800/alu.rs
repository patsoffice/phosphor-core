use super::{CcFlag, ExecState, M6800};
use crate::core::{Bus, BusMaster};

mod binary;
mod shift;
mod unary;

impl M6800 {
    /// Helper to set N, Z, V (cleared) flags for logical operations
    #[inline]
    pub(crate) fn set_flags_logical(&mut self, result: u8) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// Helper to set N, Z, V, C flags for arithmetic operations
    #[inline]
    pub(crate) fn set_flags_arithmetic(&mut self, result: u8, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V, C flags for 16-bit arithmetic
    #[inline]
    pub(crate) fn set_flags_arithmetic16(&mut self, result: u16, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, C flags for shift/rotate operations.
    /// V = N XOR C (post-operation).
    #[inline]
    pub(crate) fn set_flags_shift(&mut self, result: u8, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::C, carry);
        let n = result & 0x80 != 0;
        self.set_flag(CcFlag::V, n ^ carry);
    }

    /// Generic immediate mode helper.
    /// 2 cycles total: 1 fetch + 1 execute (read operand + apply).
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

    /// Generic direct mode helper (page 0 only, no DP register).
    /// 3 cycles total: 1 fetch + 1 read addr + 1 read operand.
    #[inline]
    pub(crate) fn alu_direct<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                // Read address byte (page 0)
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Read operand from effective address
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic extended mode helper (16-bit absolute address).
    /// 4 cycles total: 1 fetch + 1 read hi + 1 read lo + 1 read operand.
    #[inline]
    pub(crate) fn alu_extended<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                // Read high byte of address
                self.temp_addr = (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                // Read low byte of address
                self.temp_addr |= bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Read operand from effective address
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic indexed mode helper (X + unsigned 8-bit offset).
    /// 5 cycles total: 1 fetch + 1 read offset + 2 internal + 1 read operand.
    #[inline]
    pub(crate) fn alu_indexed<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                // Read offset byte
                let offset = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 | 2 => {
                // Internal cycles (address computation)
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            3 => {
                // Read operand from effective address
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic direct mode read-modify-write helper.
    /// 6 cycles total: 1 fetch + 1 read addr + 1 read operand + 1 internal + 1 write + 1 internal.
    #[inline]
    pub(crate) fn rmw_direct<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                // Read operand
                let operand = bus.read(master, self.temp_addr);
                // Store operand in opcode field temporarily (reuse available storage)
                self.opcode = operand;
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                // Internal: perform operation
                let result = operation(self, self.opcode);
                self.opcode = result;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                // Write result back
                bus.write(master, self.temp_addr, self.opcode);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                // Final internal cycle
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic extended mode read-modify-write helper.
    /// 7 cycles total: 1 fetch + 1 hi + 1 lo + 1 read + 1 internal + 1 write + 1 internal.
    #[inline]
    pub(crate) fn rmw_extended<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                self.temp_addr = (bus.read(master, self.pc) as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.temp_addr |= bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                let operand = bus.read(master, self.temp_addr);
                self.opcode = operand;
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                let result = operation(self, self.opcode);
                self.opcode = result;
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr, self.opcode);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic indexed mode read-modify-write helper.
    /// 7 cycles total: 1 fetch + 1 offset + 2 internal + 1 read + 1 write + 1 internal.
    #[inline]
    pub(crate) fn rmw_indexed<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
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
                let offset = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 | 2 => {
                // Internal cycles
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            3 => {
                let operand = bus.read(master, self.temp_addr);
                self.opcode = operand;
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                let result = operation(self, self.opcode);
                self.opcode = result;
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                bus.write(master, self.temp_addr, self.opcode);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
