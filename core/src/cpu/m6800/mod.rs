mod alu;
mod branch;
mod load_store;
mod stack;

use std::mem;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::{
    Cpu,
    state::{CpuStateTrait, M6800State},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum CcFlag {
    C = 0x01, // Carry
    V = 0x02, // Overflow
    Z = 0x04, // Zero
    N = 0x08, // Negative
    I = 0x10, // IRQ mask
    H = 0x20, // Half carry
}

pub struct M6800 {
    // Registers
    pub a: u8,
    pub b: u8,
    pub x: u16,
    pub sp: u16,
    pub pc: u16,
    pub cc: u8,

    // Internal state
    pub(crate) state: ExecState,
    pub(crate) opcode: u8,
    pub(crate) temp_addr: u16,
    /// Interrupt type being processed: 0=none, 1=NMI, 2=IRQ, 3=SWI
    pub(crate) interrupt_type: u8,
    /// Previous NMI line state for edge detection
    pub(crate) nmi_previous: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8),      // (opcode, cycle)
    Halted {
        return_state: Box<ExecState>,
    },
    /// Hardware interrupt response sequence (NMI/IRQ push + vector)
    Interrupt(u8),
    /// WAI: all registers pushed, waiting for interrupt
    WaitForInterrupt,
}

impl Default for M6800 {
    fn default() -> Self {
        Self::new()
    }
}

impl M6800 {
    pub fn new() -> Self {
        Self {
            a: 0,
            b: 0,
            x: 0,
            sp: 0,
            pc: 0,
            cc: 0,
            state: ExecState::Fetch,
            opcode: 0,
            temp_addr: 0,
            interrupt_type: 0,
            nmi_previous: false,
        }
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: CcFlag, set: bool) {
        if set {
            self.cc |= flag as u8
        } else {
            self.cc &= !(flag as u8)
        }
    }

    /// Execute one cycle - handles fetch/execute state machine
    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        // Check TSC via the generic bus
        if bus.is_halted_for(master) {
            if !matches!(self.state, ExecState::Halted { .. }) {
                self.state = ExecState::Halted {
                    return_state: Box::new(self.state.clone()),
                };
            }
            return;
        }

        // TSC released — restore the pre-halt state (one dead cycle for re-sync)
        if let ExecState::Halted { .. } = self.state {
            let old = mem::replace(&mut self.state, ExecState::Fetch);
            if let ExecState::Halted { return_state } = old {
                self.state = *return_state;
            }
            return;
        }

        match self.state {
            ExecState::Halted { .. } => unreachable!("handled above"),
            ExecState::Fetch => {
                let ints = bus.check_interrupts(master);
                if self.handle_interrupts(ints) {
                    return;
                }

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
            ExecState::WaitForInterrupt => {
                self.wait_for_interrupt(bus, master);
            }
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        _bus: &mut B,
        _master: BusMaster,
    ) {
        match opcode {
            // NOP (0x01) - 2 cycles total: 1 fetch + 1 internal
            0x01 => {
                if cycle == 0 {
                    self.state = ExecState::Fetch;
                }
            }

            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    /// Check for pending hardware interrupts at instruction boundary.
    /// Returns true if an interrupt is taken.
    /// Priority: NMI (edge-triggered) > IRQ (level, masked by I).
    fn handle_interrupts(&mut self, ints: InterruptState) -> bool {
        // NMI is edge-triggered: detect rising edge
        let nmi_edge = ints.nmi && !self.nmi_previous;
        self.nmi_previous = ints.nmi;

        if nmi_edge {
            self.interrupt_type = 1; // NMI
            self.state = ExecState::Interrupt(0);
            return true;
        }

        // IRQ: level-sensitive, masked by I flag
        if ints.irq && (self.cc & CcFlag::I as u8) == 0 {
            self.interrupt_type = 2; // IRQ
            self.state = ExecState::Interrupt(0);
            return true;
        }

        false
    }
}

impl Component for M6800 {
    fn tick(&mut self) -> bool {
        false
    }
}

impl BusMasterComponent for M6800 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for M6800 {
    fn reset(&mut self) {
        self.pc = 0;
        self.cc = CcFlag::I as u8; // IRQ masked
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {
        // Latch interrupts for sampling at instruction boundary
    }

    fn is_sleeping(&self) -> bool {
        matches!(
            self.state,
            ExecState::Halted { .. } | ExecState::WaitForInterrupt
        )
    }
}

impl CpuStateTrait for M6800 {
    type Snapshot = M6800State;

    fn snapshot(&self) -> M6800State {
        M6800State {
            a: self.a,
            b: self.b,
            x: self.x,
            sp: self.sp,
            pc: self.pc,
            cc: self.cc,
        }
    }
}
