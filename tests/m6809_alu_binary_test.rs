use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_add_accumulator_immediate() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x10,       // LDA #$10
        0x8B, 0x20,       // ADDA #$20
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x30, "A should be 0x30 after 0x10 + 0x20");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, 0, "Negative should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, 0, "Overflow should be clear");
    assert_eq!(sys.get_cpu_state().pc, 4, "PC should be at 0x04");
}

#[test]
fn test_add_accumulator_overflow() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0xFF,       // LDA #$FF
        0x8B, 0x01,       // ADDA #$01
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x00, "A should wrap to 0x00");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, CcFlag::C as u8, "Carry should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, CcFlag::Z as u8, "Zero should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, 0, "Negative should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, 0, "Overflow should be clear");
}

#[test]
fn test_add_accumulator_signed_overflow() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x7F,       // LDA #$7F (127)
        0x8B, 0x01,       // ADDA #$01
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x80, "A should be 0x80 (-128)");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, CcFlag::V as u8, "Overflow should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, CcFlag::N as u8, "Negative should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
}

#[test]
fn test_sub_accumulator_immediate() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x30,       // LDA #$30
        0x80, 0x10,       // SUBA #$10
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x20, "A should be 0x20 after 0x30 - 0x10");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, 0, "Negative should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, 0, "Overflow should be clear");
}

#[test]
fn test_sub_accumulator_zero_result() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x42,       // LDA #$42
        0x80, 0x42,       // SUBA #$42
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x00, "A should be 0x00");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, CcFlag::Z as u8, "Zero should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, 0, "Negative should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, 0, "Overflow should be clear");
}

#[test]
fn test_sub_accumulator_borrow() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x10,       // LDA #$10
        0x80, 0x20,       // SUBA #$20
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0xF0, "A should wrap to 0xF0");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, CcFlag::C as u8, "Carry (borrow) should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, CcFlag::N as u8, "Negative should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, 0, "Overflow should be clear");
}

#[test]
fn test_sub_accumulator_signed_overflow() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x80,       // LDA #$80 (-128)
        0x80, 0x01,       // SUBA #$01
    ]);

    for _ in 0..4 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x7F, "A should be 0x7F (127)");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::V as u8, CcFlag::V as u8, "Overflow should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::N as u8, 0, "Negative should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear (no unsigned borrow)");
}

#[test]
fn test_mul_basic() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x03,       // LDA #$03
        0xC6, 0x07,       // LDB #$07
        0x3D,             // MUL
    ]);

    for _ in 0..6 {
        sys.tick();
    }

    // 3 * 7 = 21 (0x0015) -> A=0x00, B=0x15
    assert_eq!(sys.get_cpu_state().a, 0x00, "A (high byte) should be 0x00");
    assert_eq!(sys.get_cpu_state().b, 0x15, "B (low byte) should be 0x15");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear (bit 7 of B is 0)");
}

#[test]
fn test_mul_large_result() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0xFF,       // LDA #$FF
        0xC6, 0xFF,       // LDB #$FF
        0x3D,             // MUL
    ]);

    for _ in 0..6 {
        sys.tick();
    }

    // 255 * 255 = 65025 (0xFE01) -> A=0xFE, B=0x01
    assert_eq!(sys.get_cpu_state().a, 0xFE, "A (high byte) should be 0xFE");
    assert_eq!(sys.get_cpu_state().b, 0x01, "B (low byte) should be 0x01");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear (bit 7 of B is 0)");
}

#[test]
fn test_mul_zero() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x05,       // LDA #$05
        0xC6, 0x00,       // LDB #$00
        0x3D,             // MUL
    ]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x00, "A should be 0x00");
    assert_eq!(sys.get_cpu_state().b, 0x00, "B should be 0x00");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, CcFlag::Z as u8, "Zero should be set");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear");
}

#[test]
fn test_mul_carry_set() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[
        0x86, 0x10,       // LDA #$10
        0xC6, 0x10,       // LDB #$10
        0x3D,             // MUL
    ]);

    for _ in 0..6 {
        sys.tick();
    }

    // 16 * 16 = 256 (0x0100) -> A=0x01, B=0x00
    assert_eq!(sys.get_cpu_state().a, 0x01, "A (high byte) should be 0x01");
    assert_eq!(sys.get_cpu_state().b, 0x00, "B (low byte) should be 0x00");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::C as u8, 0, "Carry should be clear (bit 7 of B is 0)");
    assert_eq!(sys.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");

    // Now test a case where carry IS set: 0x02 * 0xC0 = 0x0180 -> B=0x80
    let mut sys2 = Simple6809System::new();
    sys2.load_rom(0, &[
        0x86, 0x02,       // LDA #$02
        0xC6, 0xC0,       // LDB #$C0
        0x3D,             // MUL
    ]);

    for _ in 0..6 {
        sys2.tick();
    }

    // 2 * 192 = 384 (0x0180) -> A=0x01, B=0x80
    assert_eq!(sys2.get_cpu_state().b, 0x80, "B should be 0x80");
    assert_eq!(sys2.get_cpu_state().cc & CcFlag::C as u8, CcFlag::C as u8, "Carry should be set (bit 7 of B is 1)");
    assert_eq!(sys2.get_cpu_state().cc & CcFlag::Z as u8, 0, "Zero should be clear");
}