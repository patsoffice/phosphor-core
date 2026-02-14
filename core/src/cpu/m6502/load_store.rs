use super::M6502;
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- LDA (Load Accumulator) ----

    /// LDA Immediate (0xA9) - 2 cycles
    pub(crate) fn op_lda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA Zero Page (0xA5) - 3 cycles
    pub(crate) fn op_lda_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA Zero Page,X (0xB5) - 4 cycles
    pub(crate) fn op_lda_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA Absolute (0xAD) - 4 cycles
    pub(crate) fn op_lda_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA Absolute,X (0xBD) - 4 or 5 cycles (+1 page crossing)
    pub(crate) fn op_lda_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA Absolute,Y (0xB9) - 4 or 5 cycles (+1 page crossing)
    pub(crate) fn op_lda_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA (Indirect,X) (0xA1) - 6 cycles
    pub(crate) fn op_lda_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }

    /// LDA (Indirect),Y (0xB1) - 5 or 6 cycles (+1 page crossing)
    pub(crate) fn op_lda_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| {
            cpu.a = op;
            cpu.set_nz(op);
        });
    }
}
