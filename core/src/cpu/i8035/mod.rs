mod alu;
mod branch;
mod load_store;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::{
    Cpu,
    state::{CpuStateTrait, I8035State},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum PswFlag {
    CY = 0x80, // Carry
    AC = 0x40, // Auxiliary carry
    F0 = 0x20, // User flag 0
    BS = 0x10, // Register bank select
}

pub struct I8035 {
    // Registers
    pub a: u8,
    pub pc: u16,
    pub psw: u8,
    pub f1: bool,
    pub t: u8,
    pub dbbb: u8,
    pub p1: u8,
    pub p2: u8,

    // Internal RAM (sized for largest MCS-48 variant; 8035 uses 64 bytes)
    pub ram: [u8; 256],
    pub ram_mask: u8,

    // Memory bank flag
    pub(crate) a11: bool,
    pub(crate) a11_pending: bool,

    // Timer/counter state
    pub(crate) timer_enabled: bool,
    pub(crate) counter_enabled: bool,
    pub(crate) timer_overflow: bool,
    pub(crate) t1_prev: bool,

    // Interrupt state
    pub(crate) int_enabled: bool,
    pub(crate) tcnti_enabled: bool,
    pub(crate) in_interrupt: bool,
    pub(crate) irq_pending: bool,
    pub(crate) timer_irq_pending: bool,

    // Execution state
    pub(crate) state: ExecState,
    pub(crate) opcode: u8,
    pub(crate) temp_data: u8,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    /// Second machine cycle of a 2-cycle instruction
    Execute(u8),
    /// Hardware interrupt entry sequence
    Interrupt(u8),
    /// Halted / idle (not used by standard MCS-48 but reserved)
    Stopped,
}

impl Default for I8035 {
    fn default() -> Self {
        Self::new()
    }
}

impl I8035 {
    pub fn new() -> Self {
        Self {
            a: 0,
            pc: 0,
            psw: 0,
            f1: false,
            t: 0,
            dbbb: 0xFF,
            p1: 0xFF,
            p2: 0xFF,
            ram: [0; 256],
            ram_mask: 0x3F, // 64 bytes for 8035
            a11: false,
            a11_pending: false,
            timer_enabled: false,
            counter_enabled: false,
            timer_overflow: false,
            t1_prev: false,
            int_enabled: false,
            tcnti_enabled: false,
            in_interrupt: false,
            irq_pending: false,
            timer_irq_pending: false,
            state: ExecState::Fetch,
            opcode: 0,
            temp_data: 0,
        }
    }

    // --- Flag helpers ---

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: PswFlag, set: bool) {
        if set {
            self.psw |= flag as u8;
        } else {
            self.psw &= !(flag as u8);
        }
    }

    #[inline]
    pub(crate) fn flag_set(&self, flag: PswFlag) -> bool {
        self.psw & (flag as u8) != 0
    }

    // --- Register access ---

    /// Returns the RAM base address for the active register bank.
    /// Bank 0: 0x00-0x07, Bank 1: 0x18-0x1F.
    #[inline]
    fn reg_bank_offset(&self) -> u8 {
        if self.psw & PswFlag::BS as u8 != 0 {
            0x18
        } else {
            0x00
        }
    }

    #[inline]
    pub(crate) fn get_reg(&self, n: u8) -> u8 {
        let addr = self.reg_bank_offset() + (n & 0x07);
        self.ram[(addr & self.ram_mask) as usize]
    }

    #[inline]
    pub(crate) fn set_reg(&mut self, n: u8, val: u8) {
        let addr = self.reg_bank_offset() + (n & 0x07);
        self.ram[(addr & self.ram_mask) as usize] = val;
    }

    #[inline]
    pub(crate) fn read_ram(&self, addr: u8) -> u8 {
        self.ram[(addr & self.ram_mask) as usize]
    }

    #[inline]
    pub(crate) fn write_ram(&mut self, addr: u8, val: u8) {
        self.ram[(addr & self.ram_mask) as usize] = val;
    }

    // --- Stack ---

    /// Push PC and PSW upper nibble onto the internal stack.
    /// Stack entry format: byte0 = PC[7:0], byte1 = PSW[7:4] | PC[11:8].
    pub(crate) fn push_pc_psw(&mut self) {
        let sp = self.psw & 0x07;
        let addr = 2 * sp + 8;
        self.write_ram(addr, self.pc as u8);
        self.write_ram(
            addr + 1,
            ((self.pc >> 8) as u8 & 0x0F) | (self.psw & 0xF0),
        );
        let new_sp = (sp + 1) & 0x07;
        self.psw = (self.psw & 0xF8) | new_sp;
    }

    /// Pop PC (and optionally PSW flags) from the internal stack.
    /// RET uses restore_psw=false, RETR uses restore_psw=true.
    pub(crate) fn pop_pc_psw(&mut self, restore_psw: bool) {
        let sp = (self.psw & 0x07).wrapping_sub(1) & 0x07;
        self.psw = (self.psw & 0xF8) | sp;
        let addr = 2 * sp + 8;
        let lo = self.read_ram(addr);
        let hi = self.read_ram(addr + 1);
        self.pc = ((hi & 0x0F) as u16) << 8 | lo as u16;
        if restore_psw {
            self.psw = (self.psw & 0x0F) | (hi & 0xF0);
        }
    }

    // --- Timer/Counter ---

    /// Bus I/O address for T1 test pin (counter input).
    const PORT_T1: u16 = 0x111;

    /// Increment the T register; set overflow flag and IRQ pending on wrap.
    fn increment_t(&mut self) {
        let (new_t, overflow) = self.t.overflowing_add(1);
        self.t = new_t;
        if overflow {
            self.timer_overflow = true;
            if self.tcnti_enabled {
                self.timer_irq_pending = true;
            }
        }
    }

    /// Advance timer and/or counter (called every machine cycle).
    /// Timer mode: increments T every cycle.
    /// Counter mode: increments T on T1 falling edge.
    fn tick_timer_counter<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        if self.timer_enabled {
            self.increment_t();
        }
        if self.counter_enabled {
            let t1 = bus.io_read(master, Self::PORT_T1) != 0;
            if self.t1_prev && !t1 {
                self.increment_t();
            }
            self.t1_prev = t1;
        }
    }

    // --- State machine ---

    /// Execute one machine cycle.
    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        match self.state {
            ExecState::Fetch => {
                // Check interrupts at instruction boundary
                let ints = bus.check_interrupts(master);
                if self.handle_interrupts(ints) {
                    self.tick_timer_counter(bus, master);
                    return;
                }

                // Fetch opcode from program memory
                self.opcode = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;

                // Execute cycle 0 (1-cycle ops complete here)
                self.execute_instruction(self.opcode, 0, bus, master);
                self.tick_timer_counter(bus, master);
            }
            ExecState::Execute(op) => {
                // Execute cycle 1 of a 2-cycle instruction
                self.execute_instruction(op, 1, bus, master);
                self.tick_timer_counter(bus, master);
            }
            ExecState::Interrupt(cycle) => {
                self.execute_interrupt(cycle, bus, master);
                self.tick_timer_counter(bus, master);
            }
            ExecState::Stopped => {}
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        _cycle: u8,
        _bus: &mut B,
        _master: BusMaster,
    ) {
        match opcode {
            // NOP (0x00) - 1 machine cycle
            0x00 => {
                self.state = ExecState::Fetch;
            }

            // Unknown/unimplemented opcode - treat as NOP
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    /// Check for pending interrupts at instruction boundary.
    /// Priority: external INT > timer/counter overflow.
    fn handle_interrupts(&mut self, ints: InterruptState) -> bool {
        // External interrupt (level-triggered, masked by int_enabled and in_interrupt)
        if self.int_enabled && !self.in_interrupt && ints.irq {
            self.irq_pending = true;
            self.state = ExecState::Interrupt(0);
            return true;
        }

        // Timer/counter overflow interrupt
        if self.tcnti_enabled && self.timer_irq_pending && !self.in_interrupt {
            self.state = ExecState::Interrupt(0);
            return true;
        }

        false
    }

    fn execute_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        _bus: &mut B,
        _master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Push PC and PSW to internal stack
                self.push_pc_psw();
                self.int_enabled = false;
                self.in_interrupt = true;
                self.state = ExecState::Interrupt(1);
            }
            1 => {
                // Jump to interrupt vector
                if self.irq_pending {
                    self.irq_pending = false;
                    self.pc = 0x003;
                } else if self.timer_irq_pending {
                    self.timer_irq_pending = false;
                    self.pc = 0x007;
                }
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}

impl Component for I8035 {
    fn tick(&mut self) -> bool {
        false
    }
}

impl BusMasterComponent for I8035 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for I8035 {
    fn reset(&mut self) {
        self.pc = 0;
        self.psw = 0;
        self.a11 = false;
        self.a11_pending = false;
        self.timer_enabled = false;
        self.counter_enabled = false;
        self.timer_overflow = false;
        self.int_enabled = false;
        self.tcnti_enabled = false;
        self.in_interrupt = false;
        self.irq_pending = false;
        self.timer_irq_pending = false;
        self.state = ExecState::Fetch;
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {
        // Interrupts are sampled at instruction boundary via check_interrupts
    }

    fn is_sleeping(&self) -> bool {
        matches!(self.state, ExecState::Stopped)
    }
}

impl CpuStateTrait for I8035 {
    type Snapshot = I8035State;

    fn snapshot(&self) -> I8035State {
        I8035State {
            a: self.a,
            pc: self.pc,
            psw: self.psw,
            f1: self.f1,
            t: self.t,
            dbbb: self.dbbb,
            p1: self.p1,
            p2: self.p2,
        }
    }
}
