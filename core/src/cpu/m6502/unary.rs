use super::M6502;
use crate::core::{Bus, BusMaster};

impl M6502 {
    // ---- INC (Increment Memory) ----

    /// INC Zero Page (0xE6) - 5 cycles. N, Z affected.
    pub(crate) fn op_inc_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_add(1);
            cpu.set_nz(result);
            result
        });
    }

    /// INC Zero Page,X (0xF6) - 6 cycles. N, Z affected.
    pub(crate) fn op_inc_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_add(1);
            cpu.set_nz(result);
            result
        });
    }

    /// INC Absolute (0xEE) - 6 cycles. N, Z affected.
    pub(crate) fn op_inc_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_add(1);
            cpu.set_nz(result);
            result
        });
    }

    /// INC Absolute,X (0xFE) - 7 cycles. N, Z affected.
    pub(crate) fn op_inc_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_add(1);
            cpu.set_nz(result);
            result
        });
    }

    // ---- DEC (Decrement Memory) ----

    /// DEC Zero Page (0xC6) - 5 cycles. N, Z affected.
    pub(crate) fn op_dec_zp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_sub(1);
            cpu.set_nz(result);
            result
        });
    }

    /// DEC Zero Page,X (0xD6) - 6 cycles. N, Z affected.
    pub(crate) fn op_dec_zp_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_zp_x(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_sub(1);
            cpu.set_nz(result);
            result
        });
    }

    /// DEC Absolute (0xCE) - 6 cycles. N, Z affected.
    pub(crate) fn op_dec_abs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_sub(1);
            cpu.set_nz(result);
            result
        });
    }

    /// DEC Absolute,X (0xDE) - 7 cycles. N, Z affected.
    pub(crate) fn op_dec_abs_x<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.rmw_abs_x(cycle, bus, master, |cpu, val| {
            let result = val.wrapping_sub(1);
            cpu.set_nz(result);
            result
        });
    }
}
