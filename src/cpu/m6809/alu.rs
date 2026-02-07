use crate::core::{Bus, BusMaster};
use super::{M6809, CcFlag, ExecState};

impl M6809 {
    /// Helper to set N, Z, V (cleared) flags for logical operations
    #[inline]
    fn set_flags_logical(&mut self, result: u8) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// Helper to set N, Z, V, C flags for arithmetic operations
    #[inline]
    fn set_flags_arithmetic(&mut self, result: u8, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// SUBA immediate (0x80)
    pub(crate) fn op_suba_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let (result, borrow) = self.a.overflowing_sub(operand);
                let half_borrow = (self.a & 0x0F) < (operand & 0x0F);
                let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;
                self.a = result;
                self.set_flag(CcFlag::H, half_borrow);
                self.set_flags_arithmetic(result, overflow, borrow);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ADDA immediate (0x8B)
    pub(crate) fn op_adda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let (result, carry) = self.a.overflowing_add(operand);
                let half_carry = (self.a & 0x0F) + (operand & 0x0F) > 0x0F;
                let overflow = (self.a ^ operand) & 0x80 == 0 && (self.a ^ result) & 0x80 != 0;
                self.a = result;
                self.set_flag(CcFlag::H, half_carry);
                self.set_flags_arithmetic(result, overflow, carry);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// MUL inherent (0x3D): A * B -> D (A=high, B=low)
    pub(crate) fn op_mul(&mut self, cycle: u8) {
        match cycle {
            0 => {
                let result = (self.a as u16) * (self.b as u16);
                self.a = (result >> 8) as u8;
                self.b = (result & 0xFF) as u8;
                self.set_flag(CcFlag::Z, result == 0);
                self.set_flag(CcFlag::C, self.b & 0x80 != 0);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// CMPA immediate (0x81)
    pub(crate) fn op_cmpa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let (result, borrow) = self.a.overflowing_sub(operand);
                let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;
                self.set_flags_arithmetic(result, overflow, borrow);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SBCA immediate (0x82)
    pub(crate) fn op_sbca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let carry = if self.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

                let a = self.a as u16;
                let m = operand as u16;
                let c = carry as u16;

                let diff = a.wrapping_sub(m).wrapping_sub(c);
                let result = diff as u8;
                let borrow = a < m + c;

                let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;

                self.a = result;
                self.set_flags_arithmetic(result, overflow, borrow);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ANDA immediate (0x84)
    pub(crate) fn op_anda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a &= operand;
                self.set_flags_logical(self.a);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// BITA immediate (0x85)
    pub(crate) fn op_bita_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let result = self.a & operand;
                self.set_flags_logical(result);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// EORA immediate (0x88)
    pub(crate) fn op_eora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a ^= operand;
                self.set_flags_logical(self.a);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ADCA immediate (0x89)
    pub(crate) fn op_adca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                let carry_in = if self.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

                let a_u16 = self.a as u16;
                let m_u16 = operand as u16;
                let c_u16 = carry_in as u16;

                let sum = a_u16 + m_u16 + c_u16;
                let result = sum as u8;
                let carry_out = sum > 0xFF;

                let half_carry = (self.a & 0x0F) + (operand & 0x0F) + carry_in > 0x0F;
                let overflow = (self.a ^ operand) & 0x80 == 0 && (self.a ^ result) & 0x80 != 0;

                self.a = result;
                self.set_flag(CcFlag::H, half_carry);
                self.set_flags_arithmetic(result, overflow, carry_out);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ORA immediate (0x8A)
    pub(crate) fn op_ora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let operand = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a |= operand;
                self.set_flags_logical(self.a);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }
}
