use crate::core::{Bus, BusMaster, bus::InterruptState};
use crate::cpu::m6502::M6502;

pub struct Simple6502System {
    pub cpu: M6502,
    ram: [u8; 0x10000], // 64KB RAM
    clock: u64,
}

pub struct CpuState {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub p: u8,
}

impl Simple6502System {
    pub fn new() -> Self {
        Self {
            cpu: M6502::new(),
            ram: [0; 0x10000],
            clock: 0,
        }
    }

    pub fn tick(&mut self) {
        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }
        self.clock += 1;
    }

    pub fn load_program(&mut self, offset: usize, data: &[u8]) {
        if offset + data.len() <= self.ram.len() {
            self.ram[offset..offset + data.len()].copy_from_slice(data);
        }
    }

    pub fn get_cpu_state(&self) -> CpuState {
        CpuState {
            a: self.cpu.a,
            x: self.cpu.x,
            y: self.cpu.y,
            pc: self.cpu.pc,
            sp: self.cpu.sp,
            p: self.cpu.p,
        }
    }
}

impl Bus for Simple6502System {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        self.ram[addr as usize] = data;
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState::default()
    }
}
