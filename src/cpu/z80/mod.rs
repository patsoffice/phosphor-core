mod load_store;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::Cpu;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Flag {
    C = 0x01,  // Carry
    N = 0x02,  // Add/Subtract
    PV = 0x04, // Parity/Overflow
    X = 0x08,  // Unused (copy of bit 3)
    H = 0x10,  // Half Carry
    Y = 0x20,  // Unused (copy of bit 5)
    Z = 0x40,  // Zero
    S = 0x80,  // Sign
}

pub struct Z80 {
    // Registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,

    // Internal state
    pub(crate) state: ExecState,
    pub(crate) opcode: u8,
    #[allow(dead_code)]
    pub(crate) temp_addr: u16,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8), // (opcode, cycle)
}

impl Z80 {
    pub fn new() -> Self {
        Self {
            a: 0xFF,
            f: 0xFF,
            b: 0xFF,
            c: 0xFF,
            d: 0xFF,
            e: 0xFF,
            h: 0xFF,
            l: 0xFF,
            sp: 0xFFFF,
            pc: 0x0000,
            state: ExecState::Fetch,
            opcode: 0,
            temp_addr: 0,
        }
    }

    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        match self.state {
            ExecState::Fetch => {
                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 0);
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match opcode {
            // LD A, n (0x3E)
            0x3E => self.op_ld_a_n(cycle, bus, master),
            _ => self.state = ExecState::Fetch,
        }
    }
}

impl Component for Z80 {
    fn tick(&mut self) -> bool {
        false
    }
}

impl BusMasterComponent for Z80 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for Z80 {
    fn reset(&mut self) {
        self.pc = 0;
        self.a = 0xFF;
        self.f = 0xFF;
        self.sp = 0xFFFF;
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {}

    fn is_sleeping(&self) -> bool {
        false
    }
}
