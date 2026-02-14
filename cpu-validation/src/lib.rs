use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::{Bus, BusMaster};
use serde::{Deserialize, Serialize};

// --- TracingBus: flat 64KB memory with cycle-by-cycle recording ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BusOp {
    Read,
    Write,
    Internal,
}

#[derive(Clone, Debug)]
pub struct BusCycle {
    pub addr: u16,
    pub data: u8,
    pub op: BusOp,
}

pub struct TracingBus {
    pub memory: [u8; 0x10000],
    pub cycles: Vec<BusCycle>,
}

impl TracingBus {
    pub fn new() -> Self {
        Self {
            memory: [0; 0x10000],
            cycles: Vec::new(),
        }
    }

    pub fn load(&mut self, addr: u16, data: &[u8]) {
        let start = addr as usize;
        self.memory[start..start + data.len()].copy_from_slice(data);
    }

    pub fn clear_cycles(&mut self) {
        self.cycles.clear();
    }
}

impl Default for TracingBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for TracingBus {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        let data = self.memory[addr as usize];
        self.cycles.push(BusCycle {
            addr,
            data,
            op: BusOp::Read,
        });
        data
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
        self.cycles.push(BusCycle {
            addr,
            data,
            op: BusOp::Write,
        });
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState::default()
    }
}

// --- JSON test vector types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub initial: CpuState,
    #[serde(rename = "final")]
    pub final_state: CpuState,
    pub cycles: Vec<(u16, u8, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuState {
    pub pc: u16,
    pub s: u16,
    pub u: u16,
    pub a: u8,
    pub b: u8,
    pub dp: u8,
    pub x: u16,
    pub y: u16,
    pub cc: u8,
    pub ram: Vec<(u16, u8)>,
}

// --- M6502 JSON test vector types (SingleStepTests/65x02 format) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M6502TestCase {
    pub name: String,
    pub initial: M6502CpuState,
    #[serde(rename = "final")]
    pub final_state: M6502CpuState,
    pub cycles: Vec<(u16, u8, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M6502CpuState {
    pub pc: u16,
    pub s: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub ram: Vec<(u16, u8)>,
}

// --- M6800 JSON test vector types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M6800TestCase {
    pub name: String,
    pub initial: M6800CpuState,
    #[serde(rename = "final")]
    pub final_state: M6800CpuState,
    pub cycles: Vec<(u16, u8, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M6800CpuState {
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub b: u8,
    pub x: u16,
    pub cc: u8,
    pub ram: Vec<(u16, u8)>,
}
