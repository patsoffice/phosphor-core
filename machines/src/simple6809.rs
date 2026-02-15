use phosphor_core::core::{Bus, BusMaster, bus::InterruptState};
use phosphor_core::cpu::state::M6809State;
use phosphor_core::cpu::{CpuStateTrait, m6809::M6809};

pub struct Simple6809System {
    #[allow(dead_code)]
    cpu: M6809,
    ram: [u8; 0x8000],
    rom: [u8; 0x8000],
    clock: u64,
}

impl Default for Simple6809System {
    fn default() -> Self {
        Self::new()
    }
}

impl Simple6809System {
    pub fn new() -> Self {
        Self {
            cpu: M6809::new(),
            ram: [0; 0x8000],
            rom: [0; 0x8000],
            clock: 0,
        }
    }

    pub fn run_frame(&mut self) {
        // 1MHz CPU, 60Hz
        for _ in 0..16667 {
            self.tick();
        }
    }

    pub fn tick(&mut self) {
        // Execute one CPU cycle manually to avoid borrow checker issues
        // Split the borrow: execute_cycle needs &mut M6809 and &mut Bus
        let bus_ptr: *mut Self = self;

        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }

        self.clock += 1;
    }

    /// Load code into RAM at the specified address (for testing)
    /// In a real system, this would load ROM, but for testing we load into RAM
    pub fn load_rom(&mut self, offset: usize, data: &[u8]) {
        if offset + data.len() <= self.ram.len() {
            self.ram[offset..offset + data.len()].copy_from_slice(data);
        }
    }

    /// Get a copy of the current CPU state for testing/debugging
    pub fn get_cpu_state(&self) -> M6809State {
        self.cpu.snapshot()
    }

    /// Set the CPU stack pointer (S register) for testing
    pub fn set_cpu_s(&mut self, val: u16) {
        self.cpu.s = val;
    }

    /// Set the CPU direct page register (DP) for testing
    pub fn set_cpu_dp(&mut self, val: u8) {
        self.cpu.dp = val;
    }

    /// Set the CPU Y register for testing
    pub fn set_cpu_y(&mut self, val: u16) {
        self.cpu.y = val;
    }

    /// Set the CPU condition code register (CC) for testing
    pub fn set_cpu_cc(&mut self, val: u8) {
        self.cpu.cc = val;
    }

    /// Read a byte from RAM
    pub fn read_ram(&self, addr: usize) -> u8 {
        if addr < self.ram.len() {
            self.ram[addr]
        } else {
            0
        }
    }

    /// Write a byte to RAM
    pub fn write_ram(&mut self, addr: usize, data: u8) {
        if addr < self.ram.len() {
            self.ram[addr] = data;
        }
    }
}

impl Bus for Simple6809System {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.ram[addr as usize],
            0x8000..=0xFFFF => self.rom[(addr - 0x8000) as usize],
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        if addr < 0x8000 {
            self.ram[addr as usize] = data;
        }
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            firq: false,
            irq: self.clock.is_multiple_of(16667),
        }
    }
}
