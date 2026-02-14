mod alu;
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
    pub(crate) temp_addr: u16,
    /// Temporary data storage for multi-cycle operations (RMW operand, address bytes)
    pub(crate) temp_data: u8,
    /// Interrupt type being processed: 0=none, 1=NMI, 2=IRQ, 3=BRK
    pub(crate) interrupt_type: u8,
    /// Previous NMI line state for edge detection
    pub(crate) nmi_previous: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8), // (opcode, cycle)
    /// Hardware interrupt response sequence (NMI/IRQ push + vector)
    Interrupt(u8),
}

impl Default for M6502 {
    fn default() -> Self {
        Self::new()
    }
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
            temp_data: 0,
            interrupt_type: 0,
            nmi_previous: false,
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
            ExecState::Interrupt(cycle) => {
                self.execute_interrupt(cycle, bus, master);
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
            // --- LDA ---
            0xA9 => self.op_lda_imm(cycle, bus, master),
            0xA5 => self.op_lda_zp(cycle, bus, master),
            0xB5 => self.op_lda_zp_x(cycle, bus, master),
            0xAD => self.op_lda_abs(cycle, bus, master),
            0xBD => self.op_lda_abs_x(cycle, bus, master),
            0xB9 => self.op_lda_abs_y(cycle, bus, master),
            0xA1 => self.op_lda_ind_x(cycle, bus, master),
            0xB1 => self.op_lda_ind_y(cycle, bus, master),

            // --- LDX ---
            0xA2 => self.op_ldx_imm(cycle, bus, master),
            0xA6 => self.op_ldx_zp(cycle, bus, master),
            0xB6 => self.op_ldx_zp_y(cycle, bus, master),
            0xAE => self.op_ldx_abs(cycle, bus, master),
            0xBE => self.op_ldx_abs_y(cycle, bus, master),

            // --- LDY ---
            0xA0 => self.op_ldy_imm(cycle, bus, master),
            0xA4 => self.op_ldy_zp(cycle, bus, master),
            0xB4 => self.op_ldy_zp_x(cycle, bus, master),
            0xAC => self.op_ldy_abs(cycle, bus, master),
            0xBC => self.op_ldy_abs_x(cycle, bus, master),

            // --- STA ---
            0x85 => self.op_sta_zp(cycle, bus, master),
            0x95 => self.op_sta_zp_x(cycle, bus, master),
            0x8D => self.op_sta_abs(cycle, bus, master),
            0x9D => self.op_sta_abs_x(cycle, bus, master),
            0x99 => self.op_sta_abs_y(cycle, bus, master),
            0x81 => self.op_sta_ind_x(cycle, bus, master),
            0x91 => self.op_sta_ind_y(cycle, bus, master),

            // --- STX ---
            0x86 => self.op_stx_zp(cycle, bus, master),
            0x96 => self.op_stx_zp_y(cycle, bus, master),
            0x8E => self.op_stx_abs(cycle, bus, master),

            // --- STY ---
            0x84 => self.op_sty_zp(cycle, bus, master),
            0x94 => self.op_sty_zp_x(cycle, bus, master),
            0x8C => self.op_sty_abs(cycle, bus, master),

            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    /// Placeholder for interrupt execution (will be implemented in Phase 7)
    fn execute_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        _cycle: u8,
        _bus: &mut B,
        _master: BusMaster,
    ) {
        self.state = ExecState::Fetch;
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
