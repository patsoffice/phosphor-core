mod load_store;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::{
    Cpu,
    state::{CpuStateTrait, M6502State},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum StatusFlag {
    C = 0x01, // Carry
    Z = 0x02, // Zero
    I = 0x04, // Interrupt Disable
    D = 0x08, // Decimal
    B = 0x10, // Break
    U = 0x20, // Unused (always 1)
    V = 0x40, // Overflow
    N = 0x80, // Negative
}

pub struct M6502 {
    // Registers
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub p: u8,

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

impl M6502 {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            p: 0x24, // I=1, U=1
            state: ExecState::Fetch,
            opcode: 0,
            temp_addr: 0,
        }
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: StatusFlag, set: bool) {
        if set {
            self.p |= flag as u8;
        } else {
            self.p &= !(flag as u8);
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
            // LDA Immediate
            0xA9 => self.op_lda_imm(cycle, bus, master),
            _ => self.state = ExecState::Fetch,
        }
    }
}

impl Component for M6502 {
    fn tick(&mut self) -> bool {
        false
    }
}

impl BusMasterComponent for M6502 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for M6502 {
    fn reset(&mut self) {
        self.pc = 0;
        self.sp = 0xFD;
        self.p = 0x24;
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {}

    fn is_sleeping(&self) -> bool {
        false
    }
}

impl CpuStateTrait for M6502 {
    type Snapshot = M6502State;

    fn snapshot(&self) -> M6502State {
        M6502State {
            a: self.a,
            x: self.x,
            y: self.y,
            pc: self.pc,
            sp: self.sp,
            p: self.p,
        }
    }
}
