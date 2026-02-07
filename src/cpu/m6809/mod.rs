mod alu;
mod load_store;

use crate::core::{Bus, BusMaster, bus::InterruptState, component::{BusMasterComponent, Component}};
use crate::cpu::Cpu;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum CcFlag {
    C = 0x01, // Carry
    V = 0x02, // Overflow
    Z = 0x04, // Zero
    N = 0x08, // Negative
    I = 0x10, // IRQ mask
    H = 0x20, // Half carry
    F = 0x40, // FIRQ mask
    E = 0x80, // Entire
}

pub struct M6809 {
    // Registers (a,b,x,y,u,s,pc,cc)
    pub a: u8, pub b: u8,
    pub x: u16, pub y: u16,
    pub u: u16, pub s: u16,
    pub pc: u16,
    pub cc: u8,

    // Internal state (generic enough to support TSC/RDY logic)
    state: ExecState,
    opcode: u8,
    micro_cycle: u8,
    pub(crate) temp_addr: u16,
    #[allow(dead_code)]
    resume_delay: u8,  // For TSC/RDY release timing
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8),  // (opcode, cycle)
    #[allow(dead_code)]
    Halted { return_state: Box<ExecState>, saved_cycle: u8 },
    // ... etc
}

impl M6809 {
    pub fn new() -> Self {
        Self {
            a: 0, b: 0, x: 0, y: 0, u: 0, s: 0, pc: 0, cc: 0,
            state: ExecState::Fetch,
            opcode: 0,
            micro_cycle: 0,
            temp_addr: 0,
            resume_delay: 0,
        }
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: CcFlag, set: bool) {
        if set { self.cc |= flag as u8 } else { self.cc &= !(flag as u8) }
    }

    /// Execute one cycle - handles fetch/execute state machine
    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, bus: &mut B, master: BusMaster) {
        // Check TSC via the generic bus
        if bus.is_halted_for(master) {
            if !matches!(self.state, ExecState::Halted { .. }) {
                self.state = ExecState::Halted {
                    return_state: Box::new(self.state.clone()),
                    saved_cycle: self.micro_cycle,
                };
            }
            return;
        }

        match self.state {
            ExecState::Halted { .. } => {
                // TSC released? Bus trait handles the logic; we just check again next cycle
            }
            ExecState::Fetch => {
                let ints = bus.check_interrupts(master);
                self.handle_interrupts(ints);

                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 0);
                self.micro_cycle = 0;
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match opcode {
            // ALU instructions
            0x3D => self.op_mul(cycle),
            0x80 => self.op_suba_imm(cycle, bus, master),
            0x8B => self.op_adda_imm(cycle, bus, master),

            // Load/store instructions
            0x86 => self.op_lda_imm(cycle, bus, master),
            0x97 => self.op_sta_direct(opcode, cycle, bus, master),
            0xC6 => self.op_ldb_imm(cycle, bus, master),

            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    fn handle_interrupts(&mut self, ints: InterruptState) {
        // 6809-specific: check FIRQ, IRQ, NMI
        if ints.nmi { /* ... */ }
        if ints.firq && (self.cc & CcFlag::F as u8) == 0 { /* ... */ }
        if ints.irq && (self.cc & CcFlag::I as u8) == 0 { /* ... */ }
    }
}

impl Component for M6809 {
    fn tick(&mut self) -> bool {
        // This would be called for clock-domain only ticks (no bus)
        // For CPUs, we usually use tick_with_bus instead
        false
    }
}

impl BusMasterComponent for M6809 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        // Return true if instruction boundary reached
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for M6809 {
    fn reset(&mut self) {
        self.pc = 0; // Should read vector from FFFE/FFFF via bus later
        self.cc = CcFlag::I as u8 | CcFlag::F as u8; // IRQ/FIRQ masked
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {
        // Latch interrupts for sampling at instruction boundary
    }

    fn is_sleeping(&self) -> bool {
        matches!(self.state, ExecState::Halted { .. })
    }
}
