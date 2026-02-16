use super::M6502;
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- ASL (Arithmetic Shift Left) - Memory modes ----

    /// ASL Zero Page (0x06) - 5 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_asl_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ASL Zero Page,X (0x16) - 6 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_asl_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ASL Absolute (0x0E) - 6 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_asl_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    /// ASL Absolute,X (0x1E) - 7 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_asl_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| cpu.perform_asl(val));
    }

    // ---- LSR (Logical Shift Right) - Memory modes ----

    /// LSR Zero Page (0x46) - 5 cycles. N cleared, Z, C affected. C = old bit 0.
    pub(crate) fn op_lsr_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// LSR Zero Page,X (0x56) - 6 cycles. N cleared, Z, C affected. C = old bit 0.
    pub(crate) fn op_lsr_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// LSR Absolute (0x4E) - 6 cycles. N cleared, Z, C affected. C = old bit 0.
    pub(crate) fn op_lsr_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    /// LSR Absolute,X (0x5E) - 7 cycles. N cleared, Z, C affected. C = old bit 0.
    pub(crate) fn op_lsr_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| cpu.perform_lsr(val));
    }

    // ---- ROL (Rotate Left) - Memory modes ----

    /// ROL Zero Page (0x26) - 5 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_rol_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    /// ROL Zero Page,X (0x36) - 6 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_rol_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    /// ROL Absolute (0x2E) - 6 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_rol_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    /// ROL Absolute,X (0x3E) - 7 cycles. N, Z, C affected. C = old bit 7.
    pub(crate) fn op_rol_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| cpu.perform_rol(val));
    }

    // ---- ROR (Rotate Right) - Memory modes ----

    /// ROR Zero Page (0x66) - 5 cycles. N, Z, C affected. C = old bit 0.
    pub(crate) fn op_ror_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }

    /// ROR Zero Page,X (0x76) - 6 cycles. N, Z, C affected. C = old bit 0.
    pub(crate) fn op_ror_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }

    /// ROR Absolute (0x6E) - 6 cycles. N, Z, C affected. C = old bit 0.
    pub(crate) fn op_ror_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }

    /// ROR Absolute,X (0x7E) - 7 cycles. N, Z, C affected. C = old bit 0.
    pub(crate) fn op_ror_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| cpu.perform_ror(val));
    }
}
