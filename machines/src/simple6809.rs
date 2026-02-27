use phosphor_core::bus_split;
use phosphor_core::core::memory_map::{AccessKind, MemoryMap, WatchpointHit, WatchpointKind};
use phosphor_core::core::{Bus, BusMaster, bus::InterruptState};
use phosphor_core::cpu::state::M6809State;
use phosphor_core::cpu::{CpuStateTrait, m6809::M6809};
use phosphor_macros::MemoryRegion;

// Region IDs for Simple6809System address space
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, MemoryRegion)]
enum Region {
    Ram = 1,
    Rom = 2,
}

pub struct Simple6809System {
    #[allow(dead_code)]
    cpu: M6809,
    ram: [u8; 0x8000],
    rom: [u8; 0x8000],
    clock: u64,
    memory_map: MemoryMap,
}

impl Default for Simple6809System {
    fn default() -> Self {
        Self::new()
    }
}

impl Simple6809System {
    pub fn new() -> Self {
        let mut memory_map = MemoryMap::new();
        memory_map
            .region(Region::Ram, "RAM", 0x0000, 0x8000, AccessKind::ReadWrite)
            .region(Region::Rom, "ROM", 0x8000, 0x8000, AccessKind::ReadOnly);

        Self {
            cpu: M6809::new(),
            ram: [0; 0x8000],
            rom: [0; 0x8000],
            clock: 0,
            memory_map,
        }
    }

    pub fn run_frame(&mut self) {
        // 1MHz CPU, 60Hz
        for _ in 0..16667 {
            self.tick();
        }
    }

    pub fn tick(&mut self) {
        bus_split!(self, bus => {
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        });
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

    /// Consume a pending watchpoint hit (for testing)
    pub fn take_watchpoint_hit(&mut self) -> Option<WatchpointHit> {
        self.memory_map.take_hit()
    }

    /// Set a memory watchpoint (for testing)
    pub fn set_watchpoint(&mut self, addr: u16, kind: WatchpointKind) {
        self.memory_map.set_watchpoint(addr, kind);
    }
}

impl Bus for Simple6809System {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        let page = self.memory_map.page(addr);
        let data = match page.region_id {
            Region::RAM => self.ram[addr as usize],
            Region::ROM => self.rom[(addr - 0x8000) as usize],
            _ => 0xFF,
        };
        self.memory_map.check_read_watch(addr, data);
        data
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        if self.memory_map.page(addr).region_id == Region::RAM {
            self.ram[addr as usize] = data;
        }
        self.memory_map.check_write_watch(addr, data);
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&mut self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            firq: false,
            irq: self.clock.is_multiple_of(16667),
            irq_vector: 0,
        }
    }
}
