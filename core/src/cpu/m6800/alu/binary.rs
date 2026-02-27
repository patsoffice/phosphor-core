use crate::core::{Bus, BusMaster};
use crate::cpu::m68xx::{Acc, M68xxAlu};
use crate::cpu::m6800::M6800;

impl M6800 {
    // --- Direct mode ops (3 cycles: 1 fetch + 1 read addr + 1 read operand) ---

    /// SUBA direct (0x90). N, Z, V, C affected.
    pub(crate) fn op_suba_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::A, op));
    }

    /// CMPA direct (0x91). N, Z, V, C affected.
    pub(crate) fn op_cmpa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::A, op));
    }

    /// SBCA direct (0x92). N, Z, V, C affected.
    pub(crate) fn op_sbca_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::A, op));
    }

    /// ANDA direct (0x94). N, Z affected. V cleared.
    pub(crate) fn op_anda_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::A, op));
    }

    /// BITA direct (0x95). N, Z affected. V cleared.
    pub(crate) fn op_bita_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::A, op));
    }

    /// EORA direct (0x98). N, Z affected. V cleared.
    pub(crate) fn op_eora_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::A, op));
    }

    /// ADCA direct (0x99). H, N, Z, V, C affected.
    pub(crate) fn op_adca_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::A, op));
    }

    /// ORAA direct (0x9A). N, Z affected. V cleared.
    pub(crate) fn op_oraa_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::A, op));
    }

    /// ADDA direct (0x9B). H, N, Z, V, C affected.
    pub(crate) fn op_adda_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::A, op));
    }

    /// SUBB direct (0xD0). N, Z, V, C affected.
    pub(crate) fn op_subb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::B, op));
    }

    /// CMPB direct (0xD1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::B, op));
    }

    /// SBCB direct (0xD2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::B, op));
    }

    /// ANDB direct (0xD4). N, Z affected. V cleared.
    pub(crate) fn op_andb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::B, op));
    }

    /// BITB direct (0xD5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::B, op));
    }

    /// EORB direct (0xD8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::B, op));
    }

    /// ADCB direct (0xD9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::B, op));
    }

    /// ORAB direct (0xDA). N, Z affected. V cleared.
    pub(crate) fn op_orab_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::B, op));
    }

    /// ADDB direct (0xDB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_dir<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_direct(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::B, op));
    }

    // --- Indexed mode ops (5 cycles: 1 fetch + 1 read offset + 2 internal + 1 read operand) ---

    /// SUBA indexed (0xA0). N, Z, V, C affected.
    pub(crate) fn op_suba_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::A, op));
    }

    /// CMPA indexed (0xA1). N, Z, V, C affected.
    pub(crate) fn op_cmpa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::A, op));
    }

    /// SBCA indexed (0xA2). N, Z, V, C affected.
    pub(crate) fn op_sbca_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::A, op));
    }

    /// ANDA indexed (0xA4). N, Z affected. V cleared.
    pub(crate) fn op_anda_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::A, op));
    }

    /// BITA indexed (0xA5). N, Z affected. V cleared.
    pub(crate) fn op_bita_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::A, op));
    }

    /// EORA indexed (0xA8). N, Z affected. V cleared.
    pub(crate) fn op_eora_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::A, op));
    }

    /// ADCA indexed (0xA9). H, N, Z, V, C affected.
    pub(crate) fn op_adca_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::A, op));
    }

    /// ORAA indexed (0xAA). N, Z affected. V cleared.
    pub(crate) fn op_oraa_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::A, op));
    }

    /// ADDA indexed (0xAB). H, N, Z, V, C affected.
    pub(crate) fn op_adda_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::A, op));
    }

    /// SUBB indexed (0xE0). N, Z, V, C affected.
    pub(crate) fn op_subb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::B, op));
    }

    /// CMPB indexed (0xE1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::B, op));
    }

    /// SBCB indexed (0xE2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::B, op));
    }

    /// ANDB indexed (0xE4). N, Z affected. V cleared.
    pub(crate) fn op_andb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::B, op));
    }

    /// BITB indexed (0xE5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::B, op));
    }

    /// EORB indexed (0xE8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::B, op));
    }

    /// ADCB indexed (0xE9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::B, op));
    }

    /// ORAB indexed (0xEA). N, Z affected. V cleared.
    pub(crate) fn op_orab_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::B, op));
    }

    /// ADDB indexed (0xEB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_idx<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_indexed(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::B, op));
    }

    // --- Extended mode ops (4 cycles: 1 fetch + 1 read hi + 1 read lo + 1 read operand) ---

    /// SUBA extended (0xB0). N, Z, V, C affected.
    pub(crate) fn op_suba_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::A, op));
    }

    /// CMPA extended (0xB1). N, Z, V, C affected.
    pub(crate) fn op_cmpa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::A, op));
    }

    /// SBCA extended (0xB2). N, Z, V, C affected.
    pub(crate) fn op_sbca_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::A, op));
    }

    /// ANDA extended (0xB4). N, Z affected. V cleared.
    pub(crate) fn op_anda_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::A, op));
    }

    /// BITA extended (0xB5). N, Z affected. V cleared.
    pub(crate) fn op_bita_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::A, op));
    }

    /// EORA extended (0xB8). N, Z affected. V cleared.
    pub(crate) fn op_eora_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::A, op));
    }

    /// ADCA extended (0xB9). H, N, Z, V, C affected.
    pub(crate) fn op_adca_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::A, op));
    }

    /// ORAA extended (0xBA). N, Z affected. V cleared.
    pub(crate) fn op_oraa_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::A, op));
    }

    /// ADDA extended (0xBB). H, N, Z, V, C affected.
    pub(crate) fn op_adda_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::A, op));
    }

    /// SUBB extended (0xF0). N, Z, V, C affected.
    pub(crate) fn op_subb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::B, op));
    }

    /// CMPB extended (0xF1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::B, op));
    }

    /// SBCB extended (0xF2). N, Z, V, C affected.
    pub(crate) fn op_sbcb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::B, op));
    }

    /// ANDB extended (0xF4). N, Z affected. V cleared.
    pub(crate) fn op_andb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::B, op));
    }

    /// BITB extended (0xF5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::B, op));
    }

    /// EORB extended (0xF8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::B, op));
    }

    /// ADCB extended (0xF9). H, N, Z, V, C affected.
    pub(crate) fn op_adcb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::B, op));
    }

    /// ORAB extended (0xFA). N, Z affected. V cleared.
    pub(crate) fn op_orab_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::B, op));
    }

    /// ADDB extended (0xFB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_ext<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_extended(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::B, op));
    }

    // --- Immediate mode ops (2 cycles: 1 fetch + 1 read operand & execute) ---

    /// SUBA immediate (0x80). N, Z, V, C affected.
    pub(crate) fn op_suba_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::A, op));
    }

    /// CMPA immediate (0x81). N, Z, V, C affected.
    pub(crate) fn op_cmpa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::A, op));
    }

    /// SBCA immediate (0x82). A = A - M - C. N, Z, V, C affected.
    pub(crate) fn op_sbca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::A, op));
    }

    /// ANDA immediate (0x84). N, Z affected. V cleared.
    pub(crate) fn op_anda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::A, op));
    }

    /// BITA immediate (0x85). N, Z affected. V cleared.
    pub(crate) fn op_bita_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::A, op));
    }

    /// EORA immediate (0x88). N, Z affected. V cleared.
    pub(crate) fn op_eora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::A, op));
    }

    /// ADCA immediate (0x89). A = A + M + C. H, N, Z, V, C affected.
    pub(crate) fn op_adca_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::A, op));
    }

    /// ORAA immediate (0x8A). N, Z affected. V cleared.
    pub(crate) fn op_oraa_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::A, op));
    }

    /// ADDA immediate (0x8B). H, N, Z, V, C affected.
    pub(crate) fn op_adda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::A, op));
    }

    /// SUBB immediate (0xC0). N, Z, V, C affected.
    pub(crate) fn op_subb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sub(Acc::B, op));
    }

    /// CMPB immediate (0xC1). N, Z, V, C affected.
    pub(crate) fn op_cmpb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_cmp(Acc::B, op));
    }

    /// SBCB immediate (0xC2). B = B - M - C. N, Z, V, C affected.
    pub(crate) fn op_sbcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sbc(Acc::B, op));
    }

    /// ANDB immediate (0xC4). N, Z affected. V cleared.
    pub(crate) fn op_andb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_and(Acc::B, op));
    }

    /// BITB immediate (0xC5). N, Z affected. V cleared.
    pub(crate) fn op_bitb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_bit(Acc::B, op));
    }

    /// EORB immediate (0xC8). N, Z affected. V cleared.
    pub(crate) fn op_eorb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_eor(Acc::B, op));
    }

    /// ADCB immediate (0xC9). B = B + M + C. H, N, Z, V, C affected.
    pub(crate) fn op_adcb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adc(Acc::B, op));
    }

    /// ORAB immediate (0xCA). N, Z affected. V cleared.
    pub(crate) fn op_orab_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_or(Acc::B, op));
    }

    /// ADDB immediate (0xCB). H, N, Z, V, C affected.
    pub(crate) fn op_addb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_add(Acc::B, op));
    }
}
