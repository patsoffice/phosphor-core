use crate::core::{Bus, BusMaster};
use super::{M6809, CcFlag, ExecState};

impl M6809 {
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
                self.set_flag(CcFlag::N, result & 0x80 != 0);
                self.set_flag(CcFlag::Z, result == 0);
                self.set_flag(CcFlag::V, overflow);
                self.set_flag(CcFlag::C, borrow);
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
                self.set_flag(CcFlag::N, result & 0x80 != 0);
                self.set_flag(CcFlag::Z, result == 0);
                self.set_flag(CcFlag::V, overflow);
                self.set_flag(CcFlag::C, carry);
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
}
