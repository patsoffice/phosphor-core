use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6502::{M6502, StatusFlag};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` cycles
fn tick(cpu: &mut M6502, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// INC (Increment Memory)
// =============================================================================

#[test]
fn test_inc_zp_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xE6, 0x10]); // INC $10
    bus.memory[0x10] = 0x05;
    tick(&mut cpu, &mut bus, 5); // RMW zp = 5 cycles
    assert_eq!(bus.memory[0x10], 0x06);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_inc_zp_wrap_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xE6, 0x10]); // INC $10
    bus.memory[0x10] = 0xFF;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_inc_zp_to_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xE6, 0x10]); // INC $10
    bus.memory[0x10] = 0x7F;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_inc_zp_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xF6, 0x10]); // INC $10,X → $15
    bus.memory[0x15] = 0x42;
    tick(&mut cpu, &mut bus, 6); // RMW zp,X = 6 cycles
    assert_eq!(bus.memory[0x15], 0x43);
}

#[test]
fn test_inc_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xEE, 0x00, 0x20]); // INC $2000
    bus.memory[0x2000] = 0x09;
    tick(&mut cpu, &mut bus, 6); // RMW abs = 6 cycles
    assert_eq!(bus.memory[0x2000], 0x0A);
}

#[test]
fn test_inc_abs_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x03;
    bus.load(0, &[0xFE, 0x00, 0x20]); // INC $2000,X → $2003
    bus.memory[0x2003] = 0x00;
    tick(&mut cpu, &mut bus, 7); // RMW abs,X = 7 cycles
    assert_eq!(bus.memory[0x2003], 0x01);
}

#[test]
fn test_inc_does_not_modify_a() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0xE6, 0x10]); // INC $10
    bus.memory[0x10] = 0x05;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

// =============================================================================
// DEC (Decrement Memory)
// =============================================================================

#[test]
fn test_dec_zp_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x10]); // DEC $10
    bus.memory[0x10] = 0x05;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x04);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_dec_zp_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x10]); // DEC $10
    bus.memory[0x10] = 0x01;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_dec_zp_wrap_to_ff() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x10]); // DEC $10
    bus.memory[0x10] = 0x00;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_dec_zp_boundary_80_to_7f() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x10]); // DEC $10
    bus.memory[0x10] = 0x80;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x7F);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // 0x7F is positive
}

#[test]
fn test_dec_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xCE, 0x00, 0x20]); // DEC $2000
    bus.memory[0x2000] = 0x10;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x2000], 0x0F);
}

#[test]
fn test_dec_abs_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x02;
    bus.load(0, &[0xDE, 0x00, 0x20]); // DEC $2000,X → $2002
    bus.memory[0x2002] = 0x01;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x2002], 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

// =============================================================================
// INC/DEC round-trip
// =============================================================================

#[test]
fn test_inc_dec_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xE6, 0x10, 0xC6, 0x10]); // INC $10; DEC $10
    bus.memory[0x10] = 0x42;
    tick(&mut cpu, &mut bus, 10); // 5 + 5 cycles
    assert_eq!(bus.memory[0x10], 0x42); // Back to original
}
