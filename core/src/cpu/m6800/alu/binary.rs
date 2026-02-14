use crate::core::{Bus, BusMaster};
use crate::cpu::m6800::{CcFlag, M6800};

impl M6800 {
    // --- Internal ALU Helpers ---

    /// ADD: reg + operand. Sets H, N, Z, V, C.
    #[inline]
    pub(crate) fn perform_adda(&mut self, operand: u8) {
        let (result, carry) = self.a.overflowing_add(operand);
        let half_carry = (self.a & 0x0F) + (operand & 0x0F) > 0x0F;
        let overflow = (self.a ^ operand) & 0x80 == 0 && (self.a ^ result) & 0x80 != 0;
        self.a = result;
        self.set_flag(CcFlag::H, half_carry);
        self.set_flags_arithmetic(result, overflow, carry);
    }

    #[inline]
    pub(crate) fn perform_addb(&mut self, operand: u8) {
        let (result, carry) = self.b.overflowing_add(operand);
        let half_carry = (self.b & 0x0F) + (operand & 0x0F) > 0x0F;
        let overflow = (self.b ^ operand) & 0x80 == 0 && (self.b ^ result) & 0x80 != 0;
        self.b = result;
        self.set_flag(CcFlag::H, half_carry);
        self.set_flags_arithmetic(result, overflow, carry);
    }

    /// ADC: reg + operand + carry. Sets H, N, Z, V, C.
    #[inline]
    pub(crate) fn perform_adca(&mut self, operand: u8) {
        let carry_in = (self.cc & CcFlag::C as u8) as u16;
        let a_u16 = self.a as u16;
        let m_u16 = operand as u16;
        let sum = a_u16 + m_u16 + carry_in;
        let result = sum as u8;
        let carry_out = sum > 0xFF;
        let half_carry = (self.a & 0x0F) + (operand & 0x0F) + (carry_in as u8) > 0x0F;
        let overflow = (self.a ^ operand) & 0x80 == 0 && (self.a ^ result) & 0x80 != 0;
        self.a = result;
        self.set_flag(CcFlag::H, half_carry);
        self.set_flags_arithmetic(result, overflow, carry_out);
    }

    #[inline]
    pub(crate) fn perform_adcb(&mut self, operand: u8) {
        let carry_in = (self.cc & CcFlag::C as u8) as u16;
        let b_u16 = self.b as u16;
        let m_u16 = operand as u16;
        let sum = b_u16 + m_u16 + carry_in;
        let result = sum as u8;
        let carry_out = sum > 0xFF;
        let half_carry = (self.b & 0x0F) + (operand & 0x0F) + (carry_in as u8) > 0x0F;
        let overflow = (self.b ^ operand) & 0x80 == 0 && (self.b ^ result) & 0x80 != 0;
        self.b = result;
        self.set_flag(CcFlag::H, half_carry);
        self.set_flags_arithmetic(result, overflow, carry_out);
    }

    /// SUB: reg - operand. Sets N, Z, V, C.
    #[inline]
    pub(crate) fn perform_suba(&mut self, operand: u8) {
        let (result, borrow) = self.a.overflowing_sub(operand);
        let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;
        self.a = result;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    #[inline]
    pub(crate) fn perform_subb(&mut self, operand: u8) {
        let (result, borrow) = self.b.overflowing_sub(operand);
        let overflow = (self.b ^ operand) & 0x80 != 0 && (self.b ^ result) & 0x80 != 0;
        self.b = result;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    /// SBC: reg - operand - carry. Sets N, Z, V, C.
    #[inline]
    pub(crate) fn perform_sbca(&mut self, operand: u8) {
        let carry = (self.cc & CcFlag::C as u8) as u16;
        let a = self.a as u16;
        let m = operand as u16;
        let diff = a.wrapping_sub(m).wrapping_sub(carry);
        let result = diff as u8;
        let borrow = a < m + carry;
        let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;
        self.a = result;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    #[inline]
    pub(crate) fn perform_sbcb(&mut self, operand: u8) {
        let carry = (self.cc & CcFlag::C as u8) as u16;
        let b = self.b as u16;
        let m = operand as u16;
        let diff = b.wrapping_sub(m).wrapping_sub(carry);
        let result = diff as u8;
        let borrow = b < m + carry;
        let overflow = (self.b ^ operand) & 0x80 != 0 && (self.b ^ result) & 0x80 != 0;
        self.b = result;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    /// CMP: reg - operand (discard result). Sets N, Z, V, C.
    #[inline]
    pub(crate) fn perform_cmpa(&mut self, operand: u8) {
        let (result, borrow) = self.a.overflowing_sub(operand);
        let overflow = (self.a ^ operand) & 0x80 != 0 && (self.a ^ result) & 0x80 != 0;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    #[inline]
    pub(crate) fn perform_cmpb(&mut self, operand: u8) {
        let (result, borrow) = self.b.overflowing_sub(operand);
        let overflow = (self.b ^ operand) & 0x80 != 0 && (self.b ^ result) & 0x80 != 0;
        self.set_flags_arithmetic(result, overflow, borrow);
    }

    /// AND: reg & operand. Sets N, Z. V cleared.
    #[inline]
    pub(crate) fn perform_anda(&mut self, operand: u8) {
        self.a &= operand;
        self.set_flags_logical(self.a);
    }

    #[inline]
    pub(crate) fn perform_andb(&mut self, operand: u8) {
        self.b &= operand;
        self.set_flags_logical(self.b);
    }

    /// BIT: reg & operand (discard result). Sets N, Z. V cleared.
    #[inline]
    pub(crate) fn perform_bita(&mut self, operand: u8) {
        let result = self.a & operand;
        self.set_flags_logical(result);
    }

    #[inline]
    pub(crate) fn perform_bitb(&mut self, operand: u8) {
        let result = self.b & operand;
        self.set_flags_logical(result);
    }

    /// EOR: reg ^ operand. Sets N, Z. V cleared.
    #[inline]
    pub(crate) fn perform_eora(&mut self, operand: u8) {
        self.a ^= operand;
        self.set_flags_logical(self.a);
    }

    #[inline]
    pub(crate) fn perform_eorb(&mut self, operand: u8) {
        self.b ^= operand;
        self.set_flags_logical(self.b);
    }

    /// ORA: reg | operand. Sets N, Z. V cleared.
    #[inline]
    pub(crate) fn perform_oraa(&mut self, operand: u8) {
        self.a |= operand;
        self.set_flags_logical(self.a);
    }

    #[inline]
    pub(crate) fn perform_orab(&mut self, operand: u8) {
        self.b |= operand;
        self.set_flags_logical(self.b);
    }

    // --- Immediate mode ops (2 cycles: 1 fetch + 1 read operand & execute) ---

    /// SUBA immediate (0x80). N, Z, V, C affected.
    pub(crate) fn op_suba_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_suba(op));
    }

    /// CMPA immediate (0x81). N, Z, V, C affected.
    pub(crate) fn op_cmpa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_cmpa(op));
    }

    /// SBCA immediate (0x82). A = A - M - C. N, Z, V, C affected.
    pub(crate) fn op_sbca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sbca(op));
    }

    /// ANDA immediate (0x84). N, Z affected. V cleared.
    pub(crate) fn op_anda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_anda(op));
    }

    /// BITA immediate (0x85). N, Z affected. V cleared.
    pub(crate) fn op_bita_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_bita(op));
    }

    /// EORA immediate (0x88). N, Z affected. V cleared.
    pub(crate) fn op_eora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_eora(op));
    }

    /// ADCA immediate (0x89). A = A + M + C. H, N, Z, V, C affected.
    pub(crate) fn op_adca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adca(op));
    }

    /// ORAA immediate (0x8A). N, Z affected. V cleared.
    pub(crate) fn op_oraa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_oraa(op));
    }

    /// ADDA immediate (0x8B). H, N, Z, V, C affected.
    pub(crate) fn op_adda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adda(op));
    }

    /// SUBB immediate (0xC0). N, Z, V, C affected.
    pub(crate) fn op_subb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_subb(op));
    }

    /// CMPB immediate (0xC1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_cmpb(op));
    }

    /// SBCB immediate (0xC2). B = B - M - C. N, Z, V, C affected.
    pub(crate) fn op_sbcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sbcb(op));
    }

    /// ANDB immediate (0xC4). N, Z affected. V cleared.
    pub(crate) fn op_andb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_andb(op));
    }

    /// BITB immediate (0xC5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_bitb(op));
    }

    /// EORB immediate (0xC8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_eorb(op));
    }

    /// ADCB immediate (0xC9). B = B + M + C. H, N, Z, V, C affected.
    pub(crate) fn op_adcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adcb(op));
    }

    /// ORAB immediate (0xCA). N, Z affected. V cleared.
    pub(crate) fn op_orab_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_orab(op));
    }

    /// ADDB immediate (0xCB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_addb(op));
    }
}
