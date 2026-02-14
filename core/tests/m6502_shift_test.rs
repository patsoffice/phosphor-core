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
// ASL (Arithmetic Shift Left)
// =============================================================================

#[test]
fn test_asl_acc_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01;
    bus.load(0, &[0x0A]); // ASL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x02);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_asl_acc_carry_out() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // Bit 7 set → carry
    bus.load(0, &[0x0A]); // ASL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_asl_acc_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x40; // Shift into bit 7
    bus.load(0, &[0x0A]); // ASL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_asl_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x06, 0x10]); // ASL $10
    bus.memory[0x10] = 0x55;
    tick(&mut cpu, &mut bus, 5); // RMW zp = 5 cycles
    assert_eq!(bus.memory[0x10], 0xAA);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_asl_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x0E, 0x00, 0x20]); // ASL $2000
    bus.memory[0x2000] = 0x81;
    tick(&mut cpu, &mut bus, 6); // RMW abs = 6 cycles
    assert_eq!(bus.memory[0x2000], 0x02);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_asl_abs_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0x1E, 0x00, 0x20]); // ASL $2000,X
    bus.memory[0x2005] = 0x01;
    tick(&mut cpu, &mut bus, 7); // RMW abs,X = 7 cycles (always)
    assert_eq!(bus.memory[0x2005], 0x02);
}

// =============================================================================
// LSR (Logical Shift Right)
// =============================================================================

#[test]
fn test_lsr_acc_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x04;
    bus.load(0, &[0x4A]); // LSR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x02);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // LSR always clears N
}

#[test]
fn test_lsr_acc_carry_out() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01; // Bit 0 set → carry
    bus.load(0, &[0x4A]); // LSR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_lsr_acc_always_clears_n() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x4A]); // LSR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // N always clear after LSR
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_lsr_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x46, 0x10]); // LSR $10
    bus.memory[0x10] = 0xAA;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x55);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

// =============================================================================
// ROL (Rotate Left)
// =============================================================================

#[test]
fn test_rol_acc_no_carry() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    cpu.p &= !(StatusFlag::C as u8); // C=0
    bus.load(0, &[0x2A]); // ROL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAA); // 0101_0101 << 1 = 1010_1010, bit 0 = 0
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_rol_acc_with_carry_in() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    cpu.p |= StatusFlag::C as u8; // C=1
    bus.load(0, &[0x2A]); // ROL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAB); // 0101_0101 << 1 | 1 = 1010_1011
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_rol_acc_carry_out() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x2A]); // ROL A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_rol_acc_carry_through() {
    // Two ROLs: first shifts bit 7 into C, second shifts C back into bit 0
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x2A, 0x2A]); // ROL A; ROL A
    tick(&mut cpu, &mut bus, 2); // First ROL: A=0x00, C=1
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    tick(&mut cpu, &mut bus, 2); // Second ROL: A=0x01, C=0
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_rol_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::C as u8; // C=1
    bus.load(0, &[0x26, 0x10]); // ROL $10
    bus.memory[0x10] = 0x00;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x01); // 0 rotated left with C=1 → 1
}

// =============================================================================
// ROR (Rotate Right)
// =============================================================================

#[test]
fn test_ror_acc_no_carry() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    cpu.p &= !(StatusFlag::C as u8); // C=0
    bus.load(0, &[0x6A]); // ROR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x55); // 1010_1010 >> 1 = 0101_0101, bit 7 = 0
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_ror_acc_with_carry_in() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    cpu.p |= StatusFlag::C as u8; // C=1
    bus.load(0, &[0x6A]); // ROR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xD5); // 1010_1010 >> 1 | 0x80 = 1101_0101
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_ror_acc_carry_out() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x6A]); // ROR A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_ror_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x66, 0x10]); // ROR $10
    bus.memory[0x10] = 0x02;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(bus.memory[0x10], 0x01);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_ror_abs_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x02;
    cpu.p |= StatusFlag::C as u8; // C=1
    bus.load(0, &[0x7E, 0x00, 0x20]); // ROR $2000,X
    bus.memory[0x2002] = 0x00;
    tick(&mut cpu, &mut bus, 7); // RMW abs,X = 7 cycles
    assert_eq!(bus.memory[0x2002], 0x80); // C rotated into bit 7
}

// =============================================================================
// Accumulator shift cycle count verification
// =============================================================================

#[test]
fn test_acc_shifts_are_2_cycles() {
    let opcodes: &[(u8, &str)] = &[
        (0x0A, "ASL A"),
        (0x4A, "LSR A"),
        (0x2A, "ROL A"),
        (0x6A, "ROR A"),
    ];
    for &(opcode, name) in opcodes {
        let mut cpu = M6502::new();
        let mut bus = TestBus::new();
        cpu.a = 0x42;
        bus.load(0, &[opcode, 0xEA]); // shift; NOP
        tick(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.pc, 1, "{name} should advance PC by 1 (2 cycles)");
    }
}
