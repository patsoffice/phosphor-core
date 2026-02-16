use super::M6502;
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- LDA (Load Accumulator) ----

    /// LDA Immediate (0xA9) - 2 cycles. N, Z affected.
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

    /// LDA Zero Page (0xA5) - 3 cycles. N, Z affected.
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

    /// LDA Zero Page,X (0xB5) - 4 cycles. N, Z affected.
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

    /// LDA Absolute (0xAD) - 4 cycles. N, Z affected.
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

    /// LDA Absolute,X (0xBD) - 4 or 5 cycles (+1 page crossing). N, Z affected.
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

    /// LDA Absolute,Y (0xB9) - 4 or 5 cycles (+1 page crossing). N, Z affected.
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

    /// LDA (Indirect,X) (0xA1) - 6 cycles. N, Z affected.
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

    /// LDA (Indirect),Y (0xB1) - 5 or 6 cycles (+1 page crossing). N, Z affected.
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

    // ---- LDX (Load X Register) ----

    /// LDX Immediate (0xA2) - 2 cycles. N, Z affected.
    pub(crate) fn op_ldx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.x = op;
            cpu.set_nz(op);
        });
    }

    /// LDX Zero Page (0xA6) - 3 cycles. N, Z affected.
    pub(crate) fn op_ldx_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            cpu.x = op;
            cpu.set_nz(op);
        });
    }

    /// LDX Zero Page,Y (0xB6) - 4 cycles. N, Z affected. Note: uses Y, not X.
    pub(crate) fn op_ldx_zp_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_y(cycle, bus, master, |cpu, op| {
            cpu.x = op;
            cpu.set_nz(op);
        });
    }

    /// LDX Absolute (0xAE) - 4 cycles. N, Z affected.
    pub(crate) fn op_ldx_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            cpu.x = op;
            cpu.set_nz(op);
        });
    }

    /// LDX Absolute,Y (0xBE) - 4 or 5 cycles (+1 page crossing). N, Z affected. Note: uses Y, not X.
    pub(crate) fn op_ldx_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| {
            cpu.x = op;
            cpu.set_nz(op);
        });
    }

    // ---- LDY (Load Y Register) ----

    /// LDY Immediate (0xA0) - 2 cycles. N, Z affected.
    pub(crate) fn op_ldy_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            cpu.y = op;
            cpu.set_nz(op);
        });
    }

    /// LDY Zero Page (0xA4) - 3 cycles. N, Z affected.
    pub(crate) fn op_ldy_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            cpu.y = op;
            cpu.set_nz(op);
        });
    }

    /// LDY Zero Page,X (0xB4) - 4 cycles. N, Z affected.
    pub(crate) fn op_ldy_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| {
            cpu.y = op;
            cpu.set_nz(op);
        });
    }

    /// LDY Absolute (0xAC) - 4 cycles. N, Z affected.
    pub(crate) fn op_ldy_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            cpu.y = op;
            cpu.set_nz(op);
        });
    }

    /// LDY Absolute,X (0xBC) - 4 or 5 cycles (+1 page crossing). N, Z affected.
    pub(crate) fn op_ldy_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| {
            cpu.y = op;
            cpu.set_nz(op);
        });
    }

    // ---- STA (Store Accumulator) ----

    /// STA Zero Page (0x85) - 3 cycles. No flags affected.
    pub(crate) fn op_sta_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_zp(cycle, bus, master, data);
    }

    /// STA Zero Page,X (0x95) - 4 cycles. No flags affected.
    pub(crate) fn op_sta_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_zp_x(cycle, bus, master, data);
    }

    /// STA Absolute (0x8D) - 4 cycles. No flags affected.
    pub(crate) fn op_sta_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_abs(cycle, bus, master, data);
    }

    /// STA Absolute,X (0x9D) - 5 cycles (always takes penalty cycle). No flags affected.
    pub(crate) fn op_sta_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_abs_x(cycle, bus, master, data);
    }

    /// STA Absolute,Y (0x99) - 5 cycles (always takes penalty cycle). No flags affected.
    pub(crate) fn op_sta_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_abs_y(cycle, bus, master, data);
    }

    /// STA (Indirect,X) (0x81) - 6 cycles. No flags affected.
    pub(crate) fn op_sta_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_ind_x(cycle, bus, master, data);
    }

    /// STA (Indirect),Y (0x91) - 6 cycles (always takes penalty cycle). No flags affected.
    pub(crate) fn op_sta_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.a;
        self.store_ind_y(cycle, bus, master, data);
    }

    // ---- STX (Store X Register) ----

    /// STX Zero Page (0x86) - 3 cycles. No flags affected.
    pub(crate) fn op_stx_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.x;
        self.store_zp(cycle, bus, master, data);
    }

    /// STX Zero Page,Y (0x96) - 4 cycles. No flags affected. Note: uses Y, not X.
    pub(crate) fn op_stx_zp_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.x;
        self.store_zp_y(cycle, bus, master, data);
    }

    /// STX Absolute (0x8E) - 4 cycles. No flags affected.
    pub(crate) fn op_stx_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.x;
        self.store_abs(cycle, bus, master, data);
    }

    // ---- STY (Store Y Register) ----

    /// STY Zero Page (0x84) - 3 cycles. No flags affected.
    pub(crate) fn op_sty_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.y;
        self.store_zp(cycle, bus, master, data);
    }

    /// STY Zero Page,X (0x94) - 4 cycles. No flags affected.
    pub(crate) fn op_sty_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.y;
        self.store_zp_x(cycle, bus, master, data);
    }

    /// STY Absolute (0x8C) - 4 cycles. No flags affected.
    pub(crate) fn op_sty_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let data = self.y;
        self.store_abs(cycle, bus, master, data);
    }
}
