use crate::core::{Bus, BusMaster, bus::InterruptState};
use crate::cpu::m6809::M6809;
use crate::device::pia6820::Pia6820;

pub struct Simple6809System {
    #[allow(dead_code)]
    cpu: M6809,
    ram: [u8; 0x8000],
    rom: [u8; 0x8000],
    pia: Pia6820,

    // Bus arbitration state
    dma_request: bool,
    clock: u64,
}

impl Simple6809System {
    pub fn new() -> Self {
        Self {
            cpu: M6809::new(),
            ram: [0; 0x8000],
            rom: [0; 0x8000],
            pia: Pia6820::new(),
            dma_request: false,
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
        // PIA or other devices can request the bus (assert TSC)
        if self.pia.dma_requested() {
            self.dma_request = true;
        }

        // Execute one CPU cycle manually to avoid borrow checker issues
        if !self.dma_request {
            // Split the borrow: execute_cycle needs &mut M6809 and &mut Bus
            // We need to separate accessing cpu from accessing bus
            let bus_ptr: *mut Self = self;

            unsafe {
                let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
                self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
            }
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
    pub fn get_cpu_state(&self) -> CpuState {
        CpuState {
            a: self.cpu.a,
            b: self.cpu.b,
            dp: self.cpu.dp,
            x: self.cpu.x,
            y: self.cpu.y,
            u: self.cpu.u,
            s: self.cpu.s,
            pc: self.cpu.pc,
            cc: self.cpu.cc,
        }
    }

    /// Set the CPU stack pointer (S register) for testing
    pub fn set_cpu_s(&mut self, val: u16) {
        self.cpu.s = val;
    }

    /// Set the CPU direct page register (DP) for testing
    pub fn set_cpu_dp(&mut self, val: u8) {
        self.cpu.dp = val;
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

/// CPU state snapshot for testing
pub struct CpuState {
    pub a: u8,
    pub b: u8,
    pub dp: u8,
    pub x: u16,
    pub y: u16,
    pub u: u16,
    pub s: u16,
    pub pc: u16,
    pub cc: u8,
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

    fn is_halted_for(&self, master: BusMaster) -> bool {
        // Only CPU 0 can be halted by TSC/DMA in this simple system
        if master == BusMaster::Cpu(0) {
            self.dma_request
        } else {
            false
        }
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            firq: false,
            irq: self.clock % 16667 == 0,
        }
    }
}
