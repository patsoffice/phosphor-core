use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` cycles
fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// ADDA immediate (0x8B) - 2 cycles
// =============================================================================

#[test]
fn test_adda_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    bus.load(0, &[0x8B, 0x20]); // ADDA #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x30);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_adda_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x8B, 0x00]); // ADDA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_adda_imm_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x8B, 0x01]); // ADDA #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_adda_imm_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x7F;
    bus.load(0, &[0x8B, 0x01]); // ADDA #$01 → 0x80 (positive overflow)
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_adda_imm_half_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x8B, 0x01]); // ADDA #$01 → half carry from bit 3
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x10);
    assert_eq!(cpu.cc & CcFlag::H as u8, CcFlag::H as u8);
}

// =============================================================================
// ADDB immediate (0xCB) - 2 cycles
// =============================================================================

#[test]
fn test_addb_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    bus.load(0, &[0xCB, 0x20]); // ADDB #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x30);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_addb_imm_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x7F;
    bus.load(0, &[0xCB, 0x01]); // ADDB #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x80);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// ADCA immediate (0x89) - 2 cycles
// =============================================================================

#[test]
fn test_adca_imm_no_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    bus.load(0, &[0x89, 0x20]); // ADCA #$20 (carry clear)
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_adca_imm_with_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.cc = CcFlag::C as u8; // carry set
    bus.load(0, &[0x89, 0x20]); // ADCA #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x31); // 0x10 + 0x20 + 1
}

#[test]
fn test_adca_imm_carry_chain() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.cc = CcFlag::C as u8;
    bus.load(0, &[0x89, 0x00]); // ADCA #$00 with carry
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ADCB immediate (0xC9) - 2 cycles
// =============================================================================

#[test]
fn test_adcb_imm_with_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x40;
    cpu.cc = CcFlag::C as u8;
    bus.load(0, &[0xC9, 0x3F]); // ADCB #$3F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x80); // 0x40 + 0x3F + 1 = 0x80
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// SUBA immediate (0x80) - 2 cycles
// =============================================================================

#[test]
fn test_suba_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x30;
    bus.load(0, &[0x80, 0x10]); // SUBA #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x20);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_suba_imm_zero_result() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x80, 0x42]); // SUBA #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_suba_imm_borrow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x80, 0x01]); // SUBA #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_suba_imm_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    bus.load(0, &[0x80, 0x01]); // SUBA #$01 → 0x7F (negative overflow)
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
}

// =============================================================================
// SUBB immediate (0xC0) - 2 cycles
// =============================================================================

#[test]
fn test_subb_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x50;
    bus.load(0, &[0xC0, 0x30]); // SUBB #$30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x20);
}

// =============================================================================
// SBCA immediate (0x82) - 2 cycles
// =============================================================================

#[test]
fn test_sbca_imm_no_borrow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x30;
    bus.load(0, &[0x82, 0x10]); // SBCA #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x20);
}

#[test]
fn test_sbca_imm_with_borrow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x30;
    cpu.cc = CcFlag::C as u8; // borrow set
    bus.load(0, &[0x82, 0x10]); // SBCA #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x1F); // 0x30 - 0x10 - 1
}

// =============================================================================
// SBCB immediate (0xC2) - 2 cycles
// =============================================================================

#[test]
fn test_sbcb_imm_with_borrow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    cpu.cc = CcFlag::C as u8;
    bus.load(0, &[0xC2, 0x00]); // SBCB #$00 with borrow
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0xFF); // 0x00 - 0x00 - 1
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// CMPA immediate (0x81) - 2 cycles
// =============================================================================

#[test]
fn test_cmpa_imm_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x81, 0x42]); // CMPA #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

#[test]
fn test_cmpa_imm_greater() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    bus.load(0, &[0x81, 0x30]); // CMPA #$30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x50); // A unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

#[test]
fn test_cmpa_imm_less() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    bus.load(0, &[0x81, 0x30]); // CMPA #$30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x10); // A unchanged
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8); // borrow
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// CMPB immediate (0xC1) - 2 cycles
// =============================================================================

#[test]
fn test_cmpb_imm_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x80;
    bus.load(0, &[0xC1, 0x80]); // CMPB #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x80); // B unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ANDA immediate (0x84) - 2 cycles
// =============================================================================

#[test]
fn test_anda_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x84, 0x0F]); // ANDA #$0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x0F);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V always cleared
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
}

#[test]
fn test_anda_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF0;
    bus.load(0, &[0x84, 0x0F]); // ANDA #$0F → 0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ANDB immediate (0xC4) - 2 cycles
// =============================================================================

#[test]
fn test_andb_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xAA;
    bus.load(0, &[0xC4, 0x55]); // ANDB #$55
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// BITA immediate (0x85) - 2 cycles
// =============================================================================

#[test]
fn test_bita_imm_nonzero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x85, 0x80]); // BITA #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF); // A unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_bita_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x85, 0xF0]); // BITA #$F0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x0F); // A unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// BITB immediate (0xC5) - 2 cycles
// =============================================================================

#[test]
fn test_bitb_imm_nonzero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x55;
    bus.load(0, &[0xC5, 0x01]); // BITB #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x55); // B unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
}

// =============================================================================
// EORA immediate (0x88) - 2 cycles
// =============================================================================

#[test]
fn test_eora_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x88, 0xFF]); // EORA #$FF → 0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_eora_imm_toggle_bits() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    bus.load(0, &[0x88, 0x55]); // EORA #$55
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// EORB immediate (0xC8) - 2 cycles
// =============================================================================

#[test]
fn test_eorb_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x0F;
    bus.load(0, &[0xC8, 0x0F]); // EORB #$0F → 0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ORAA immediate (0x8A) - 2 cycles
// =============================================================================

#[test]
fn test_oraa_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x8A, 0xF0]); // ORAA #$F0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_oraa_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x8A, 0x00]); // ORAA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ORAB immediate (0xCA) - 2 cycles
// =============================================================================

#[test]
fn test_orab_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x80;
    bus.load(0, &[0xCA, 0x01]); // ORAB #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x81);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// LDAA immediate (0x86) - 2 cycles
// =============================================================================

#[test]
fn test_ldaa_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x42]); // LDAA #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_ldaa_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x00]); // LDAA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_ldaa_imm_negative() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x80]); // LDAA #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V always cleared
}

// =============================================================================
// LDAB immediate (0xC6) - 2 cycles
// =============================================================================

#[test]
fn test_ldab_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x55]); // LDAB #$55
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x55);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_ldab_imm_clears_v() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::V as u8; // V previously set
    bus.load(0, &[0xC6, 0x42]); // LDAB #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x42);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V cleared
}

// =============================================================================
// CPX immediate (0x8C) - 3 cycles
// =============================================================================

#[test]
fn test_cpx_imm_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1234;
    bus.load(0, &[0x8C, 0x12, 0x34]); // CPX #$1234
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x1234); // X unchanged
    assert_eq!(cpu.pc, 3);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_cpx_imm_greater() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x5000;
    bus.load(0, &[0x8C, 0x30, 0x00]); // CPX #$3000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x5000); // X unchanged
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
}

#[test]
fn test_cpx_imm_less() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1000;
    bus.load(0, &[0x8C, 0x20, 0x00]); // CPX #$2000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x1000); // X unchanged
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_cpx_imm_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x8000;
    bus.load(0, &[0x8C, 0x00, 0x01]); // CPX #$0001 → 0x7FFF (overflow)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
}

#[test]
fn test_cpx_imm_does_not_affect_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0000;
    cpu.cc = CcFlag::C as u8; // C previously set
    bus.load(0, &[0x8C, 0x00, 0x01]); // CPX #$0001 (X < operand)
    tick(&mut cpu, &mut bus, 3);
    // C should remain set from before (not affected by CPX on 6800)
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
}

#[test]
fn test_cpx_imm_does_not_set_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0000;
    cpu.cc = 0; // C clear
    bus.load(0, &[0x8C, 0x00, 0x01]); // CPX #$0001 (X < operand)
    tick(&mut cpu, &mut bus, 3);
    // C should remain clear (not affected by CPX on 6800)
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

// =============================================================================
// LDS immediate (0x8E) - 3 cycles
// =============================================================================

#[test]
fn test_lds_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x8E, 0x01, 0xFF]); // LDS #$01FF
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.sp, 0x01FF);
    assert_eq!(cpu.pc, 3);
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_lds_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x8E, 0x00, 0x00]); // LDS #$0000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.sp, 0x0000);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_lds_imm_negative() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x8E, 0x80, 0x00]); // LDS #$8000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.sp, 0x8000);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V always cleared
}

// =============================================================================
// LDX immediate (0xCE) - 3 cycles
// =============================================================================

#[test]
fn test_ldx_imm_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xCE, 0xAB, 0xCD]); // LDX #$ABCD
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0xABCD);
    assert_eq!(cpu.pc, 3);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_ldx_imm_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xCE, 0x00, 0x00]); // LDX #$0000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x0000);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_ldx_imm_clears_v() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::V as u8; // V previously set
    bus.load(0, &[0xCE, 0x12, 0x34]); // LDX #$1234
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x1234);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V cleared
}

// =============================================================================
// Multi-instruction sequences
// =============================================================================

#[test]
fn test_ldaa_then_adda_sequence() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x10, 0x8B, 0x20]); // LDAA #$10; ADDA #$20
    tick(&mut cpu, &mut bus, 4); // 2 cycles each
    assert_eq!(cpu.a, 0x30);
    assert_eq!(cpu.pc, 4);
}

#[test]
fn test_ldaa_suba_zero_flag() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x42, 0x80, 0x42]); // LDAA #$42; SUBA #$42
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_ldx_cpx_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // LDX #$1234 (3 cycles); CPX #$1234 (3 cycles)
    bus.load(0, &[0xCE, 0x12, 0x34, 0x8C, 0x12, 0x34]);
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.x, 0x1234);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_adda_adcb_carry_chain() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.b = 0x00;
    // ADDA #$01 (generates carry); ADCB #$00 (picks up carry)
    bus.load(0, &[0x8B, 0x01, 0xC9, 0x00]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.b, 0x01); // 0x00 + 0x00 + carry = 0x01
}

// =============================================================================
// Edge cases: signed boundaries
// =============================================================================

#[test]
fn test_adda_imm_neg_plus_neg_no_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // -128
    bus.load(0, &[0x8B, 0xFF]); // ADDA #$FF (-1) → -129 wraps to 0x7F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8); // overflow: neg + neg = pos
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
}

#[test]
fn test_suba_imm_pos_minus_neg_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x7F; // +127
    bus.load(0, &[0x80, 0x80]); // SUBA #$80 (-128) → +255 wraps to 0xFF
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8); // overflow: pos - neg = neg
}
