//! CPU state snapshot types and traits

/// Trait for CPU types that can provide state snapshots
pub trait CpuStateTrait {
    type Snapshot;
    fn snapshot(&self) -> Self::Snapshot;
}

/// M6809 CPU state snapshot
#[derive(Debug, Clone, PartialEq)]
pub struct M6809State {
    pub a: u8,   // Accumulator A
    pub b: u8,   // Accumulator B
    pub dp: u8,  // Direct Page register
    pub x: u16,  // Index register X
    pub y: u16,  // Index register Y
    pub u: u16,  // User stack pointer
    pub s: u16,  // Hardware stack pointer
    pub pc: u16, // Program counter
    pub cc: u8,  // Condition codes
}

/// M6502 CPU state snapshot
#[derive(Debug, Clone, PartialEq)]
pub struct M6502State {
    pub a: u8,   // Accumulator
    pub x: u8,   // X index register
    pub y: u8,   // Y index register
    pub pc: u16, // Program counter
    pub sp: u8,  // Stack pointer (0xFF based)
    pub p: u8,   // Status register (flags)
}

/// M6800 CPU state snapshot
#[derive(Debug, Clone, PartialEq)]
pub struct M6800State {
    pub a: u8,   // Accumulator A
    pub b: u8,   // Accumulator B
    pub x: u16,  // Index register X
    pub sp: u16, // Stack pointer
    pub pc: u16, // Program counter
    pub cc: u8,  // Condition codes
}

/// Z80 CPU state snapshot
#[derive(Debug, Clone, PartialEq)]
pub struct Z80State {
    pub a: u8,        // Accumulator
    pub f: u8,        // Flags register
    pub b: u8,        // Register B
    pub c: u8,        // Register C
    pub d: u8,        // Register D
    pub e: u8,        // Register E
    pub h: u8,        // Register H
    pub l: u8,        // Register L
    pub a_prime: u8,  // Shadow accumulator
    pub f_prime: u8,  // Shadow flags
    pub b_prime: u8,  // Shadow B
    pub c_prime: u8,  // Shadow C
    pub d_prime: u8,  // Shadow D
    pub e_prime: u8,  // Shadow E
    pub h_prime: u8,  // Shadow H
    pub l_prime: u8,  // Shadow L
    pub ix: u16,      // Index register X
    pub iy: u16,      // Index register Y
    pub sp: u16,      // Stack pointer
    pub pc: u16,      // Program counter
    pub i: u8,        // Interrupt vector register
    pub r: u8,        // Memory refresh register
    pub iff1: bool,   // Interrupt flip-flop 1
    pub iff2: bool,   // Interrupt flip-flop 2
    pub im: u8,       // Interrupt mode (0, 1, 2)
    pub memptr: u16,  // Hidden WZ register
    pub p: bool,      // LD A,I/R tracker
    pub q: u8,        // Copy of F when flags modified, 0 otherwise
}
