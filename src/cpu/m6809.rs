use crate::core::{Bus, BusMaster, bus::InterruptState, component::{BusMasterComponent, Component}};
use crate::cpu::Cpu;

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
    temp_addr: u16,
    #[allow(dead_code)]
    resume_delay: u8,  // For TSC/RDY release timing
}

#[derive(Clone, Debug)]
enum ExecState {
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
            // LDA immediate (0x86)
            0x86 => {
                match cycle {
                    0 => {
                        // Fetch operand
                        self.a = bus.read(master, self.pc);
                        self.pc = self.pc.wrapping_add(1);
                        // Update condition codes (simplified)
                        self.cc = if self.a == 0 { self.cc | 0x04 } else { self.cc & !0x04 };
                        self.state = ExecState::Fetch;
                    }
                    _ => {}
                }
            }
            // STA direct (0x97)
            0x97 => {
                match cycle {
                    0 => {
                        // Fetch address
                        let addr = bus.read(master, self.pc) as u16;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = addr;
                        self.state = ExecState::Execute(opcode, 1);
                    }
                    1 => {
                        // Store A to memory
                        bus.write(master, self.temp_addr, self.a);
                        // Update condition codes
                        self.cc = if self.a == 0 { self.cc | 0x04 } else { self.cc & !0x04 };
                        self.state = ExecState::Fetch;
                    }
                    _ => {}
                }
            }
            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }
    
    fn handle_interrupts(&mut self, ints: InterruptState) {
        // 6809-specific: check FIRQ, IRQ, NMI
        if ints.nmi { /* ... */ }
        if ints.firq && (self.cc & 0x40) == 0 { /* ... */ }
        if ints.irq && (self.cc & 0x10) == 0 { /* ... */ }
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
        self.cc = 0x50; // IRQ/FIRQ masked
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {
        // Latch interrupts for sampling at instruction boundary
    }

    fn is_sleeping(&self) -> bool {
        matches!(self.state, ExecState::Halted { .. })
    }
}
