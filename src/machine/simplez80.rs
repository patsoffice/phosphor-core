use crate::core::{Bus, BusMaster, bus::InterruptState};
use crate::cpu::z80::Z80;

pub struct SimpleZ80System {
    pub cpu: Z80,
    ram: [u8; 0x10000], // 64KB RAM
    clock: u64,
}

pub struct CpuState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl SimpleZ80System {
    pub fn new() -> Self {
        Self {
            cpu: Z80::new(),
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
            f: self.cpu.f,
            b: self.cpu.b,
            c: self.cpu.c,
            d: self.cpu.d,
            e: self.cpu.e,
            h: self.cpu.h,
            l: self.cpu.l,
            sp: self.cpu.sp,
            pc: self.cpu.pc,
        }
    }
}

impl Bus for SimpleZ80System {
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
