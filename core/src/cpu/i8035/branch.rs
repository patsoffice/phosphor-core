use super::{ExecState, I8035, PswFlag};
use crate::core::{Bus, BusMaster};

/// Bus I/O address conventions for MCS-48 test pins.
const PORT_T0: u16 = 0x110;
const PORT_T1: u16 = 0x111;

impl I8035 {
    // === Helpers ===

    /// Cycle 1 of any conditional jump: read address byte, branch if temp_data != 0.
    fn jump_if_temp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let page = self.pc & 0xF00;
        let addr = bus.read(master, self.pc);
        self.pc = (self.pc + 1) & 0x0FFF;
        if self.temp_data != 0 {
            self.pc = page | addr as u16;
        }
        self.state = ExecState::Fetch;
    }

    // === Unconditional jumps ===

    /// JMP addr11 (0x04/0x24/0x44/0x64/0x84/0xA4/0xC4/0xE4): Jump to 12-bit address. 2 cycles.
    /// Target = (A11 << 11) | (opcode[7:5] << 8) | addr_byte.
    pub(crate) fn op_jmp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            _ => {
                let addr_byte = bus.read(master, self.pc);
                self.a11 = self.a11_pending;
                self.pc = (if self.a11 { 0x800u16 } else { 0 })
                    | ((self.opcode as u16 & 0xE0) << 3)
                    | addr_byte as u16;
                self.state = ExecState::Fetch;
            }
        }
    }

    /// CALL addr11 (0x14/0x34/0x54/0x74/0x94/0xB4/0xD4/0xF4): Call subroutine. 2 cycles.
    /// Pushes return address (PC after 2-byte instruction) and PSW, then jumps.
    pub(crate) fn op_call<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            _ => {
                let addr_byte = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.push_pc_psw();
                self.a11 = self.a11_pending;
                self.pc = (if self.a11 { 0x800u16 } else { 0 })
                    | ((self.opcode as u16 & 0xE0) << 3)
                    | addr_byte as u16;
                self.state = ExecState::Fetch;
            }
        }
    }

    /// JMPP @A (0xB3): Indirect jump via program memory lookup. 2 cycles.
    /// PC <- (current_page | program_memory[current_page | A]).
    pub(crate) fn op_jmpp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            _ => {
                let page = self.pc & 0xF00;
                self.pc = page | bus.read(master, page | self.a as u16) as u16;
                self.state = ExecState::Fetch;
            }
        }
    }

    // === Returns ===

    /// RET (0x83): Return from subroutine (pop PC, don't restore PSW). 2 cycles.
    pub(crate) fn op_ret(&mut self, cycle: u8) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            _ => {
                self.pop_pc_psw(false);
                self.state = ExecState::Fetch;
            }
        }
    }

    /// RETR (0x93): Return from interrupt (pop PC and restore PSW). 2 cycles.
    /// Clears in_interrupt flag to re-enable interrupt acceptance.
    pub(crate) fn op_retr(&mut self, cycle: u8) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            _ => {
                self.pop_pc_psw(true);
                self.in_interrupt = false;
                self.state = ExecState::Fetch;
            }
        }
    }

    // === Decrement and jump ===

    /// DJNZ Rn,addr (0xE8-0xEF): Decrement register, jump if non-zero. 2 cycles.
    pub(crate) fn op_djnz<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        n: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let val = Self::perform_dec(self.get_reg(n));
                self.set_reg(n, val);
                self.temp_data = (val != 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    // === Conditional jumps (flag-based, no bus side effects) ===

    /// JC addr (0xF6): Jump if carry set. 2 cycles.
    pub(crate) fn op_jc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = self.flag_set(PswFlag::CY) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JNC addr (0xE6): Jump if carry clear. 2 cycles.
    pub(crate) fn op_jnc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (!self.flag_set(PswFlag::CY)) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JZ addr (0x96): Jump if accumulator is zero. 2 cycles.
    pub(crate) fn op_jz<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (self.a == 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JNZ addr (0xA6): Jump if accumulator is non-zero. 2 cycles.
    pub(crate) fn op_jnz<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (self.a != 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JF0 addr (0xB6): Jump if F0 flag set. 2 cycles.
    pub(crate) fn op_jf0<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = self.flag_set(PswFlag::F0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JF1 addr (0x76): Jump if F1 flag set. 2 cycles.
    pub(crate) fn op_jf1<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = self.f1 as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    // === Conditional jumps (pin/interrupt tests, require bus access) ===

    /// JT0 addr (0x26): Jump if T0 pin is high. 2 cycles.
    pub(crate) fn op_jt0<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (bus.io_read(master, PORT_T0) != 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JNT0 addr (0x46): Jump if T0 pin is low. 2 cycles.
    pub(crate) fn op_jnt0<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (bus.io_read(master, PORT_T0) == 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JT1 addr (0x36): Jump if T1 pin is high. 2 cycles.
    pub(crate) fn op_jt1<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (bus.io_read(master, PORT_T1) != 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JNT1 addr (0x56): Jump if T1 pin is low. 2 cycles.
    pub(crate) fn op_jnt1<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = (bus.io_read(master, PORT_T1) == 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JTF addr (0x16): Jump if timer overflow flag set, then clear it. 2 cycles.
    pub(crate) fn op_jtf<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = self.timer_overflow as u8;
                self.timer_overflow = false;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    /// JNI addr (0x86): Jump if INT pin is asserted (low). 2 cycles.
    pub(crate) fn op_jni<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                let ints = bus.check_interrupts(master);
                self.temp_data = ints.irq as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }

    // === Bit test jumps ===

    /// JBb addr (0x12/0x32/0x52/0x72/0x92/0xB2/0xD2/0xF2): Jump if bit b of A is set. 2 cycles.
    pub(crate) fn op_jbb<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bit: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.temp_data = ((self.a >> bit) & 1 != 0) as u8;
                self.state = ExecState::Execute(self.opcode);
            }
            _ => self.jump_if_temp(bus, master),
        }
    }
}
