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
    pub(crate) temp_addr: u16,
    #[allow(dead_code)]
    resume_delay: u8,  // For TSC/RDY release timing
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8),  // (opcode, cycle)
    #[allow(dead_code)]
    Halted { return_state: Box<ExecState> },
    // ... etc
}

impl M6809 {
    pub fn new() -> Self {
        Self {
            a: 0, b: 0, x: 0, y: 0, u: 0, s: 0, pc: 0, cc: 0,
            state: ExecState::Fetch,
            opcode: 0,
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
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(&mut self, opcode: u8, cycle: u8, bus: &mut B, master: BusMaster) {
        match opcode {
            // ALU instructions (A register inherent)
            0x3D => self.op_mul(cycle),
            0x40 => self.op_nega(cycle),
            0x43 => self.op_coma(cycle),
            0x44 => self.op_lsra(cycle),
            0x46 => self.op_rora(cycle),
            0x47 => self.op_asra(cycle),
            0x48 => self.op_asla(cycle),
            0x49 => self.op_rola(cycle),
            0x4A => self.op_deca(cycle),
            0x4C => self.op_inca(cycle),
            0x4D => self.op_tsta(cycle),
            0x4F => self.op_clra(cycle),
            0x80 => self.op_suba_imm(cycle, bus, master),
            0x81 => self.op_cmpa_imm(cycle, bus, master),
            0x82 => self.op_sbca_imm(cycle, bus, master),
            0x83 => self.op_subd_imm(opcode, cycle, bus, master),
            0x84 => self.op_anda_imm(cycle, bus, master),
            0x85 => self.op_bita_imm(cycle, bus, master),
            0x88 => self.op_eora_imm(cycle, bus, master),
            0x89 => self.op_adca_imm(cycle, bus, master),
            0x8A => self.op_ora_imm(cycle, bus, master),
            0x8B => self.op_adda_imm(cycle, bus, master),

            // ALU instructions (B register inherent)
            0x50 => self.op_negb(cycle),
            0x53 => self.op_comb(cycle),
            0x54 => self.op_lsrb(cycle),
            0x56 => self.op_rorb(cycle),
            0x57 => self.op_asrb(cycle),
            0x58 => self.op_aslb(cycle),
            0x59 => self.op_rolb(cycle),
            0x5A => self.op_decb(cycle),
            0x5C => self.op_incb(cycle),
            0x5D => self.op_tstb(cycle),
            0x5F => self.op_clrb(cycle),
            0xC0 => self.op_subb_imm(cycle, bus, master),
            0xC1 => self.op_cmpb_imm(cycle, bus, master),
            0xC2 => self.op_sbcb_imm(cycle, bus, master),
            0xC3 => self.op_addd_imm(opcode, cycle, bus, master),
            0xC4 => self.op_andb_imm(cycle, bus, master),
            0xC5 => self.op_bitb_imm(cycle, bus, master),
            0xC8 => self.op_eorb_imm(cycle, bus, master),
            0xC9 => self.op_adcb_imm(cycle, bus, master),
            0xCA => self.op_orb_imm(cycle, bus, master),
            0xCB => self.op_addb_imm(cycle, bus, master),

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
