use super::{ExecState, Z80};
use crate::core::{Bus, BusMaster};

impl Z80 {
    /// LD A, n (0x3E)
    pub(crate) fn op_ld_a_n<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        if cycle == 0 {
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            self.a = operand;
            // No flags affected
            self.state = ExecState::Fetch;
        }
    }
}
