/// Tests for M6800 extended addressing mode (16-bit absolute) operations.
///
/// Extended mode: 4 cycles for 8-bit ALU, 5 cycles for 8-bit stores,
/// 5 cycles for 16-bit loads/CPX, 6 cycles for 16-bit stores.
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};

mod common;
use common::TestBus;

fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ---- 8-bit ALU extended ----

#[test]
fn test_suba_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    bus.memory[0x1234] = 0x10;
    bus.load(0, &[0xB0, 0x12, 0x34]); // SUBA $1234
    tick(&mut cpu, &mut bus, 4); // 4 cycles
    assert_eq!(cpu.a, 0x40);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_cmpa_ext_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.memory[0x2000] = 0x42;
    bus.load(0, &[0xB1, 0x20, 0x00]); // CMPA $2000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x42); // unchanged
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_sbca_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.cc |= CcFlag::C as u8;
    bus.memory[0x3000] = 0x20;
    bus.load(0, &[0xB2, 0x30, 0x00]); // SBCA $3000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x2F); // 0x50 - 0x20 - 1
}

#[test]
fn test_anda_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    bus.memory[0x4000] = 0x55;
    bus.load(0, &[0xB4, 0x40, 0x00]); // ANDA $4000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_bita_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    bus.memory[0x5000] = 0x80;
    bus.load(0, &[0xB5, 0x50, 0x00]); // BITA $5000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x80); // A unchanged
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // bit 7 set in result
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_eora_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    bus.memory[0x1000] = 0x55;
    bus.load(0, &[0xB8, 0x10, 0x00]); // EORA $1000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xFF);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_adca_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.cc |= CcFlag::C as u8;
    bus.memory[0x1000] = 0x7F;
    bus.load(0, &[0xB9, 0x10, 0x00]); // ADCA $1000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x00); // 0x80 + 0x7F + 1 = 0x100 → 0x00
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // carry
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // zero
}

#[test]
fn test_oraa_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.memory[0x1000] = 0xFF;
    bus.load(0, &[0xBA, 0x10, 0x00]); // ORAA $1000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xFF);
}

#[test]
fn test_adda_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x30;
    bus.memory[0x2000] = 0x40;
    bus.load(0, &[0xBB, 0x20, 0x00]); // ADDA $2000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x70);
}

// ---- B register extended ----

#[test]
fn test_subb_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x40;
    bus.memory[0x1234] = 0x40;
    bus.load(0, &[0xF0, 0x12, 0x34]); // SUBB $1234
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.b, 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_cmpb_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    bus.memory[0x1000] = 0x20;
    bus.load(0, &[0xF1, 0x10, 0x00]); // CMPB $1000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.b, 0x10); // unchanged
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // borrow
}

#[test]
fn test_addb_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    bus.memory[0x8000] = 0x20;
    bus.load(0, &[0xFB, 0x80, 0x00]); // ADDB $8000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.b, 0x30);
}

#[test]
fn test_andb_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFF;
    bus.memory[0x1000] = 0x0F;
    bus.load(0, &[0xF4, 0x10, 0x00]); // ANDB $1000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.b, 0x0F);
}

// ---- 8-bit Load/Store extended ----

#[test]
fn test_ldaa_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x1234] = 0x99;
    bus.load(0, &[0xB6, 0x12, 0x34]); // LDAA $1234
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x99);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_ldab_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x42;
    bus.load(0, &[0xF6, 0xFF, 0xFE]); // LDAB $FFFE
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.b, 0x42);
}

#[test]
fn test_staa_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xDE;
    bus.load(0, &[0xB7, 0x20, 0x00]); // STAA $2000
    tick(&mut cpu, &mut bus, 5); // 5 cycles
    assert_eq!(bus.memory[0x2000], 0xDE);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_stab_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    bus.load(0, &[0xF7, 0x30, 0x00]); // STAB $3000
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x3000], 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

// ---- 16-bit Load/Store/Compare extended ----

#[test]
fn test_ldx_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x2000] = 0xAB;
    bus.memory[0x2001] = 0xCD;
    bus.load(0, &[0xFE, 0x20, 0x00]); // LDX $2000
    tick(&mut cpu, &mut bus, 5); // 5 cycles
    assert_eq!(cpu.x, 0xABCD);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_lds_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x2000] = 0x01;
    bus.memory[0x2001] = 0x00;
    bus.load(0, &[0xBE, 0x20, 0x00]); // LDS $2000
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.sp, 0x0100);
}

#[test]
fn test_stx_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1234;
    bus.load(0, &[0xFF, 0x30, 0x00]); // STX $3000
    tick(&mut cpu, &mut bus, 6); // 6 cycles
    assert_eq!(bus.memory[0x3000], 0x12);
    assert_eq!(bus.memory[0x3001], 0x34);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_sts_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFFFF;
    bus.load(0, &[0xBF, 0x40, 0x00]); // STS $4000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x4000], 0xFF);
    assert_eq!(bus.memory[0x4001], 0xFF);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_cpx_ext_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x5678;
    bus.memory[0x1000] = 0x56;
    bus.memory[0x1001] = 0x78;
    bus.load(0, &[0xBC, 0x10, 0x00]); // CPX $1000
    tick(&mut cpu, &mut bus, 5);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_cpx_ext_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x8000; // -32768 signed
    bus.memory[0x1000] = 0x00;
    bus.memory[0x1001] = 0x01; // operand = 1
    bus.load(0, &[0xBC, 0x10, 0x00]); // CPX $1000
    tick(&mut cpu, &mut bus, 5);
    assert_ne!(cpu.cc & (CcFlag::V as u8), 0); // overflow: -32768 - 1 wraps to +32767
}

// ---- High-address access ----

#[test]
fn test_ldaa_ext_high_addr() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0xFF00] = 0xAA;
    bus.load(0, &[0xB6, 0xFF, 0x00]); // LDAA $FF00
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xAA);
}

// ---- Multi-instruction extended sequences ----

#[test]
fn test_ldaa_adda_staa_extended_sequence() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x1000] = 0x10;
    bus.memory[0x2000] = 0x20;
    // LDAA $1000; ADDA $2000; STAA $3000
    bus.load(0, &[
        0xB6, 0x10, 0x00,  // LDAA $1000
        0xBB, 0x20, 0x00,  // ADDA $2000
        0xB7, 0x30, 0x00,  // STAA $3000
    ]);
    tick(&mut cpu, &mut bus, 4); // LDAA $1000
    assert_eq!(cpu.a, 0x10);
    tick(&mut cpu, &mut bus, 4); // ADDA $2000
    assert_eq!(cpu.a, 0x30);
    tick(&mut cpu, &mut bus, 5); // STAA $3000
    assert_eq!(bus.memory[0x3000], 0x30);
    assert_eq!(cpu.pc, 9);
}

#[test]
fn test_ldx_stx_roundtrip_extended() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x1000] = 0xDE;
    bus.memory[0x1001] = 0xAD;
    // LDX $1000; STX $2000
    bus.load(0, &[
        0xFE, 0x10, 0x00,  // LDX $1000
        0xFF, 0x20, 0x00,  // STX $2000
    ]);
    tick(&mut cpu, &mut bus, 5); // LDX
    assert_eq!(cpu.x, 0xDEAD);
    tick(&mut cpu, &mut bus, 6); // STX
    assert_eq!(bus.memory[0x2000], 0xDE);
    assert_eq!(bus.memory[0x2001], 0xAD);
}

// ---- Cross-mode test: immediate → direct → store ----

#[test]
fn test_cross_mode_imm_dir_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x30; // for ADDA $10
    // LDAA #$20; ADDA $10; STAA $1000
    bus.load(0, &[
        0x86, 0x20,         // LDAA #$20
        0x9B, 0x10,         // ADDA $10
        0xB7, 0x10, 0x00,   // STAA $1000
    ]);
    tick(&mut cpu, &mut bus, 2); // LDAA imm (2 cycles)
    assert_eq!(cpu.a, 0x20);
    tick(&mut cpu, &mut bus, 3); // ADDA dir (3 cycles)
    assert_eq!(cpu.a, 0x50);
    tick(&mut cpu, &mut bus, 5); // STAA ext (5 cycles)
    assert_eq!(bus.memory[0x1000], 0x50);
}
