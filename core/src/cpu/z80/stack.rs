use crate::core::{Bus, BusMaster};
use crate::cpu::z80::{ExecState, Z80};

impl Z80 {
    /// PUSH rr — 11 T: M1(4) + internal(1) + MW(3) + MW(3)
    /// Opcode mask: 11 rr0 101 (rr: 0=BC, 1=DE, 2=HL/IX/IY, 3=AF)
    pub fn op_push<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let rp = (opcode >> 4) & 0x03;
        // cycles 1=T4, 2=internal, 3=MW1 write high, 4-5=pad, 6=MW2 write low, 7-8=pad
        match cycle {
            1 | 2 | 4 | 5 | 7 => self.state = ExecState::Execute(opcode, cycle + 1),
            3 => {
                let val = self.get_rp_af(rp);
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, (val >> 8) as u8);
                self.state = ExecState::Execute(opcode, 4);
            }
            6 => {
                let val = self.get_rp_af(rp);
                self.sp = self.sp.wrapping_sub(1);
                bus.write(master, self.sp, val as u8);
                self.state = ExecState::Execute(opcode, 7);
            }
            8 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }

    /// POP rr — 10 T: M1(4) + MR(3) + MR(3)
    /// Opcode mask: 11 rr0 001 (rr: 0=BC, 1=DE, 2=HL/IX/IY, 3=AF)
    pub fn op_pop<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let rp = (opcode >> 4) & 0x03;
        // cycles 1=T4, 2=MR1 read low, 3-4=pad, 5=MR2 read high, 6-7=pad
        match cycle {
            1 | 3 | 4 | 6 => self.state = ExecState::Execute(opcode, cycle + 1),
            2 => {
                self.temp_data = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(opcode, 3);
            }
            5 => {
                let high = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                let val = ((high as u16) << 8) | self.temp_data as u16;
                self.set_rp_af(rp, val);
                self.state = ExecState::Execute(opcode, 6);
            }
            7 => self.state = ExecState::Fetch,
            _ => unreachable!(),
        }
    }
}
