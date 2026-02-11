use super::{ExecState, StatusFlag, M6502};
use crate::core::{Bus, BusMaster};

impl M6502 {
    /// LDA Immediate (0xA9)
    pub(crate) fn op_lda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        if cycle == 0 {
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            self.a = operand;
            self.set_flag(StatusFlag::Z, self.a == 0);
            self.set_flag(StatusFlag::N, self.a & 0x80 != 0);
            self.state = ExecState::Fetch;
        }
    }
}
