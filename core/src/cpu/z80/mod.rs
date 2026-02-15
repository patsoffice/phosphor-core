mod load_store;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::{
    Cpu,
    state::{CpuStateTrait, Z80State},
};

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
    // Shadow Registers
    pub a_prime: u8,
    pub f_prime: u8,
    pub b_prime: u8,
    pub c_prime: u8,
    pub d_prime: u8,
    pub e_prime: u8,
    pub h_prime: u8,
    pub l_prime: u8,
    // Index & Special Registers
    pub ix: u16,
    pub iy: u16,
    pub i: u8,
    pub r: u8,
    pub sp: u16,
    pub pc: u16,

    // Internal state
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub memptr: u16, // Hidden WZ register
    pub halted: bool,

    pub(crate) state: ExecState,
    pub(crate) opcode: u8,
    #[allow(dead_code)]
    pub(crate) temp_addr: u16,

    // Prefix handling
    pub(crate) index_mode: IndexMode,
    pub(crate) prefix_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IndexMode {
    HL,
    IX,
    IY,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8), // (opcode, cycle)

    // Prefix States
    PrefixCB(u8),
    ExecuteCB(u8, u8),
    PrefixED(u8),
    ExecuteED(u8, u8),
    // Indexed CB (DD CB d o / FD CB d o)
    PrefixIndexCB_ReadOffset(u8),
    PrefixIndexCB_FetchOp(u8),
    ExecuteIndexCB(u8, u8),
}

impl Default for Z80 {
    fn default() -> Self {
        Self::new()
    }
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
            a_prime: 0xFF,
            f_prime: 0xFF,
            b_prime: 0xFF,
            c_prime: 0xFF,
            d_prime: 0xFF,
            e_prime: 0xFF,
            h_prime: 0xFF,
            l_prime: 0xFF,
            ix: 0xFFFF,
            iy: 0xFFFF,
            i: 0,
            r: 0,
            sp: 0xFFFF,
            pc: 0x0000,
            iff1: false,
            iff2: false,
            im: 0,
            memptr: 0,
            halted: false,
            state: ExecState::Fetch,
            opcode: 0,
            temp_addr: 0,
            index_mode: IndexMode::HL,
            prefix_pending: false,
        }
    }

    // Helpers for 16-bit register access
    pub fn get_bc(&self) -> u16 { ((self.b as u16) << 8) | self.c as u16 }
    pub fn set_bc(&mut self, val: u16) { self.b = (val >> 8) as u8; self.c = val as u8; }

    pub fn get_de(&self) -> u16 { ((self.d as u16) << 8) | self.e as u8 as u16 }
    pub fn set_de(&mut self, val: u16) { self.d = (val >> 8) as u8; self.e = val as u8; }

    pub fn get_hl(&self) -> u16 { ((self.h as u16) << 8) | self.l as u8 as u16 }
    pub fn set_hl(&mut self, val: u16) { self.h = (val >> 8) as u8; self.l = val as u8; }

    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        match self.state {
            ExecState::Fetch => {
                // If the previous instruction was NOT a prefix, reset index mode to HL
                if !self.prefix_pending {
                    self.index_mode = IndexMode::HL;
                }
                self.prefix_pending = false;

                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                // Refresh R register (7 bits incremented, bit 7 preserved)
                self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);

                self.state = ExecState::Execute(self.opcode, 0);
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
            ExecState::PrefixCB(cyc) => {
                // Fetch opcode for CB prefix
                if cyc == 0 {
                    self.opcode = bus.read(master, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
                    self.state = ExecState::ExecuteCB(self.opcode, 0);
                }
            }
            ExecState::ExecuteCB(op, cyc) => {
                // TODO: Implement CB instructions
                self.state = ExecState::Fetch;
            }
            ExecState::PrefixED(cyc) => {
                if cyc == 0 {
                    self.opcode = bus.read(master, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
                    self.state = ExecState::ExecuteED(self.opcode, 0);
                }
            }
            ExecState::ExecuteED(op, cyc) => {
                // TODO: Implement ED instructions
                self.state = ExecState::Fetch;
            }
            _ => {
                // TODO: Implement other states
                self.state = ExecState::Fetch;
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
            // Prefixes
            0xCB => {
                // TODO: Handle DD/FD CB (Index+Bit)
                self.state = ExecState::PrefixCB(0);
            }
            0xED => {
                self.index_mode = IndexMode::HL;
                self.state = ExecState::PrefixED(0);
            }
            0xDD => {
                self.index_mode = IndexMode::IX;
                self.prefix_pending = true;
                self.state = ExecState::Fetch;
            }
            0xFD => {
                self.index_mode = IndexMode::IY;
                self.prefix_pending = true;
                self.state = ExecState::Fetch;
            }

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
        self.pc = 0x0000;
        self.i = 0;
        self.r = 0;
        self.im = 0;
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {}

    fn is_sleeping(&self) -> bool {
        false
    }
}

impl CpuStateTrait for Z80 {
    type Snapshot = Z80State;

    fn snapshot(&self) -> Z80State {
        Z80State {
            a: self.a,
            f: self.f,
            b: self.b,
            c: self.c,
            d: self.d,
            e: self.e,
            h: self.h,
            l: self.l,
            sp: self.sp,
            pc: self.pc,
        }
    }
}
