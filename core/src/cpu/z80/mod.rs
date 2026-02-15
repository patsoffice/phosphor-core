mod alu;
mod load_store;
mod stack;

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
    pub(crate) temp_addr: u16,
    pub(crate) temp_data: u8,

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
    Fetch,     // M1 T1: address on bus, reset prefix state
    FetchRead, // M1 T2: read opcode, increment PC, refresh R
    Execute(u8, u8), // (opcode, cycle)

    // Prefix States
    PrefixCB(u8),
    ExecuteCB(u8, u8),
    PrefixED(u8),
    ExecuteED(u8, u8),
    // Indexed CB (DD CB d o / FD CB d o)
    #[allow(non_camel_case_types)]
    PrefixIndexCB_ReadOffset(u8),
    #[allow(non_camel_case_types)]
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
            temp_data: 0,
            index_mode: IndexMode::HL,
            prefix_pending: false,
        }
    }

    // Helpers for 16-bit register access
    pub fn get_bc(&self) -> u16 { ((self.b as u16) << 8) | self.c as u16 }
    pub fn set_bc(&mut self, val: u16) { self.b = (val >> 8) as u8; self.c = val as u8; }

    pub fn get_de(&self) -> u16 { ((self.d as u16) << 8) | self.e as u16 }
    pub fn set_de(&mut self, val: u16) { self.d = (val >> 8) as u8; self.e = val as u8; }

    pub fn get_hl(&self) -> u16 { ((self.h as u16) << 8) | self.l as u16 }
    pub fn set_hl(&mut self, val: u16) { self.h = (val >> 8) as u8; self.l = val as u8; }

    pub fn get_af(&self) -> u16 { ((self.a as u16) << 8) | self.f as u16 }
    pub fn set_af(&mut self, val: u16) { self.a = (val >> 8) as u8; self.f = val as u8; }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: Flag, set: bool) {
        if set { self.f |= flag as u8 } else { self.f &= !(flag as u8) }
    }

    /// Get 8-bit register by index, respecting IX/IY prefix for H/L (undocumented IXH/IXL/IYH/IYL).
    /// Index 6 is NOT handled here — callers must handle (HL)/(IX+d)/(IY+d) separately.
    pub fn get_reg8_ix(&self, index: u8) -> u8 {
        match (index, self.index_mode) {
            (4, IndexMode::IX) => (self.ix >> 8) as u8,
            (5, IndexMode::IX) => self.ix as u8,
            (4, IndexMode::IY) => (self.iy >> 8) as u8,
            (5, IndexMode::IY) => self.iy as u8,
            _ => self.get_reg8(index),
        }
    }

    pub fn set_reg8_ix(&mut self, index: u8, val: u8) {
        match (index, self.index_mode) {
            (4, IndexMode::IX) => self.ix = (self.ix & 0x00FF) | ((val as u16) << 8),
            (5, IndexMode::IX) => self.ix = (self.ix & 0xFF00) | val as u16,
            (4, IndexMode::IY) => self.iy = (self.iy & 0x00FF) | ((val as u16) << 8),
            (5, IndexMode::IY) => self.iy = (self.iy & 0xFF00) | val as u16,
            _ => self.set_reg8(index, val),
        }
    }

    /// Get the effective address for (HL)/(IX+d)/(IY+d).
    /// For IX/IY modes, the displacement must already be stored in temp_data (as signed byte).
    pub(crate) fn get_index_addr(&self) -> u16 {
        match self.index_mode {
            IndexMode::HL => self.get_hl(),
            IndexMode::IX => self.ix.wrapping_add(self.temp_data as i8 as i16 as u16),
            IndexMode::IY => self.iy.wrapping_add(self.temp_data as i8 as i16 as u16),
        }
    }

    /// Get 16-bit register pair by index (0=BC, 1=DE, 2=HL/IX/IY, 3=SP).
    /// Index 2 respects current index_mode for DD/FD prefixed instructions.
    pub(crate) fn get_rp(&self, index: u8) -> u16 {
        match index {
            0 => self.get_bc(),
            1 => self.get_de(),
            2 => match self.index_mode {
                IndexMode::HL => self.get_hl(),
                IndexMode::IX => self.ix,
                IndexMode::IY => self.iy,
            },
            3 => self.sp,
            _ => unreachable!("get_rp called with index {}", index),
        }
    }

    /// Set 16-bit register pair by index (0=BC, 1=DE, 2=HL/IX/IY, 3=SP).
    pub(crate) fn set_rp(&mut self, index: u8, val: u16) {
        match index {
            0 => self.set_bc(val),
            1 => self.set_de(val),
            2 => match self.index_mode {
                IndexMode::HL => self.set_hl(val),
                IndexMode::IX => self.ix = val,
                IndexMode::IY => self.iy = val,
            },
            3 => self.sp = val,
            _ => unreachable!("set_rp called with index {}", index),
        }
    }

    /// Get 16-bit register pair by index for PUSH/POP (0=BC, 1=DE, 2=HL/IX/IY, 3=AF).
    pub(crate) fn get_rp_af(&self, index: u8) -> u16 {
        match index {
            0 => self.get_bc(),
            1 => self.get_de(),
            2 => match self.index_mode {
                IndexMode::HL => self.get_hl(),
                IndexMode::IX => self.ix,
                IndexMode::IY => self.iy,
            },
            3 => self.get_af(),
            _ => unreachable!("get_rp_af called with index {}", index),
        }
    }

    /// Set 16-bit register pair by index for PUSH/POP (0=BC, 1=DE, 2=HL/IX/IY, 3=AF).
    pub(crate) fn set_rp_af(&mut self, index: u8, val: u16) {
        match index {
            0 => self.set_bc(val),
            1 => self.set_de(val),
            2 => match self.index_mode {
                IndexMode::HL => self.set_hl(val),
                IndexMode::IX => self.ix = val,
                IndexMode::IY => self.iy = val,
            },
            3 => self.set_af(val),
            _ => unreachable!("set_rp_af called with index {}", index),
        }
    }

    pub fn get_reg8(&self, index: u8) -> u8 {
        match index {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            7 => self.a,
            _ => unreachable!("get_reg8 called with index {}", index),
        }
    }

    pub fn set_reg8(&mut self, index: u8, val: u8) {
        match index {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => self.h = val,
            5 => self.l = val,
            7 => self.a = val,
            _ => unreachable!("set_reg8 called with index {}", index),
        }
    }

    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        match self.state {
            ExecState::Fetch => {
                // M1 T1: address on bus, reset prefix state
                if !self.prefix_pending {
                    self.index_mode = IndexMode::HL;
                }
                self.prefix_pending = false;
                self.state = ExecState::FetchRead;
            }
            ExecState::FetchRead => {
                // M1 T2: read opcode, increment PC, refresh R
                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
                self.state = ExecState::Execute(self.opcode, 0);
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
            ExecState::PrefixCB(cyc) => {
                // CB prefix M1 fetch: 4 T-states
                // cyc 0 = T1 (address on bus)
                // cyc 1 = T2 (read CB opcode, R refresh)
                // cyc 2 = T3 (internal)
                // cyc 3 → dispatch to ExecuteCB
                match cyc {
                    0 => self.state = ExecState::PrefixCB(1),
                    1 => {
                        self.opcode = bus.read(master, self.pc);
                        self.pc = self.pc.wrapping_add(1);
                        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
                        self.state = ExecState::PrefixCB(2);
                    }
                    2 => self.state = ExecState::PrefixCB(3),
                    3 => self.state = ExecState::ExecuteCB(self.opcode, 0),
                    _ => unreachable!(),
                }
            }
            ExecState::ExecuteCB(_op, _cyc) => {
                // TODO: Implement CB instructions
                self.state = ExecState::Fetch;
            }
            ExecState::PrefixED(cyc) => {
                // ED prefix M1 fetch: 4 T-states
                match cyc {
                    0 => self.state = ExecState::PrefixED(1),
                    1 => {
                        self.opcode = bus.read(master, self.pc);
                        self.pc = self.pc.wrapping_add(1);
                        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
                        self.state = ExecState::PrefixED(2);
                    }
                    2 => self.state = ExecState::PrefixED(3),
                    3 => self.state = ExecState::ExecuteED(self.opcode, 0),
                    _ => unreachable!(),
                }
            }
            ExecState::ExecuteED(_op, _cyc) => {
                // TODO: Implement ED instructions
                self.state = ExecState::Fetch;
            }
            _ => {
                // TODO: Implement other states
                self.state = ExecState::Fetch;
            }
        }
    }

    /// M1 T3 overhead (1 cycle), then dispatch to handlers.
    /// Handlers receive raw cycle numbers starting at 1 (T4 of M1).
    /// Total T-states: Fetch(T1) + FetchRead(T2) + overhead(T3) + handler cycles.
    /// 4T instruction: handler cycle 1 → Fetch (1 handler cycle, total 4).
    /// 7T instruction: handler cycles 1-4 (4 handler cycles, total 7).
    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        // M1 T3: shared overhead (refresh)
        if cycle == 0 {
            self.state = ExecState::Execute(opcode, 1);
            return;
        }

        match opcode {
            // NOP — 4 T: M1 only
            0x00 => self.state = ExecState::Fetch,

            // HALT — 4 T: M1 only
            0x76 => {
                self.halted = true;
                self.pc = self.pc.wrapping_sub(1);
                self.state = ExecState::Fetch;
            }

            // Prefixes — 4 T each (M1 only)
            0xCB => {
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

            // --- Load/Store ---

            // LD (BC), A — 7 T
            0x02 => self.op_ld_bc_a(opcode, cycle, bus, master),
            // LD (DE), A — 7 T
            0x12 => self.op_ld_de_a(opcode, cycle, bus, master),
            // LD (nn), HL — 16 T
            0x22 => self.op_ld_nn_hl(opcode, cycle, bus, master),
            // LD (nn), A — 13 T
            0x32 => self.op_ld_nn_a(opcode, cycle, bus, master),

            // EX AF, AF' — 4 T
            0x08 => self.op_ex_af_af(),

            // LD A, (BC) — 7 T
            0x0A => self.op_ld_a_bc(opcode, cycle, bus, master),
            // LD A, (DE) — 7 T
            0x1A => self.op_ld_a_de(opcode, cycle, bus, master),
            // LD HL, (nn) — 16 T
            0x2A => self.op_ld_hl_nn_ind(opcode, cycle, bus, master),
            // LD A, (nn) — 13 T
            0x3A => self.op_ld_a_nn(opcode, cycle, bus, master),

            // LD rr, nn (0x01/0x11/0x21/0x31) — 10 T
            op if (op & 0xCF) == 0x01 => self.op_ld_rr_nn(op, cycle, bus, master),

            // LD r, n (0x06, 0x0E, ... 0x3E) — 7 T: M1 + MR
            op if (op & 0xC7) == 0x06 => self.op_ld_r_n(op, cycle, bus, master),

            // LD r, r' (0x40-0x7F excluding 0x76) — 4/7 T
            op if (op & 0xC0) == 0x40 => self.op_ld_r_r(op, cycle, bus, master),

            // LD SP, HL — 6 T
            0xF9 => self.op_ld_sp_hl(opcode, cycle, bus, master),

            // EX DE, HL — 4 T
            0xEB => self.op_ex_de_hl(),
            // EXX — 4 T
            0xD9 => self.op_exx(),
            // EX (SP), HL — 19 T
            0xE3 => self.op_ex_sp_hl(opcode, cycle, bus, master),

            // --- Stack ---

            // PUSH rr (0xC5/D5/E5/F5) — 11 T
            op if (op & 0xCF) == 0xC5 => self.op_push(op, cycle, bus, master),
            // POP rr (0xC1/D1/E1/F1) — 10 T
            op if (op & 0xCF) == 0xC1 => self.op_pop(op, cycle, bus, master),

            // --- ALU ---

            // ALU A, r (0x80 - 0xBF) — 4 T (reg) or 7 T ((HL))
            op if (op & 0xC0) == 0x80 => self.op_alu_r(op, cycle, bus, master),
            // ALU A, n (0xC6, 0xCE, ... 0xFE) — 7 T: M1 + MR
            op if (op & 0xC7) == 0xC6 => self.op_alu_n(op, cycle, bus, master),

            // INC r (0x04, 0x0C...) — 4 T (reg) or 11 T ((HL))
            op if (op & 0xC7) == 0x04 => self.op_inc_dec_r(op, cycle, bus, master),
            // DEC r (0x05, 0x0D...) — 4 T (reg) or 11 T ((HL))
            op if (op & 0xC7) == 0x05 => self.op_inc_dec_r(op, cycle, bus, master),

            // ADD HL,rr (0x09/0x19/0x29/0x39) — 11 T
            op if (op & 0xCF) == 0x09 => self.op_add_hl_rr(op, cycle),
            // INC rr (0x03/0x13/0x23/0x33) — 6 T
            op if (op & 0xCF) == 0x03 => self.op_inc_dec_rr(op, cycle),
            // DEC rr (0x0B/0x1B/0x2B/0x3B) — 6 T
            op if (op & 0xCF) == 0x0B => self.op_inc_dec_rr(op, cycle),

            // Accumulator rotates — 4 T
            0x07 => self.op_rlca(),
            0x0F => self.op_rrca(),
            0x17 => self.op_rla(),
            0x1F => self.op_rra(),

            // Misc ALU — 4 T
            0x27 => self.op_daa(),
            0x2F => self.op_cpl(),
            0x37 => self.op_scf(),
            0x3F => self.op_ccf(),

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
        // Instruction boundary: at Fetch AND not mid-prefix (DD/FD set prefix_pending)
        matches!(self.state, ExecState::Fetch) && !self.prefix_pending
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
            a_prime: self.a_prime,
            f_prime: self.f_prime,
            b_prime: self.b_prime,
            c_prime: self.c_prime,
            d_prime: self.d_prime,
            e_prime: self.e_prime,
            h_prime: self.h_prime,
            l_prime: self.l_prime,
            ix: self.ix,
            iy: self.iy,
            sp: self.sp,
            pc: self.pc,
            i: self.i,
            r: self.r,
            iff1: self.iff1,
            iff2: self.iff2,
            im: self.im,
            memptr: self.memptr,
        }
    }
}
