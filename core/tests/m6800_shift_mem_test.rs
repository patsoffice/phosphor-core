/// Tests for M6800 memory shift/rotate operations (indexed and extended addressing).
///
/// RMW cycle counts: indexed = 7 cycles, extended = 6 cycles.
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};

mod common;
use common::TestBus;

fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ---- ASL memory ----

#[test]
fn test_asl_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0x55; // 0101_0101
    bus.load(0, &[0x68, 0x05]); // ASL 5,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0105], 0xAA); // 1010_1010
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // bit 7 was 0
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_asl_ext_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x2000] = 0x80; // 1000_0000
    bus.load(0, &[0x78, 0x20, 0x00]); // ASL $2000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x2000], 0x00);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // bit 7 was 1
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_asl_ext_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x2000] = 0x40; // 0100_0000 → 1000_0000
    bus.load(0, &[0x78, 0x20, 0x00]); // ASL $2000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x2000], 0x80);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
    // V = N XOR C = 1 XOR 0 = 1
    assert_ne!(cpu.cc & (CcFlag::V as u8), 0);
}

// ---- ASR memory ----

#[test]
fn test_asr_idx_positive() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x04; // 0000_0100
    bus.load(0, &[0x67, 0x00]); // ASR 0,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0100], 0x02); // 0000_0010
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_asr_ext_negative() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x3000] = 0x80; // 1000_0000 → 1100_0000 (sign preserved)
    bus.load(0, &[0x77, 0x30, 0x00]); // ASR $3000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x3000], 0xC0);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // still negative
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_asr_idx_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x01; // 0000_0001 → 0000_0000, C=1
    bus.load(0, &[0x67, 0x00]); // ASR 0,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0100], 0x00);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

// ---- LSR memory ----

#[test]
fn test_lsr_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0xFE; // 1111_1110
    bus.load(0, &[0x64, 0x00]); // LSR 0,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0100], 0x7F); // 0111_1111
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0); // N always cleared
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // bit 0 was 0
}

#[test]
fn test_lsr_ext_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x4000] = 0x01; // 0000_0001 → 0000_0000, C=1
    bus.load(0, &[0x74, 0x40, 0x00]); // LSR $4000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x4000], 0x00);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    // V unchanged by right-shift (stays 0 from init)
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

// ---- ROL memory ----

#[test]
fn test_rol_idx_no_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x55; // 0101_0101, C=0
    bus.load(0, &[0x69, 0x00]); // ROL 0,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0100], 0xAA); // 1010_1010
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // bit 7 was 0
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_rol_ext_with_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8; // carry set
    bus.memory[0x2000] = 0x80; // 1000_0000
    bus.load(0, &[0x79, 0x20, 0x00]); // ROL $2000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x2000], 0x01); // old C → bit 0
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // bit 7 was 1
}

// ---- ROR memory ----

#[test]
fn test_ror_idx_no_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0xAA; // 1010_1010, C=0
    bus.load(0, &[0x66, 0x00]); // ROR 0,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0100], 0x55); // 0101_0101
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // bit 0 was 0
}

#[test]
fn test_ror_ext_with_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8;
    bus.memory[0x5000] = 0x01; // 0000_0001, C=1
    bus.load(0, &[0x76, 0x50, 0x00]); // ROR $5000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x5000], 0x80); // old C → bit 7
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // bit 0 was 1
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // bit 7 set from carry-in
}

// ---- Multi-instruction sequences ----

#[test]
fn test_asl_asl_double_shift_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x21; // 0010_0001
    // ASL 0,X; ASL 0,X → shift left by 2
    bus.load(0, &[0x68, 0x00, 0x68, 0x00]);
    tick(&mut cpu, &mut bus, 7); // ASL
    assert_eq!(bus.memory[0x0100], 0x42); // 0100_0010
    tick(&mut cpu, &mut bus, 7); // ASL
    assert_eq!(bus.memory[0x0100], 0x84); // 1000_0100
}

#[test]
fn test_rol_through_carry_ext() {
    // ROL with C=0: 0x80 → 0x00, C=1
    // ROL again: 0x00 → 0x01, C=0 (old carry rotates in)
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x1000] = 0x80;
    bus.load(
        0,
        &[
            0x79, 0x10, 0x00, // ROL $1000
            0x79, 0x10, 0x00, // ROL $1000
        ],
    );
    tick(&mut cpu, &mut bus, 6); // ROL (C=0 in)
    assert_eq!(bus.memory[0x1000], 0x00);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // C=1 out
    tick(&mut cpu, &mut bus, 6); // ROL (C=1 in)
    assert_eq!(bus.memory[0x1000], 0x01);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // C=0 out
}

#[test]
fn test_com_neg_equivalence_idx() {
    // For non-zero x: NEG(x) = COM(x) + 1
    // COM(0x55) = 0xAA, NEG(0x55) = 0xAB
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x55;
    bus.memory[0x0101] = 0x55;
    // COM 0,X; INC 0,X — should equal NEG of original
    bus.load(0, &[0x63, 0x00, 0x6C, 0x00]);
    tick(&mut cpu, &mut bus, 7); // COM 0,X → 0xAA
    assert_eq!(bus.memory[0x0100], 0xAA);
    tick(&mut cpu, &mut bus, 7); // INC 0,X → 0xAB
    assert_eq!(bus.memory[0x0100], 0xAB);

    // Now verify NEG gives the same result
    let mut cpu2 = M6800::new();
    let mut bus2 = TestBus::new();
    cpu2.x = 0x0100;
    bus2.memory[0x0101] = 0x55;
    bus2.load(0, &[0x60, 0x01]); // NEG 1,X
    tick(&mut cpu2, &mut bus2, 7);
    assert_eq!(bus2.memory[0x0101], 0xAB);
}
