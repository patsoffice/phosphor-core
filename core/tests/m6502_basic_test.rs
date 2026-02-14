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
// LDA Immediate (0xA9) - 2 cycles
// =============================================================================

#[test]
fn test_lda_imm_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA9, 0x42]); // LDA #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_lda_imm_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA9, 0x00]); // LDA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_lda_imm_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA9, 0x80]); // LDA #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

// =============================================================================
// LDA Zero Page (0xA5) - 3 cycles
// =============================================================================

#[test]
fn test_lda_zp_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA5, 0x10]); // LDA $10
    bus.memory[0x10] = 0x77;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x77);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_lda_zp_zero_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA5, 0x20]); // LDA $20
    bus.memory[0x20] = 0x00;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

// =============================================================================
// LDA Zero Page,X (0xB5) - 4 cycles
// =============================================================================

#[test]
fn test_lda_zp_x_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xB5, 0x10]); // LDA $10,X
    bus.memory[0x15] = 0x33;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x33);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_lda_zp_x_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x10;
    bus.load(0, &[0xB5, 0xF5]); // LDA $F5,X — should wrap to $05, not $105
    bus.memory[0x05] = 0xAA;
    bus.memory[0x0105] = 0xBB; // Wrong address if no wrap
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xAA); // Must read from $05, not $0105
}

// =============================================================================
// LDA Absolute (0xAD) - 4 cycles
// =============================================================================

#[test]
fn test_lda_abs_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xAD, 0x00, 0x20]); // LDA $2000
    bus.memory[0x2000] = 0x55;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_lda_abs_high_address() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xAD, 0xFF, 0xFF]); // LDA $FFFF
    bus.memory[0xFFFF] = 0xDE;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xDE);
}

// =============================================================================
// LDA Absolute,X (0xBD) - 4 cycles (no page cross), 5 cycles (page cross)
// =============================================================================

#[test]
fn test_lda_abs_x_no_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xBD, 0x00, 0x20]); // LDA $2000,X
    bus.memory[0x2005] = 0x44;
    tick(&mut cpu, &mut bus, 4); // No page cross: 4 cycles
    assert_eq!(cpu.a, 0x44);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_lda_abs_x_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x01;
    bus.load(0, &[0xBD, 0xFF, 0x20]); // LDA $20FF,X — crosses to $2100
    bus.memory[0x2100] = 0x88;
    tick(&mut cpu, &mut bus, 5); // Page cross: 5 cycles
    assert_eq!(cpu.a, 0x88);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_lda_abs_x_page_cross_cycle_count() {
    // Verify that without page cross, instruction completes in 4 cycles
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x01;
    bus.load(0, &[0xBD, 0x00, 0x20]); // LDA $2000,X — no page cross
    bus.memory[0x2001] = 0x42;

    // After 3 cycles, instruction should NOT be complete (still executing)
    tick(&mut cpu, &mut bus, 3);
    assert_ne!(cpu.a, 0x42); // Not yet loaded

    // After 4th cycle, instruction should be complete
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x42);
}

// =============================================================================
// LDA Absolute,Y (0xB9) - 4 cycles (no page cross), 5 cycles (page cross)
// =============================================================================

#[test]
fn test_lda_abs_y_no_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x03;
    bus.load(0, &[0xB9, 0x00, 0x30]); // LDA $3000,Y
    bus.memory[0x3003] = 0x66;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x66);
}

#[test]
fn test_lda_abs_y_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x02;
    bus.load(0, &[0xB9, 0xFF, 0x30]); // LDA $30FF,Y — crosses to $3101
    bus.memory[0x3101] = 0x77;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x77);
}

// =============================================================================
// LDA (Indirect,X) (0xA1) - 6 cycles
// =============================================================================

#[test]
fn test_lda_ind_x_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x04;
    bus.load(0, &[0xA1, 0x20]); // LDA ($20,X) — reads pointer from $24
    // Pointer at $24/$25 -> $4000
    bus.memory[0x24] = 0x00;
    bus.memory[0x25] = 0x40;
    bus.memory[0x4000] = 0xBB;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.a, 0xBB);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_lda_ind_x_zp_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x10;
    bus.load(0, &[0xA1, 0xF5]); // LDA ($F5,X) — $F5+$10=$05 (wraps in ZP)
    // Pointer at $05/$06 -> $1234
    bus.memory[0x05] = 0x34;
    bus.memory[0x06] = 0x12;
    bus.memory[0x1234] = 0xCC;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.a, 0xCC);
}

#[test]
fn test_lda_ind_x_pointer_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x00;
    cpu.pc = 0x0200; // Start code away from zero page
    bus.load(0x0200, &[0xA1, 0xFF]); // LDA ($FF,X) — pointer at $FF/$00 (wraps)
    bus.memory[0xFF] = 0x80;
    bus.memory[0x00] = 0x30; // High byte wraps to $00, not $100
    bus.memory[0x3080] = 0xDD;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.a, 0xDD);
}

// =============================================================================
// LDA (Indirect),Y (0xB1) - 5 cycles (no page cross), 6 cycles (page cross)
// =============================================================================

#[test]
fn test_lda_ind_y_no_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x03;
    bus.load(0, &[0xB1, 0x40]); // LDA ($40),Y
    // Pointer at $40/$41 -> $5000
    bus.memory[0x40] = 0x00;
    bus.memory[0x41] = 0x50;
    // $5000 + $03 = $5003
    bus.memory[0x5003] = 0xEE;
    tick(&mut cpu, &mut bus, 5); // No page cross: 5 cycles
    assert_eq!(cpu.a, 0xEE);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_lda_ind_y_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x01;
    bus.load(0, &[0xB1, 0x40]); // LDA ($40),Y
    // Pointer at $40/$41 -> $50FF
    bus.memory[0x40] = 0xFF;
    bus.memory[0x41] = 0x50;
    // $50FF + $01 = $5100 — page cross!
    bus.memory[0x5100] = 0xFF;
    tick(&mut cpu, &mut bus, 6); // Page cross: 6 cycles
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_lda_ind_y_zp_pointer_wrap() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x00;
    cpu.pc = 0x0200; // Start code away from zero page
    bus.load(0x0200, &[0xB1, 0xFF]); // LDA ($FF),Y — pointer at $FF/$00 (wraps in ZP)
    bus.memory[0xFF] = 0x00;
    bus.memory[0x00] = 0x60; // High byte from $00, not $100
    bus.memory[0x6000] = 0x11;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x11);
}

// =============================================================================
// Edge cases across all modes
// =============================================================================

#[test]
fn test_lda_clears_n_when_loading_positive() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // First load a negative value
    bus.load(0, &[0xA9, 0x80, 0xA9, 0x01]); // LDA #$80; LDA #$01
    tick(&mut cpu, &mut bus, 2); // Execute first LDA
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    tick(&mut cpu, &mut bus, 2); // Execute second LDA
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // N should be cleared
}

#[test]
fn test_lda_clears_z_when_loading_nonzero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // First load zero
    bus.load(0, &[0xA9, 0x00, 0xA9, 0x42]); // LDA #$00; LDA #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0); // Z should be cleared
}

#[test]
fn test_lda_boundary_0x7f() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA9, 0x7F]); // LDA #$7F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // $7F is positive
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_lda_boundary_0xff() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xA9, 0xFF]); // LDA #$FF
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}
