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

    // --- Direct mode ops (3 cycles: 1 fetch + 1 read addr + 1 read operand) ---

    /// SUBA direct (0x90). N, Z, V, C affected.
    pub(crate) fn op_suba_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_suba(op));
    }

    /// CMPA direct (0x91). N, Z, V, C affected.
    pub(crate) fn op_cmpa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_cmpa(op));
    }

    /// SBCA direct (0x92). N, Z, V, C affected.
    pub(crate) fn op_sbca_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sbca(op));
    }

    /// ANDA direct (0x94). N, Z affected. V cleared.
    pub(crate) fn op_anda_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_anda(op));
    }

    /// BITA direct (0x95). N, Z affected. V cleared.
    pub(crate) fn op_bita_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_bita(op));
    }

    /// EORA direct (0x98). N, Z affected. V cleared.
    pub(crate) fn op_eora_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_eora(op));
    }

    /// ADCA direct (0x99). H, N, Z, V, C affected.
    pub(crate) fn op_adca_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_adca(op));
    }

    /// ORAA direct (0x9A). N, Z affected. V cleared.
    pub(crate) fn op_oraa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_oraa(op));
    }

    /// ADDA direct (0x9B). H, N, Z, V, C affected.
    pub(crate) fn op_adda_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_adda(op));
    }

    /// SUBB direct (0xD0). N, Z, V, C affected.
    pub(crate) fn op_subb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_subb(op));
    }

    /// CMPB direct (0xD1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_cmpb(op));
    }

    /// SBCB direct (0xD2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sbcb(op));
    }

    /// ANDB direct (0xD4). N, Z affected. V cleared.
    pub(crate) fn op_andb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_andb(op));
    }

    /// BITB direct (0xD5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_bitb(op));
    }

    /// EORB direct (0xD8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_eorb(op));
    }

    /// ADCB direct (0xD9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_adcb(op));
    }

    /// ORAB direct (0xDA). N, Z affected. V cleared.
    pub(crate) fn op_orab_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_orab(op));
    }

    /// ADDB direct (0xDB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_addb(op));
    }

    // --- Indexed mode ops (5 cycles: 1 fetch + 1 read offset + 2 internal + 1 read operand) ---

    /// SUBA indexed (0xA0). N, Z, V, C affected.
    pub(crate) fn op_suba_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_suba(op));
    }

    /// CMPA indexed (0xA1). N, Z, V, C affected.
    pub(crate) fn op_cmpa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_cmpa(op));
    }

    /// SBCA indexed (0xA2). N, Z, V, C affected.
    pub(crate) fn op_sbca_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sbca(op));
    }

    /// ANDA indexed (0xA4). N, Z affected. V cleared.
    pub(crate) fn op_anda_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_anda(op));
    }

    /// BITA indexed (0xA5). N, Z affected. V cleared.
    pub(crate) fn op_bita_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_bita(op));
    }

    /// EORA indexed (0xA8). N, Z affected. V cleared.
    pub(crate) fn op_eora_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_eora(op));
    }

    /// ADCA indexed (0xA9). H, N, Z, V, C affected.
    pub(crate) fn op_adca_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_adca(op));
    }

    /// ORAA indexed (0xAA). N, Z affected. V cleared.
    pub(crate) fn op_oraa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_oraa(op));
    }

    /// ADDA indexed (0xAB). H, N, Z, V, C affected.
    pub(crate) fn op_adda_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_adda(op));
    }

    /// SUBB indexed (0xE0). N, Z, V, C affected.
    pub(crate) fn op_subb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_subb(op));
    }

    /// CMPB indexed (0xE1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_cmpb(op));
    }

    /// SBCB indexed (0xE2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sbcb(op));
    }

    /// ANDB indexed (0xE4). N, Z affected. V cleared.
    pub(crate) fn op_andb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_andb(op));
    }

    /// BITB indexed (0xE5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_bitb(op));
    }

    /// EORB indexed (0xE8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_eorb(op));
    }

    /// ADCB indexed (0xE9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_adcb(op));
    }

    /// ORAB indexed (0xEA). N, Z affected. V cleared.
    pub(crate) fn op_orab_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_orab(op));
    }

    /// ADDB indexed (0xEB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_addb(op));
    }

    // --- Extended mode ops (4 cycles: 1 fetch + 1 read hi + 1 read lo + 1 read operand) ---

    /// SUBA extended (0xB0). N, Z, V, C affected.
    pub(crate) fn op_suba_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_suba(op));
    }

    /// CMPA extended (0xB1). N, Z, V, C affected.
    pub(crate) fn op_cmpa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_cmpa(op));
    }

    /// SBCA extended (0xB2). N, Z, V, C affected.
    pub(crate) fn op_sbca_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sbca(op));
    }

    /// ANDA extended (0xB4). N, Z affected. V cleared.
    pub(crate) fn op_anda_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_anda(op));
    }

    /// BITA extended (0xB5). N, Z affected. V cleared.
    pub(crate) fn op_bita_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_bita(op));
    }

    /// EORA extended (0xB8). N, Z affected. V cleared.
    pub(crate) fn op_eora_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_eora(op));
    }

    /// ADCA extended (0xB9). H, N, Z, V, C affected.
    pub(crate) fn op_adca_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_adca(op));
    }

    /// ORAA extended (0xBA). N, Z affected. V cleared.
    pub(crate) fn op_oraa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_oraa(op));
    }

    /// ADDA extended (0xBB). H, N, Z, V, C affected.
    pub(crate) fn op_adda_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_adda(op));
    }

    /// SUBB extended (0xF0). N, Z, V, C affected.
    pub(crate) fn op_subb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_subb(op));
    }

    /// CMPB extended (0xF1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_cmpb(op));
    }

    /// SBCB extended (0xF2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sbcb(op));
    }

    /// ANDB extended (0xF4). N, Z affected. V cleared.
    pub(crate) fn op_andb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_andb(op));
    }

    /// BITB extended (0xF5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_bitb(op));
    }

    /// EORB extended (0xF8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_eorb(op));
    }

    /// ADCB extended (0xF9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_adcb(op));
    }

    /// ORAB extended (0xFA). N, Z affected. V cleared.
    pub(crate) fn op_orab_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_orab(op));
    }

    /// ADDB extended (0xFB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_addb(op));
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
