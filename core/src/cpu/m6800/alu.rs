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

    /// Helper to set N, Z, V (cleared) flags for 16-bit logical operations (LDX, LDS)
    #[inline]
    pub(crate) fn set_flags_logical16(&mut self, result: u16) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
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

    // ---- 8-bit Store helpers ----

    /// Store 8-bit value to direct address.
    /// 4 cycles total: 1 fetch + 1 read addr + 1 internal + 1 write.
    #[inline]
    pub(crate) fn store_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
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
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store 8-bit value to extended (16-bit) address.
    /// 5 cycles total: 1 fetch + 1 read addr hi + 1 read addr lo + 1 internal + 1 write.
    #[inline]
    pub(crate) fn store_extended<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
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

    /// Store 8-bit value to indexed address (X + offset).
    /// 6 cycles total: 1 fetch + 1 read offset + 3 internal + 1 write.
    #[inline]
    pub(crate) fn store_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u8,
    ) {
        match cycle {
            0 => {
                let offset = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 | 2 | 3 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            4 => {
                bus.write(master, self.temp_addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- 16-bit Read helpers ----

    /// Read 16-bit value from direct address and apply operation.
    /// 4 cycles total: 1 fetch + 1 read addr + 1 read hi + 1 read lo.
    #[inline]
    pub(crate) fn alu16_direct<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u16),
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
                let lo = bus.read(master, self.temp_addr.wrapping_add(1));
                let val = (self.temp_data as u16) << 8 | lo as u16;
                operation(self, val);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Read 16-bit value from extended address and apply operation.
    /// 5 cycles total: 1 fetch + 1 read addr hi + 1 read addr lo + 1 read hi + 1 read lo.
    #[inline]
    pub(crate) fn alu16_extended<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u16),
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
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                let lo = bus.read(master, self.temp_addr.wrapping_add(1));
                let val = (self.temp_data as u16) << 8 | lo as u16;
                operation(self, val);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Read 16-bit value from indexed address (X + offset) and apply operation.
    /// 6 cycles total: 1 fetch + 1 read offset + 2 internal + 1 read hi + 1 read lo.
    #[inline]
    pub(crate) fn alu16_indexed<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u16),
    {
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
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                let lo = bus.read(master, self.temp_addr.wrapping_add(1));
                let val = (self.temp_data as u16) << 8 | lo as u16;
                operation(self, val);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- 16-bit Store helpers ----

    /// Store 16-bit value to direct address.
    /// 5 cycles total: 1 fetch + 1 read addr + 1 internal + 1 write hi + 1 write lo.
    #[inline]
    pub(crate) fn store16_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u16,
    ) {
        match cycle {
            0 => {
                self.temp_addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                bus.write(master, self.temp_addr, (data >> 8) as u8);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr.wrapping_add(1), data as u8);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store 16-bit value to extended (16-bit) address.
    /// 6 cycles total: 1 fetch + 1 read addr hi + 1 read addr lo + 1 internal + 1 write hi + 1 write lo.
    #[inline]
    pub(crate) fn store16_extended<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u16,
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
            2 => {
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr, (data >> 8) as u8);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                bus.write(master, self.temp_addr.wrapping_add(1), data as u8);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Store 16-bit value to indexed address (X + offset).
    /// 7 cycles total: 1 fetch + 1 read offset + 3 internal + 1 write hi + 1 write lo.
    #[inline]
    pub(crate) fn store16_indexed<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        data: u16,
    ) {
        match cycle {
            0 => {
                let offset = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = self.x.wrapping_add(offset);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 | 2 | 3 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            4 => {
                bus.write(master, self.temp_addr, (data >> 8) as u8);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            5 => {
                bus.write(master, self.temp_addr.wrapping_add(1), data as u8);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ---- Read-Modify-Write helpers ----

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
                self.temp_data = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.temp_data = operation(self, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            3 => {
                bus.write(master, self.temp_addr, self.temp_data);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            4 => {
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// Generic extended mode read-modify-write helper.
    /// 6 cycles total: 1 fetch + 1 hi + 1 lo + 1 read + 1 internal + 1 write.
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
                self.state = ExecState::Execute(self.opcode, cycle + 1);
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
