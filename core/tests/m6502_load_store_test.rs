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
// LDX - Load X Register
// =============================================================================

#[test]
fn test_ldx_imm_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA2, 0x42]); // LDX #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x42);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_ldx_imm_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA2, 0x00]);
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_ldx_imm_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA2, 0x80]);
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_ldx_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA6, 0x10]); // LDX $10
    bus.memory[0x10] = 0x55;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x55);
}

#[test]
fn test_ldx_zp_y() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x05;
    bus.load(0, &[0xB6, 0x10]); // LDX $10,Y
    bus.memory[0x15] = 0x77;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0x77);
}

#[test]
fn test_ldx_zp_y_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x20;
    bus.load(0, &[0xB6, 0xF0]); // LDX $F0,Y — wraps to $10
    bus.memory[0x10] = 0xAA;
    bus.memory[0x0110] = 0xBB; // Wrong address
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0xAA);
}

#[test]
fn test_ldx_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xAE, 0x00, 0x20]); // LDX $2000
    bus.memory[0x2000] = 0x33;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0x33);
}

#[test]
fn test_ldx_abs_y_no_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x05;
    bus.load(0, &[0xBE, 0x00, 0x20]); // LDX $2000,Y
    bus.memory[0x2005] = 0x44;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0x44);
}

#[test]
fn test_ldx_abs_y_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x01;
    bus.load(0, &[0xBE, 0xFF, 0x20]); // LDX $20FF,Y — crosses to $2100
    bus.memory[0x2100] = 0x88;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.x, 0x88);
}

// =============================================================================
// LDY - Load Y Register
// =============================================================================

#[test]
fn test_ldy_imm_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA0, 0x42]); // LDY #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x42);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_ldy_imm_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA0, 0x00]);
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_ldy_imm_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA0, 0xFF]);
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_ldy_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA4, 0x10]); // LDY $10
    bus.memory[0x10] = 0x55;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.y, 0x55);
}

#[test]
fn test_ldy_zp_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xB4, 0x10]); // LDY $10,X
    bus.memory[0x15] = 0x77;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.y, 0x77);
}

#[test]
fn test_ldy_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xAC, 0x00, 0x20]); // LDY $2000
    bus.memory[0x2000] = 0x33;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.y, 0x33);
}

#[test]
fn test_ldy_abs_x_no_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xBC, 0x00, 0x20]); // LDY $2000,X
    bus.memory[0x2005] = 0x44;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.y, 0x44);
}

#[test]
fn test_ldy_abs_x_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x01;
    bus.load(0, &[0xBC, 0xFF, 0x20]); // LDY $20FF,X — crosses to $2100
    bus.memory[0x2100] = 0x88;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.y, 0x88);
}

// =============================================================================
// STA - Store Accumulator
// =============================================================================

#[test]
fn test_sta_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x85, 0x10]); // STA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(bus.memory[0x10], 0x42);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_sta_zp_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    cpu.x = 0x05;
    bus.load(0, &[0x95, 0x10]); // STA $10,X
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x15], 0x55);
}

#[test]
fn test_sta_zp_x_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xBB;
    cpu.x = 0x10;
    bus.load(0, &[0x95, 0xF5]); // STA $F5,X — wraps to $05
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x05], 0xBB);
    assert_eq!(bus.memory[0x0105], 0x00); // Not written here
}

#[test]
fn test_sta_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x77;
    bus.load(0, &[0x8D, 0x00, 0x30]); // STA $3000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x3000], 0x77);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_sta_abs_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xDD;
    cpu.x = 0x05;
    bus.load(0, &[0x9D, 0x00, 0x30]); // STA $3000,X
    tick(&mut cpu, &mut bus, 5); // Always 5 cycles for store
    assert_eq!(bus.memory[0x3005], 0xDD);
}

#[test]
fn test_sta_abs_y() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xEE;
    cpu.y = 0x03;
    bus.load(0, &[0x99, 0x00, 0x30]); // STA $3000,Y
    tick(&mut cpu, &mut bus, 5); // Always 5 cycles for store
    assert_eq!(bus.memory[0x3003], 0xEE);
}

#[test]
fn test_sta_ind_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xCC;
    cpu.x = 0x04;
    bus.load(0, &[0x81, 0x20]); // STA ($20,X) — pointer from $24
    bus.memory[0x24] = 0x00;
    bus.memory[0x25] = 0x40;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x4000], 0xCC);
}

#[test]
fn test_sta_ind_y() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    cpu.y = 0x03;
    bus.load(0, &[0x91, 0x40]); // STA ($40),Y
    bus.memory[0x40] = 0x00;
    bus.memory[0x41] = 0x50;
    tick(&mut cpu, &mut bus, 6); // Always 6 cycles for store
    assert_eq!(bus.memory[0x5003], 0xAA);
}

#[test]
fn test_sta_does_not_affect_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    // Set N flag, clear Z flag manually
    cpu.p |= StatusFlag::N as u8;
    cpu.p &= !(StatusFlag::Z as u8);
    let flags_before = cpu.p;
    bus.load(0, &[0x85, 0x10]); // STA $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p, flags_before); // Flags unchanged
}

// =============================================================================
// STX - Store X Register
// =============================================================================

#[test]
fn test_stx_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x42;
    bus.load(0, &[0x86, 0x10]); // STX $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(bus.memory[0x10], 0x42);
}

#[test]
fn test_stx_zp_y() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x55;
    cpu.y = 0x05;
    bus.load(0, &[0x96, 0x10]); // STX $10,Y
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x15], 0x55);
}

#[test]
fn test_stx_zp_y_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0xCC;
    cpu.y = 0x20;
    bus.load(0, &[0x96, 0xF0]); // STX $F0,Y — wraps to $10
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x10], 0xCC);
}

#[test]
fn test_stx_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x77;
    bus.load(0, &[0x8E, 0x00, 0x30]); // STX $3000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x3000], 0x77);
}

// =============================================================================
// STY - Store Y Register
// =============================================================================

#[test]
fn test_sty_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x42;
    bus.load(0, &[0x84, 0x10]); // STY $10
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(bus.memory[0x10], 0x42);
}

#[test]
fn test_sty_zp_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x55;
    cpu.x = 0x05;
    bus.load(0, &[0x94, 0x10]); // STY $10,X
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x15], 0x55);
}

#[test]
fn test_sty_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x77;
    bus.load(0, &[0x8C, 0x00, 0x30]); // STY $3000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(bus.memory[0x3000], 0x77);
}

// =============================================================================
// Load/Store round-trip tests
// =============================================================================

#[test]
fn test_lda_sta_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // LDA #$42; STA $10; LDA #$00; LDA $10
    bus.load(0, &[0xA9, 0x42, 0x85, 0x10, 0xA9, 0x00, 0xA5, 0x10]);
    tick(&mut cpu, &mut bus, 2); // LDA #$42
    assert_eq!(cpu.a, 0x42);
    tick(&mut cpu, &mut bus, 3); // STA $10
    assert_eq!(bus.memory[0x10], 0x42);
    tick(&mut cpu, &mut bus, 2); // LDA #$00
    assert_eq!(cpu.a, 0x00);
    tick(&mut cpu, &mut bus, 3); // LDA $10
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_ldx_stx_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // LDX #$99; STX $20
    bus.load(0, &[0xA2, 0x99, 0x86, 0x20]);
    tick(&mut cpu, &mut bus, 2); // LDX #$99
    tick(&mut cpu, &mut bus, 3); // STX $20
    assert_eq!(bus.memory[0x20], 0x99);
}

#[test]
fn test_ldy_sty_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // LDY #$BB; STY $30
    bus.load(0, &[0xA0, 0xBB, 0x84, 0x30]);
    tick(&mut cpu, &mut bus, 2); // LDY #$BB
    tick(&mut cpu, &mut bus, 3); // STY $30
    assert_eq!(bus.memory[0x30], 0xBB);
}
