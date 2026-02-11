use phosphor_core::core::{bus::InterruptState, Bus, BusMaster};

/// Minimal bus for testing: flat 64KB read/write memory, no peripherals.
pub struct TestBus {
    pub memory: [u8; 0x10000],
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
        }
    }

    pub fn load(&mut self, addr: u16, data: &[u8]) {
        let start = addr as usize;
        self.memory[start..start + data.len()].copy_from_slice(data);
    }
}

impl Bus for TestBus {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }
    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState::default()
    }
}
