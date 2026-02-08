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

    /// Helper to set N, Z, V, C flags for 16-bit arithmetic
    #[inline]
    fn set_flags_arithmetic16(&mut self, result: u16, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V (cleared) flags for 16-bit logical operations
    #[inline]
    fn set_flags_logical16(&mut self, result: u16) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
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

    /// ADDD immediate (0xC3): Adds a 16-bit immediate value to the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned carry out of bit 15.
    pub(crate) fn op_addd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => { // Fetch high byte of operand
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => { // Fetch low byte, execute
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let operand = self.temp_addr | low;

                let d = self.get_d();
                let (result, carry) = d.overflowing_add(operand);
                let overflow = (d ^ operand) & 0x8000 == 0 && (d ^ result) & 0x8000 != 0;

                self.set_d(result);
                self.set_flags_arithmetic16(result, overflow, carry);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SUBD immediate (0x83): Subtracts a 16-bit immediate value from the D register.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_subd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => { // Fetch high byte of operand
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => { // Fetch low byte, execute
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let operand = self.temp_addr | low;

                let d = self.get_d();
                let (result, borrow) = d.overflowing_sub(operand);
                let overflow = (d ^ operand) & 0x8000 != 0 && (d ^ result) & 0x8000 != 0;

                self.set_d(result);
                self.set_flags_arithmetic16(result, overflow, borrow);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// CMPX immediate (0x8C): Compare X with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
    pub(crate) fn op_cmpx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr |= low;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let operand = self.temp_addr;
                let (result, borrow) = self.x.overflowing_sub(operand);
                let overflow = (self.x ^ operand) & 0x8000 != 0 && (self.x ^ result) & 0x8000 != 0;
                self.set_flags_arithmetic16(result, overflow, borrow);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDD immediate (0xCC): Load D with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldd_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.set_d(val);
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDX immediate (0x8E): Load X with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.x = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDU immediate (0xCE): Load U with 16-bit immediate.
    /// N set if result bit 15 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ldu_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let val = self.temp_addr | low;
                self.u = val;
                self.set_flags_logical16(val);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SUBA immediate (0x80): Subtracts the immediate operand from accumulator A.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred (operands had different signs and result sign differs from A).
    /// C set if unsigned borrow occurred (operand > A). H set if borrow from bit 4.
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
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred (operands had same sign and result sign differs).
    /// C set if unsigned carry out of bit 7. H set if carry from bit 3 to bit 4.
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
    /// Z set if 16-bit result is zero. C set if bit 7 of B (low byte) is set.
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
    /// Performs subtraction but discards the result; only flags are updated.
    /// N set if result bit 7 is set. Z set if A == operand.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred (operand > A).
    pub(crate) fn op_cmpa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.a.overflowing_sub(operand);
            let overflow = (cpu.a ^ operand) & 0x80 != 0 && (cpu.a ^ result) & 0x80 != 0;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// SBCA immediate (0x82): Subtracts the immediate operand and carry from accumulator A.
    /// A = A - M - C. Used for multi-byte subtraction chains.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
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
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_anda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a &= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// BITA immediate (0x85): Bit test A — performs A AND operand, updates flags but discards result.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_bita_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let result = cpu.a & operand;
            cpu.set_flags_logical(result);
        });
    }

    /// EORA immediate (0x88): Performs bitwise Exclusive OR of accumulator A with the immediate operand.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_eora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a ^= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// ADCA immediate (0x89): Adds the immediate operand and carry to accumulator A.
    /// A = A + M + C. Used for multi-byte addition chains.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned carry out of bit 7.
    /// H set if carry from bit 3 to bit 4.
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
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_ora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.a |= operand;
            cpu.set_flags_logical(cpu.a);
        });
    }

    /// SUBB immediate (0xC0): Subtracts the immediate operand from accumulator B.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred (operands had different signs and result sign differs from B).
    /// C set if unsigned borrow occurred (operand > B).
    pub(crate) fn op_subb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.b.overflowing_sub(operand);
            let overflow = (cpu.b ^ operand) & 0x80 != 0 && (cpu.b ^ result) & 0x80 != 0;
            cpu.b = result;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// CMPB immediate (0xC1): Compares accumulator B with the immediate operand (B - M).
    /// Performs subtraction but discards the result; only flags are updated.
    /// N set if result bit 7 is set. Z set if B == operand.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred (operand > B).
    pub(crate) fn op_cmpb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let (result, borrow) = cpu.b.overflowing_sub(operand);
            let overflow = (cpu.b ^ operand) & 0x80 != 0 && (cpu.b ^ result) & 0x80 != 0;
            cpu.set_flags_arithmetic(result, overflow, borrow);
        });
    }

    /// SBCB immediate (0xC2): Subtracts the immediate operand and carry from accumulator B.
    /// B = B - M - C. Used for multi-byte subtraction chains.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned borrow occurred.
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
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_andb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b &= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// BITB immediate (0xC5): Bit test B — performs B AND operand, updates flags but discards result.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_bitb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            let result = cpu.b & operand;
            cpu.set_flags_logical(result);
        });
    }

    /// EORB immediate (0xC8): Performs bitwise Exclusive OR of accumulator B with the immediate operand.
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_eorb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b ^= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// ADCB immediate (0xC9): Adds the immediate operand and carry to accumulator B.
    /// B = B + M + C. Used for multi-byte addition chains.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred. C set if unsigned carry out of bit 7.
    /// H set if carry from bit 3 to bit 4.
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
    /// N set if result bit 7 is set. Z set if result is zero. V always cleared.
    pub(crate) fn op_orb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        self.alu_imm(cycle, bus, master, |cpu, operand| {
            cpu.b |= operand;
            cpu.set_flags_logical(cpu.b);
        });
    }

    /// ADDB immediate (0xCB): Adds the immediate operand to accumulator B.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if signed overflow occurred (operands had same sign and result sign differs).
    /// C set if unsigned carry out of bit 7. H set if carry from bit 3 to bit 4.
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

    /// NEGA inherent (0x40): Negate A (A = 0 - A, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if A was non-zero (borrow occurred from 0).
    pub(crate) fn op_nega(&mut self, cycle: u8) {
        if cycle == 0 {
            let (result, borrow) = (0u8).overflowing_sub(self.a);
            // Overflow occurs if A was 0x80 (-128), because -(-128) = +128 is not representable in i8
            let overflow = self.a == 0x80;
            self.a = result;
            self.set_flags_arithmetic(result, overflow, borrow);
            self.state = ExecState::Fetch;
        }
    }

    /// NEGB inherent (0x50): Negate B (B = 0 - B, two's complement).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 (-128), since -(-128) overflows signed 8-bit range.
    /// C set if B was non-zero (borrow occurred from 0).
    pub(crate) fn op_negb(&mut self, cycle: u8) {
        if cycle == 0 {
            let (result, borrow) = (0u8).overflowing_sub(self.b);
            let overflow = self.b == 0x80;
            self.b = result;
            self.set_flags_arithmetic(result, overflow, borrow);
            self.state = ExecState::Fetch;
        }
    }

    /// COMA inherent (0x43): Complement A (A = ~A, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_coma(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = !self.a;
            // COM sets N, Z, clears V, and sets C=1
            self.set_flags_logical(self.a);
            self.set_flag(CcFlag::C, true);
            self.state = ExecState::Fetch;
        }
    }

    /// COMB inherent (0x53): Complement B (B = ~B, one's complement / bitwise NOT).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V always cleared. C always set.
    pub(crate) fn op_comb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = !self.b;
            self.set_flags_logical(self.b);
            self.set_flag(CcFlag::C, true);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRA inherent (0x4F): Clear A (A = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clra(&mut self, cycle: u8) {
        if cycle == 0 {
            self.a = 0;
            self.set_flag(CcFlag::N, false);
            self.set_flag(CcFlag::Z, true);
            self.set_flag(CcFlag::V, false);
            self.set_flag(CcFlag::C, false);
            self.state = ExecState::Fetch;
        }
    }

    /// CLRB inherent (0x5F): Clear B (B = 0).
    /// Flags are always set to fixed values: N=0, Z=1, V=0, C=0.
    pub(crate) fn op_clrb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.b = 0;
            self.set_flag(CcFlag::N, false);
            self.set_flag(CcFlag::Z, true);
            self.set_flag(CcFlag::V, false);
            self.set_flag(CcFlag::C, false);
            self.state = ExecState::Fetch;
        }
    }

    /// INCA inherent (0x4C): Increment A (A = A + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_inca(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.a == 0x7F;
            self.a = self.a.wrapping_add(1);
            self.set_flag(CcFlag::N, self.a & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.a == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// INCB inherent (0x5C): Increment B (B = B + 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x7F before increment (positive-to-negative signed overflow).
    /// C is not affected.
    pub(crate) fn op_incb(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.b == 0x7F;
            self.b = self.b.wrapping_add(1);
            self.set_flag(CcFlag::N, self.b & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.b == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// DECA inherent (0x4A): Decrement A (A = A - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if A was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_deca(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.a == 0x80;
            self.a = self.a.wrapping_sub(1);
            self.set_flag(CcFlag::N, self.a & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.a == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// DECB inherent (0x5A): Decrement B (B = B - 1).
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V set if B was 0x80 before decrement (negative-to-positive signed overflow).
    /// C is not affected.
    pub(crate) fn op_decb(&mut self, cycle: u8) {
        if cycle == 0 {
            let overflow = self.b == 0x80;
            self.b = self.b.wrapping_sub(1);
            self.set_flag(CcFlag::N, self.b & 0x80 != 0);
            self.set_flag(CcFlag::Z, self.b == 0);
            self.set_flag(CcFlag::V, overflow);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTA inherent (0x4D): Test A (set flags based on A, no modification).
    /// N set if A bit 7 is set. Z set if A is zero. V always cleared.
    pub(crate) fn op_tsta(&mut self, cycle: u8) {
        if cycle == 0 {
            self.set_flags_logical(self.a);
            self.state = ExecState::Fetch;
        }
    }

    /// TSTB inherent (0x5D): Test B (set flags based on B, no modification).
    /// N set if B bit 7 is set. Z set if B is zero. V always cleared.
    pub(crate) fn op_tstb(&mut self, cycle: u8) {
        if cycle == 0 {
            self.set_flags_logical(self.b);
            self.state = ExecState::Fetch;
        }
    }

    // --- Shift and Rotate instructions ---

    /// Helper to set N, Z, V, C flags for shift/rotate operations.
    /// V is always set to N XOR C (post-operation) per 6809 datasheet.
    #[inline]
    fn set_flags_shift(&mut self, result: u8, carry: bool) {
        let n = result & 0x80 != 0;
        self.set_flag(CcFlag::N, n);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, n ^ carry);
        self.set_flag(CcFlag::C, carry);
    }

    /// ASLA/LSLA inherent (0x48): Arithmetic/Logical Shift Left A.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_asla(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x80 != 0;
            self.a = self.a << 1;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASLB/LSLB inherent (0x58): Arithmetic/Logical Shift Left B.
    /// Shifts all bits left one position. Bit 7 goes to C, 0 enters bit 0.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 7.
    pub(crate) fn op_aslb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x80 != 0;
            self.b = self.b << 1;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRA inherent (0x47): Arithmetic Shift Right A.
    /// Shifts all bits right one position. Bit 7 is preserved (sign extension).
    /// Bit 0 goes to C.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 0.
    pub(crate) fn op_asra(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x01 != 0;
            self.a = ((self.a as i8) >> 1) as u8;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ASRB inherent (0x57): Arithmetic Shift Right B.
    /// Shifts all bits right one position. Bit 7 is preserved (sign extension).
    /// Bit 0 goes to C.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-shift). C set to old bit 0.
    pub(crate) fn op_asrb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x01 != 0;
            self.b = ((self.b as i8) >> 1) as u8;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRA inherent (0x44): Logical Shift Right A.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsra(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.a & 0x01 != 0;
            self.a = self.a >> 1;
            self.set_flags_shift(self.a, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// LSRB inherent (0x54): Logical Shift Right B.
    /// Shifts all bits right one position. 0 enters bit 7, bit 0 goes to C.
    /// N always cleared. Z set if result is zero.
    /// V = N XOR C = C (since N=0). C set to old bit 0.
    pub(crate) fn op_lsrb(&mut self, cycle: u8) {
        if cycle == 0 {
            let carry = self.b & 0x01 != 0;
            self.b = self.b >> 1;
            self.set_flags_shift(self.b, carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLA inherent (0x49): Rotate Left A through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rola(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.a & 0x80 != 0;
            self.a = (self.a << 1) | (old_carry as u8);
            self.set_flags_shift(self.a, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// ROLB inherent (0x59): Rotate Left B through Carry.
    /// Old bit 7 goes to C, old C enters bit 0, other bits shift left.
    /// N set if result bit 7 is set. Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 7.
    pub(crate) fn op_rolb(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.b & 0x80 != 0;
            self.b = (self.b << 1) | (old_carry as u8);
            self.set_flags_shift(self.b, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// RORA inherent (0x46): Rotate Right A through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rora(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.a & 0x01 != 0;
            self.a = (self.a >> 1) | ((old_carry as u8) << 7);
            self.set_flags_shift(self.a, new_carry);
            self.state = ExecState::Fetch;
        }
    }

    /// RORB inherent (0x56): Rotate Right B through Carry.
    /// Old bit 0 goes to C, old C enters bit 7, other bits shift right.
    /// N set if result bit 7 is set (i.e., old C was set). Z set if result is zero.
    /// V = N XOR C (post-rotate). C set to old bit 0.
    pub(crate) fn op_rorb(&mut self, cycle: u8) {
        if cycle == 0 {
            let old_carry = self.cc & (CcFlag::C as u8) != 0;
            let new_carry = self.b & 0x01 != 0;
            self.b = (self.b >> 1) | ((old_carry as u8) << 7);
            self.set_flags_shift(self.b, new_carry);
            self.state = ExecState::Fetch;
        }
    }
}
