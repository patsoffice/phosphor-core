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
// ADC - Binary mode
// =============================================================================

#[test]
fn test_adc_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x02;
    cpu.p &= !(StatusFlag::C as u8); // Clear carry
    bus.load(0, &[0x69, 0x03]); // ADC #$03
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x05);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::V as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_adc_with_carry_in() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x02;
    cpu.p |= StatusFlag::C as u8; // Set carry
    bus.load(0, &[0x69, 0x03]); // ADC #$03
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x06); // 2 + 3 + 1 = 6
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_adc_carry_out() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x01]); // ADC #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_adc_overflow_positive() {
    // 0x50 + 0x50 = 0xA0 — two positives yield negative → V=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x50]); // ADC #$50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xA0);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_adc_overflow_negative() {
    // 0x80 + 0x80 = 0x100 → A=0x00, two negatives yield positive → V=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x80]); // ADC #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_adc_no_overflow_mixed() {
    // 0x50 + 0xD0 = 0x120 → A=0x20 — positive + negative never overflows
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0xD0]); // ADC #$D0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x20);
    assert_eq!(cpu.p & (StatusFlag::V as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_adc_zero_result() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x00]); // ADC #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_adc_ff_plus_one_with_carry() {
    // 0xFF + 0x00 + C(1) = 0x100 → A=0x00, C=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0x69, 0x00]); // ADC #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

// =============================================================================
// ADC - BCD (Decimal) mode
// =============================================================================

#[test]
fn test_adc_bcd_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x15;
    cpu.p |= StatusFlag::D as u8; // Decimal mode
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x27]); // ADC #$27
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // BCD: 15 + 27 = 42
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_adc_bcd_carry() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x99;
    cpu.p |= StatusFlag::D as u8;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x01]); // ADC #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00); // BCD: 99 + 01 = 100 → 00 with carry
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_adc_bcd_z_flag_from_binary() {
    // NMOS quirk: Z flag is based on binary result, not BCD
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p |= StatusFlag::D as u8;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x69, 0x80]); // ADC #$80
    tick(&mut cpu, &mut bus, 2);
    // Binary: 0x80 + 0x80 = 0x100, low byte = 0x00 → Z=1
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_adc_bcd_with_carry_in() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x58;
    cpu.p |= StatusFlag::D as u8;
    cpu.p |= StatusFlag::C as u8; // Carry in
    bus.load(0, &[0x69, 0x46]); // ADC #$46
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x05); // BCD: 58 + 46 + 1 = 105 → 05 with carry
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

// =============================================================================
// SBC - Binary mode
// =============================================================================

#[test]
fn test_sbc_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x05;
    cpu.p |= StatusFlag::C as u8; // No borrow (C=1 means no borrow)
    bus.load(0, &[0xE9, 0x03]); // SBC #$03
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x02);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8); // No borrow
    assert_eq!(cpu.p & (StatusFlag::V as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_sbc_borrow() {
    // 0x03 - 0x05 = 0xFE, borrow (C=0)
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x03;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0x05]); // SBC #$05
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFE);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // Borrow occurred
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_sbc_with_borrow_in() {
    // 0x05 - 0x03 - borrow(1) = 0x01 when C=0
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x05;
    cpu.p &= !(StatusFlag::C as u8); // Borrow in (C=0)
    bus.load(0, &[0xE9, 0x03]); // SBC #$03
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x01); // 5 - 3 - 1 = 1
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_sbc_overflow() {
    // 0x80 - 0x01 = 0x7F — negative minus positive yields positive → V=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0x01]); // SBC #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_sbc_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x05;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0x05]); // SBC #$05
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8); // No borrow
}

#[test]
fn test_sbc_overflow_positive_minus_negative() {
    // 0x50 - 0xB0 = 0xA0 — positive minus negative yields negative → V=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0xB0]); // SBC #$B0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xA0);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // Borrow
}

// =============================================================================
// SBC - BCD (Decimal) mode
// =============================================================================

#[test]
fn test_sbc_bcd_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.p |= StatusFlag::D as u8;
    cpu.p |= StatusFlag::C as u8; // No borrow
    bus.load(0, &[0xE9, 0x20]); // SBC #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x30); // BCD: 50 - 20 = 30
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_sbc_bcd_borrow() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.p |= StatusFlag::D as u8;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0x21]); // SBC #$21
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x89); // BCD: 10 - 21 = -11 → 89 with borrow
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // Borrow from binary result
}

#[test]
fn test_sbc_bcd_with_nibble_borrow() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x51;
    cpu.p |= StatusFlag::D as u8;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xE9, 0x19]); // SBC #$19
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x32); // BCD: 51 - 19 = 32
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

// =============================================================================
// CMP (Compare Accumulator)
// =============================================================================

#[test]
fn test_cmp_equal() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0xC9, 0x42]); // CMP #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_cmp_greater() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    bus.load(0, &[0xC9, 0x30]); // CMP #$30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_cmp_less() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x30;
    bus.load(0, &[0xC9, 0x50]); // CMP #$50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // Borrow
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_cmp_preserves_a() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0xC9, 0x10]); // CMP #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

#[test]
fn test_cmp_zero_vs_ff() {
    // 0x00 - 0xFF = 0x01 (unsigned), N=0, C=0
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0xC9, 0xFF]); // CMP #$FF
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0); // 0x00 - 0xFF = 0x01
}

#[test]
fn test_cmp_does_not_affect_v() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.p |= StatusFlag::V as u8; // Set V before CMP
    bus.load(0, &[0xC9, 0x01]); // CMP #$01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8); // V preserved
}

// =============================================================================
// CPX (Compare X Register)
// =============================================================================

#[test]
fn test_cpx_equal() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x10;
    bus.load(0, &[0xE0, 0x10]); // CPX #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_cpx_greater() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x20;
    bus.load(0, &[0xE0, 0x10]); // CPX #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_cpx_less() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x10;
    bus.load(0, &[0xE0, 0x20]); // CPX #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

// =============================================================================
// CPY (Compare Y Register)
// =============================================================================

#[test]
fn test_cpy_equal() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x10;
    bus.load(0, &[0xC0, 0x10]); // CPY #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_cpy_greater() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x20;
    bus.load(0, &[0xC0, 0x10]); // CPY #$10
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_cpy_less() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x10;
    bus.load(0, &[0xC0, 0x20]); // CPY #$20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

// =============================================================================
// AND (Logical AND)
// =============================================================================

#[test]
fn test_and_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x29, 0x0F]); // AND #$0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x0F);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_and_zero_result() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF0;
    bus.load(0, &[0x29, 0x0F]); // AND #$0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_and_negative_result() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x29, 0x80]); // AND #$80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_and_ff_identity() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x29, 0xFF]); // AND #$FF
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

// =============================================================================
// ORA (Logical Inclusive OR)
// =============================================================================

#[test]
fn test_ora_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF0;
    bus.load(0, &[0x09, 0x0F]); // ORA #$0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_ora_zero_identity() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x09, 0x00]); // ORA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

#[test]
fn test_ora_zero_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x09, 0x00]); // ORA #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

// =============================================================================
// EOR (Exclusive OR)
// =============================================================================

#[test]
fn test_eor_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x49, 0x0F]); // EOR #$0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xF0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_eor_self_gives_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x49, 0x42]); // EOR #$42
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_eor_zero_identity() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x49, 0x00]); // EOR #$00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

#[test]
fn test_eor_ff_inverts() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    bus.load(0, &[0x49, 0xFF]); // EOR #$FF
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAA);
}

// =============================================================================
// BIT (Bit Test)
// =============================================================================

#[test]
fn test_bit_n_flag_from_memory() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x24, 0x10]); // BIT $10
    bus.memory[0x10] = 0x80; // Bit 7 set
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_bit_v_flag_from_memory() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x24, 0x10]); // BIT $10
    bus.memory[0x10] = 0x40; // Bit 6 set
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_bit_z_flag_set() {
    // A & M == 0 → Z=1
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x24, 0x10]); // BIT $10
    bus.memory[0x10] = 0xF0; // No overlap with A
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    // N and V from memory, not from AND result
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
}

#[test]
fn test_bit_z_flag_clear() {
    // A & M != 0 → Z=0
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x24, 0x10]); // BIT $10
    bus.memory[0x10] = 0x01; // Overlap on bit 0
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_bit_does_not_modify_a() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x24, 0x10]); // BIT $10
    bus.memory[0x10] = 0xFF;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x42); // A unchanged
}

#[test]
fn test_bit_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x2C, 0x00, 0x20]); // BIT $2000
    bus.memory[0x2000] = 0xC0; // Bits 7 and 6 set
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

// =============================================================================
// Addressing mode integration tests (verify wiring through non-imm modes)
// =============================================================================

#[test]
fn test_adc_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.p &= !(StatusFlag::C as u8);
    bus.load(0, &[0x65, 0x20]); // ADC $20
    bus.memory[0x20] = 0x05;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x15);
}

#[test]
fn test_sbc_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.p |= StatusFlag::C as u8;
    bus.load(0, &[0xED, 0x00, 0x20]); // SBC $2000
    bus.memory[0x2000] = 0x20;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_and_abs_x_page_cross() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.x = 0x01;
    bus.load(0, &[0x3D, 0xFF, 0x20]); // AND $20FF,X → $2100
    bus.memory[0x2100] = 0x0F;
    tick(&mut cpu, &mut bus, 5); // Page cross: 5 cycles
    assert_eq!(cpu.a, 0x0F);
}

#[test]
fn test_cmp_ind_y() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.y = 0x03;
    cpu.pc = 0x0200;
    bus.load(0x0200, &[0xD1, 0x40]); // CMP ($40),Y
    bus.memory[0x40] = 0x00;
    bus.memory[0x41] = 0x50;
    // $5000 + $03 = $5003
    bus.memory[0x5003] = 0x42;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_ora_zp_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.x = 0x05;
    bus.load(0, &[0x15, 0x10]); // ORA $10,X → $15
    bus.memory[0x15] = 0xAA;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0xAA);
}

#[test]
fn test_eor_ind_x() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.x = 0x04;
    cpu.pc = 0x0200;
    bus.load(0x0200, &[0x41, 0x20]); // EOR ($20,X) → pointer at $24
    bus.memory[0x24] = 0x00;
    bus.memory[0x25] = 0x40;
    bus.memory[0x4000] = 0x0F;
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.a, 0xF0);
}

#[test]
fn test_cpx_zp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x42;
    bus.load(0, &[0xE4, 0x10]); // CPX $10
    bus.memory[0x10] = 0x42;
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

#[test]
fn test_cpy_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x10;
    bus.load(0, &[0xCC, 0x00, 0x20]); // CPY $2000
    bus.memory[0x2000] = 0x20;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // Y < M
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}
