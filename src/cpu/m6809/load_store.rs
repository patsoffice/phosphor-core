use crate::core::{Bus, BusMaster};
use super::{M6809, CcFlag, ExecState};

impl M6809 {
    /// LDA immediate (0x86)
    pub(crate) fn op_lda_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                self.a = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.set_flag(CcFlag::Z, self.a == 0);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// LDB immediate (0xC6)
    pub(crate) fn op_ldb_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                self.b = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.set_flag(CcFlag::Z, self.b == 0);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// STA direct (0x97)
    pub(crate) fn op_sta_direct<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match cycle {
            0 => {
                // Fetch address
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Store A to memory
                bus.write(master, self.temp_addr, self.a);
                self.set_flag(CcFlag::Z, self.a == 0);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }
}
