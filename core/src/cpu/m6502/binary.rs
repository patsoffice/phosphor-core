use super::M6502;
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- ADC (Add with Carry) ----

    /// ADC Immediate (0x69) - 2 cycles
    pub(crate) fn op_adc_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC Zero Page (0x65) - 3 cycles
    pub(crate) fn op_adc_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC Zero Page,X (0x75) - 4 cycles
    pub(crate) fn op_adc_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC Absolute (0x6D) - 4 cycles
    pub(crate) fn op_adc_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC Absolute,X (0x7D) - 4 or 5 cycles
    pub(crate) fn op_adc_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC Absolute,Y (0x79) - 4 or 5 cycles
    pub(crate) fn op_adc_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC (Indirect,X) (0x61) - 6 cycles
    pub(crate) fn op_adc_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    /// ADC (Indirect),Y (0x71) - 5 or 6 cycles
    pub(crate) fn op_adc_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| cpu.perform_adc(op));
    }

    // ---- SBC (Subtract with Carry) ----

    /// SBC Immediate (0xE9) - 2 cycles
    pub(crate) fn op_sbc_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC Zero Page (0xE5) - 3 cycles
    pub(crate) fn op_sbc_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC Zero Page,X (0xF5) - 4 cycles
    pub(crate) fn op_sbc_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC Absolute (0xED) - 4 cycles
    pub(crate) fn op_sbc_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC Absolute,X (0xFD) - 4 or 5 cycles
    pub(crate) fn op_sbc_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC Absolute,Y (0xF9) - 4 or 5 cycles
    pub(crate) fn op_sbc_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC (Indirect,X) (0xE1) - 6 cycles
    pub(crate) fn op_sbc_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    /// SBC (Indirect),Y (0xF1) - 5 or 6 cycles
    pub(crate) fn op_sbc_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| cpu.perform_sbc(op));
    }

    // ---- CMP (Compare Accumulator) ----

    /// CMP Immediate (0xC9) - 2 cycles
    pub(crate) fn op_cmp_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP Zero Page (0xC5) - 3 cycles
    pub(crate) fn op_cmp_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP Zero Page,X (0xD5) - 4 cycles
    pub(crate) fn op_cmp_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP Absolute (0xCD) - 4 cycles
    pub(crate) fn op_cmp_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP Absolute,X (0xDD) - 4 or 5 cycles
    pub(crate) fn op_cmp_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP Absolute,Y (0xD9) - 4 or 5 cycles
    pub(crate) fn op_cmp_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP (Indirect,X) (0xC1) - 6 cycles
    pub(crate) fn op_cmp_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    /// CMP (Indirect),Y (0xD1) - 5 or 6 cycles
    pub(crate) fn op_cmp_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| {
            let a = cpu.a;
            cpu.perform_compare(a, op);
        });
    }

    // ---- AND (Logical AND) ----

    /// AND Immediate (0x29) - 2 cycles
    pub(crate) fn op_and_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND Zero Page (0x25) - 3 cycles
    pub(crate) fn op_and_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND Zero Page,X (0x35) - 4 cycles
    pub(crate) fn op_and_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND Absolute (0x2D) - 4 cycles
    pub(crate) fn op_and_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND Absolute,X (0x3D) - 4 or 5 cycles
    pub(crate) fn op_and_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND Absolute,Y (0x39) - 4 or 5 cycles
    pub(crate) fn op_and_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND (Indirect,X) (0x21) - 6 cycles
    pub(crate) fn op_and_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    /// AND (Indirect),Y (0x31) - 5 or 6 cycles
    pub(crate) fn op_and_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| cpu.perform_and(op));
    }

    // ---- ORA (Logical Inclusive OR) ----

    /// ORA Immediate (0x09) - 2 cycles
    pub(crate) fn op_ora_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA Zero Page (0x05) - 3 cycles
    pub(crate) fn op_ora_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA Zero Page,X (0x15) - 4 cycles
    pub(crate) fn op_ora_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA Absolute (0x0D) - 4 cycles
    pub(crate) fn op_ora_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA Absolute,X (0x1D) - 4 or 5 cycles
    pub(crate) fn op_ora_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA Absolute,Y (0x19) - 4 or 5 cycles
    pub(crate) fn op_ora_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA (Indirect,X) (0x01) - 6 cycles
    pub(crate) fn op_ora_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    /// ORA (Indirect),Y (0x11) - 5 or 6 cycles
    pub(crate) fn op_ora_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| cpu.perform_ora(op));
    }

    // ---- EOR (Exclusive OR) ----

    /// EOR Immediate (0x49) - 2 cycles
    pub(crate) fn op_eor_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR Zero Page (0x45) - 3 cycles
    pub(crate) fn op_eor_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR Zero Page,X (0x55) - 4 cycles
    pub(crate) fn op_eor_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp_x(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR Absolute (0x4D) - 4 cycles
    pub(crate) fn op_eor_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR Absolute,X (0x5D) - 4 or 5 cycles
    pub(crate) fn op_eor_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_x(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR Absolute,Y (0x59) - 4 or 5 cycles
    pub(crate) fn op_eor_abs_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs_y(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR (Indirect,X) (0x41) - 6 cycles
    pub(crate) fn op_eor_ind_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_x(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    /// EOR (Indirect),Y (0x51) - 5 or 6 cycles
    pub(crate) fn op_eor_ind_y<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_ind_y(cycle, bus, master, |cpu, op| cpu.perform_eor(op));
    }

    // ---- BIT (Bit Test) ----

    /// BIT Zero Page (0x24) - 3 cycles
    pub(crate) fn op_bit_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| cpu.perform_bit(op));
    }

    /// BIT Absolute (0x2C) - 4 cycles
    pub(crate) fn op_bit_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| cpu.perform_bit(op));
    }

    // ---- CPX (Compare X Register) ----

    /// CPX Immediate (0xE0) - 2 cycles
    pub(crate) fn op_cpx_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            let x = cpu.x;
            cpu.perform_compare(x, op);
        });
    }

    /// CPX Zero Page (0xE4) - 3 cycles
    pub(crate) fn op_cpx_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            let x = cpu.x;
            cpu.perform_compare(x, op);
        });
    }

    /// CPX Absolute (0xEC) - 4 cycles
    pub(crate) fn op_cpx_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            let x = cpu.x;
            cpu.perform_compare(x, op);
        });
    }

    // ---- CPY (Compare Y Register) ----

    /// CPY Immediate (0xC0) - 2 cycles
    pub(crate) fn op_cpy_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_imm(cycle, bus, master, |cpu, op| {
            let y = cpu.y;
            cpu.perform_compare(y, op);
        });
    }

    /// CPY Zero Page (0xC4) - 3 cycles
    pub(crate) fn op_cpy_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_zp(cycle, bus, master, |cpu, op| {
            let y = cpu.y;
            cpu.perform_compare(y, op);
        });
    }

    /// CPY Absolute (0xCC) - 4 cycles
    pub(crate) fn op_cpy_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.alu_abs(cycle, bus, master, |cpu, op| {
            let y = cpu.y;
            cpu.perform_compare(y, op);
        });
    }
}
