/// Tests for M6800 direct addressing mode (page 0) operations.
///
/// Direct mode: 3 cycles for 8-bit ALU, 4 cycles for 8-bit stores,
/// 4 cycles for 16-bit loads/CPX, 5 cycles for 16-bit stores.
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};

mod common;
use common::TestBus;

fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ---- 8-bit ALU direct ----

#[test]
fn test_suba_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x40;
    bus.memory[0x10] = 0x10;
    bus.load(0, &[0x90, 0x10]); // SUBA $10
    tick(&mut cpu, &mut bus, 3); // 3 cycles
    assert_eq!(cpu.a, 0x30);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_cmpa_dir_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.memory[0x20] = 0x42;
    bus.load(0, &[0x91, 0x20]); // CMPA $20
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x42); // A unchanged
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_sbca_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.cc |= CcFlag::C as u8; // carry set
    bus.memory[0x30] = 0x10;
    bus.load(0, &[0x92, 0x30]); // SBCA $30
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x3F); // 0x50 - 0x10 - 1 = 0x3F
}

#[test]
fn test_anda_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.memory[0x05] = 0x0F;
    bus.load(0, &[0x94, 0x05]); // ANDA $05
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x0F);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0); // V cleared
}

#[test]
fn test_bita_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF0;
    bus.memory[0x05] = 0x0F;
    bus.load(0, &[0x95, 0x05]); // BITA $05
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0xF0); // A unchanged
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // result = 0x00
}

#[test]
fn test_eora_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.memory[0x10] = 0x0F;
    bus.load(0, &[0x98, 0x10]); // EORA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0xF0);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
}

#[test]
fn test_adca_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.cc |= CcFlag::C as u8;
    bus.memory[0x10] = 0x20;
    bus.load(0, &[0x99, 0x10]); // ADCA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x31); // 0x10 + 0x20 + 1
}

#[test]
fn test_oraa_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.memory[0x10] = 0xF0;
    bus.load(0, &[0x9A, 0x10]); // ORAA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0xFF);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_adda_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x20;
    bus.memory[0x10] = 0x30;
    bus.load(0, &[0x9B, 0x10]); // ADDA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x50);
}

#[test]
fn test_adda_dir_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x7F;
    bus.memory[0x10] = 0x01;
    bus.load(0, &[0x9B, 0x10]); // ADDA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x80);
    assert_ne!(cpu.cc & (CcFlag::V as u8), 0); // signed overflow
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

// ---- B register direct ----

#[test]
fn test_subb_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x40;
    bus.memory[0x10] = 0x10;
    bus.load(0, &[0xD0, 0x10]); // SUBB $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.b, 0x30);
}

#[test]
fn test_ldab_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x50] = 0xAB;
    bus.load(0, &[0xD6, 0x50]); // LDAB $50
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.b, 0xAB);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_addb_dir_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFF;
    bus.memory[0x10] = 0x01;
    bus.load(0, &[0xDB, 0x10]); // ADDB $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.b, 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0);
}

// ---- 8-bit Load/Store direct ----

#[test]
fn test_ldaa_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x42] = 0x99;
    bus.load(0, &[0x96, 0x42]); // LDAA $42
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x99);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_staa_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x97, 0x20]); // STAA $20
    tick(&mut cpu, &mut bus, 4); // 4 cycles (store)
    assert_eq!(bus.memory[0x20], 0x42);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_staa_dir_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x97, 0x20]); // STAA $20
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x20], 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_stab_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x99;
    bus.load(0, &[0xD7, 0x30]); // STAB $30
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x30], 0x99);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

// ---- 16-bit Load/Store/Compare direct ----

#[test]
fn test_ldx_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x12;
    bus.memory[0x11] = 0x34;
    bus.load(0, &[0xDE, 0x10]); // LDX $10
    tick(&mut cpu, &mut bus, 4); // 4 cycles
    assert_eq!(cpu.x, 0x1234);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_lds_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x01;
    bus.memory[0x11] = 0xFF;
    bus.load(0, &[0x9E, 0x10]); // LDS $10
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.sp, 0x01FF);
}

#[test]
fn test_stx_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0xABCD;
    bus.load(0, &[0xDF, 0x20]); // STX $20
    tick(&mut cpu, &mut bus, 5); // 1 fetch + 5 execute
    assert_eq!(bus.memory[0x20], 0xAB);
    assert_eq!(bus.memory[0x21], 0xCD);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_sts_dir() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x0100;
    bus.load(0, &[0x9F, 0x20]); // STS $20
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x20], 0x01);
    assert_eq!(bus.memory[0x21], 0x00);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_cpx_dir_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1234;
    bus.memory[0x10] = 0x12;
    bus.memory[0x11] = 0x34;
    bus.load(0, &[0x9C, 0x10]); // CPX $10
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0x1234); // unchanged
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_cpx_dir_greater() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x2000;
    bus.memory[0x10] = 0x10;
    bus.memory[0x11] = 0x00;
    bus.load(0, &[0x9C, 0x10]); // CPX $10
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

// ---- Multi-instruction sequences using direct mode ----

#[test]
fn test_ldaa_adda_staa_direct_sequence() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x20; // operand for LDAA
    bus.memory[0x11] = 0x30; // operand for ADDA
    // LDAA $10; ADDA $11; STAA $12
    bus.load(0, &[0x96, 0x10, 0x9B, 0x11, 0x97, 0x12]);
    tick(&mut cpu, &mut bus, 3); // LDAA $10 (3 cycles)
    assert_eq!(cpu.a, 0x20);
    tick(&mut cpu, &mut bus, 3); // ADDA $11 (3 cycles)
    assert_eq!(cpu.a, 0x50);
    tick(&mut cpu, &mut bus, 4); // STAA $12 (4 cycles)
    assert_eq!(bus.memory[0x12], 0x50);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_ldx_stx_roundtrip_direct() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0xBE;
    bus.memory[0x21] = 0xEF;
    // LDX $20; STX $30
    bus.load(0, &[0xDE, 0x20, 0xDF, 0x30]);
    tick(&mut cpu, &mut bus, 4); // LDX $20 (4 cycles)
    assert_eq!(cpu.x, 0xBEEF);
    tick(&mut cpu, &mut bus, 5); // STX $30 (5 cycles)
    assert_eq!(bus.memory[0x30], 0xBE);
    assert_eq!(bus.memory[0x31], 0xEF);
}
