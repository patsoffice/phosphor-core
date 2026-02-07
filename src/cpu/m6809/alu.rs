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

    /// The alu_imm function is a generic helper method designed to reduce code duplication for Immediate Addressing Mode ALU instructions (like ADDA #$10, ANDB #$FF, etc.).
    ///
    /// In the Motorola 6809, immediate mode instructions always follow a specific pattern.
    #[inline]
    fn alu_imm<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 0 {
            // 1. Fetch the operand from memory at PC
            let operand = bus.read(master, self.pc);
            // 2. Advance PC to the next instruction
            self.pc = self.pc.wrapping_add(1);
            // 3. Run the specific ALU logic provided by the caller
            operation(self, operand);
            // 4. Return to Fetch state for the next instruction
            self.state = ExecState::Fetch;
        }
    }

    /// SUBA immediate (0x80): Subtracts the immediate operand from accumulator A.
    pub(crate) fn op_suba_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.a.overflowing_sub(operand);
            let half_borrow = (cpu.a & 0x0F) < (operand & 0x0F);
            let overflow = (cpu.a ^ operand) & 0x80 != 0 && (cpu.a ^ result) & 0x80 != 0;
            cpu.a = result;
            cpu.set_flag(CcFlag::H, half_borrow);
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// ADDA immediate (0x8B): Adds the immediate operand to accumulator A.
    pub(crate) fn op_adda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, carry) = cpu.a.overflowing_add(operand);
            let half_carry = (cpu.a & 0x0F) + (operand & 0x0F) > 0x0F;
            let overflow = (cpu.a ^ operand) & 0x80 == 0 && (cpu.a ^ result) & 0x80 != 0;
            cpu.a = result;
            cpu.set_flag(CcFlag::H, half_carry);
            cpu.set_flags_arithmetic(result, overflow, carry);
        });
    }

    /// MUL inherent (0x3D): Multiplies A and B (unsigned), result in D (A=high, B=low).
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

    /// CMPA immediate (0x81): Compares accumulator A with the immediate operand (A - M).
    pub(crate) fn op_cmpa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.a.overflowing_sub(operand);
            let overflow = (cpu.a ^ operand) & 0x80 != 0 && (cpu.a ^ result) & 0x80 != 0;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// SBCA immediate (0x82): Subtracts the immediate operand and carry from accumulator A.
    pub(crate) fn op_sbca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let carry = if cpu.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

            let a = cpu.a as u16;
            let m = operand as u16;
            let c = carry as u16;

            let diff = a.wrapping_sub(m).wrapping_sub(c);
            let result = diff as u8;
            let borrow = a < m + c;

            let overflow = (cpu.a ^ operand) & 0x80 != 0 && (cpu.a ^ result) & 0x80 != 0;

            cpu.a = result;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// ANDA immediate (0x84): Performs bitwise AND of accumulator A with the immediate operand.
    pub(crate) fn op_anda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a &= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// BITA immediate (0x85): Performs bitwise AND of accumulator A with the immediate operand, updating flags but discarding result.
    pub(crate) fn op_bita_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let result = cpu.a & operand;
            cpu.set_flags_logical(result);
        });
    }

    /// EORA immediate (0x88): Performs bitwise Exclusive OR of accumulator A with the immediate operand.
    pub(crate) fn op_eora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a ^= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// ADCA immediate (0x89): Adds the immediate operand and carry to accumulator A.
    pub(crate) fn op_adca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let carry_in = if cpu.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

            let a_u16 = cpu.a as u16;
            let m_u16 = operand as u16;
            let c_u16 = carry_in as u16;

            let sum = a_u16 + m_u16 + c_u16;
            let result = sum as u8;
            let carry_out = sum > 0xFF;

            let half_carry = (cpu.a & 0x0F) + (operand & 0x0F) + carry_in > 0x0F;
            let overflow = (cpu.a ^ operand) & 0x80 == 0 && (cpu.a ^ result) & 0x80 != 0;

            cpu.a = result;
            cpu.set_flag(CcFlag::H, half_carry);
            cpu.set_flags_arithmetic(result, overflow, carry_out);
        });
    }

    /// ORA immediate (0x8A): Performs bitwise OR of accumulator A with the immediate operand.
    pub(crate) fn op_ora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a |= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// SUBB immediate (0xC0): Subtracts the immediate operand from accumulator B.
    pub(crate) fn op_subb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.b.overflowing_sub(operand);
            let overflow = (cpu.b ^ operand) & 0x80 != 0 && (cpu.b ^ result) & 0x80 != 0;
            cpu.b = result;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// CMPB immediate (0xC1): Compares accumulator B with the immediate operand (B - M).
    pub(crate) fn op_cmpb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.b.overflowing_sub(operand);
            let overflow = (cpu.b ^ operand) & 0x80 != 0 && (cpu.b ^ result) & 0x80 != 0;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// SBCB immediate (0xC2): Subtracts the immediate operand and carry from accumulator B.
    pub(crate) fn op_sbcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let carry = if cpu.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

            let b = cpu.b as u16;
            let m = operand as u16;
            let c = carry as u16;

            let diff = b.wrapping_sub(m).wrapping_sub(c);
            let result = diff as u8;
            let borrow = b < m + c;

            let overflow = (cpu.b ^ operand) & 0x80 != 0 && (cpu.b ^ result) & 0x80 != 0;

            cpu.b = result;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// ANDB immediate (0xC4): Performs bitwise AND of accumulator B with the immediate operand.
    pub(crate) fn op_andb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b &= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// BITB immediate (0xC5): Performs bitwise AND of accumulator B with the immediate operand, updating flags but discarding result.
    pub(crate) fn op_bitb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let result = cpu.b & operand;
            cpu.set_flags_logical(result);
        });
    }

    /// EORB immediate (0xC8): Performs bitwise Exclusive OR of accumulator B with the immediate operand.
    pub(crate) fn op_eorb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b ^= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// ADCB immediate (0xC9): Adds the immediate operand and carry to accumulator B.
    pub(crate) fn op_adcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let carry_in = if cpu.cc & (CcFlag::C as u8) != 0 { 1 } else { 0 };

            let b_u16 = cpu.b as u16;
            let m_u16 = operand as u16;
            let c_u16 = carry_in as u16;

            let sum = b_u16 + m_u16 + c_u16;
            let result = sum as u8;
            let carry_out = sum > 0xFF;

            let half_carry = (cpu.b & 0x0F) + (operand & 0x0F) + carry_in > 0x0F;
            let overflow = (cpu.b ^ operand) & 0x80 == 0 && (cpu.b ^ result) & 0x80 != 0;

            cpu.b = result;
            cpu.set_flag(CcFlag::H, half_carry);
            cpu.set_flags_arithmetic(result, overflow, carry_out);
        });
    }

    /// ORB immediate (0xCA): Performs bitwise OR of accumulator B with the immediate operand.
    pub(crate) fn op_orb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b |= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// ADDB immediate (0xCB): Adds the immediate operand to accumulator B.
    pub(crate) fn op_addb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, carry) = cpu.b.overflowing_add(operand);
            let half_carry = (cpu.b & 0x0F) + (operand & 0x0F) > 0x0F;
            let overflow = (cpu.b ^ operand) & 0x80 == 0 && (cpu.b ^ result) & 0x80 != 0;
            cpu.b = result;
            cpu.set_flag(CcFlag::H, half_carry);
            cpu.set_flags_arithmetic(result, overflow, carry);
        });
    }
}
