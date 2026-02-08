use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_negate() {
    let mut sys = Simple6809System::new();
    // LDA #$01, NEGA, LDB #$80, NEGB
    sys.load_rom(0, &[0x86, 0x01, 0x40, 0xC6, 0x80, 0x50]);

    sys.tick();
    sys.tick(); // LDA #$01

    // NEGA: 0 - 1 = -1 (0xFF)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0xFF);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8); // Borrow occurred
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick();
    sys.tick(); // LDB #$80 (-128)

    // NEGB: 0 - (-128) = +128 (Overflow!)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x80); // Result is still 0x80
    assert_eq!(state.cc & (CcFlag::V as u8), CcFlag::V as u8); // Overflow set
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_complement() {
    let mut sys = Simple6809System::new();
    // LDA #$AA, COMA, LDB #$00, COMB
    sys.load_rom(0, &[0x86, 0xAA, 0x43, 0xC6, 0x00, 0x53]);

    sys.tick();
    sys.tick(); // LDA #$AA

    // COMA: ~0xAA = 0x55
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x55);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8); // C always set
    assert_eq!(state.cc & (CcFlag::V as u8), 0); // V always clear
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick();
    sys.tick(); // LDB #$00

    // COMB: ~0x00 = 0xFF
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0xFF);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_clear() {
    let mut sys = Simple6809System::new();
    // LDA #$FF, CLRA, LDB #$42, CLRB
    sys.load_rom(0, &[0x86, 0xFF, 0x4F, 0xC6, 0x42, 0x5F]);

    sys.tick();
    sys.tick(); // LDA #$FF

    // CLRA: A = 0
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x00);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);

    sys.tick();
    sys.tick(); // LDB #$42

    // CLRB: B = 0
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x00);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_increment() {
    let mut sys = Simple6809System::new();
    // LDA #$7F, INCA, LDB #$FF, INCB
    sys.load_rom(0, &[0x86, 0x7F, 0x4C, 0xC6, 0xFF, 0x5C]);

    sys.tick();
    sys.tick(); // LDA #$7F

    // INCA: 0x7F + 1 = 0x80 (signed overflow: positive -> negative)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x80);
    assert_eq!(
        state.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "Overflow should be set (0x7F -> 0x80)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick();
    sys.tick(); // LDB #$FF

    // INCB: 0xFF + 1 = 0x00 (wraps to zero)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x00);
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(
        state.cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear (0xFF -> 0x00 is not signed overflow)"
    );
}

#[test]
fn test_decrement() {
    let mut sys = Simple6809System::new();
    // LDA #$80, DECA, LDB #$01, DECB
    sys.load_rom(0, &[0x86, 0x80, 0x4A, 0xC6, 0x01, 0x5A]);

    sys.tick();
    sys.tick(); // LDA #$80

    // DECA: 0x80 - 1 = 0x7F (signed overflow: negative -> positive)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x7F);
    assert_eq!(
        state.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "Overflow should be set (0x80 -> 0x7F)"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick();
    sys.tick(); // LDB #$01

    // DECB: 0x01 - 1 = 0x00
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x00);
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_test_register() {
    let mut sys = Simple6809System::new();
    // LDA #$80, TSTA, LDB #$00, TSTB
    sys.load_rom(0, &[0x86, 0x80, 0x4D, 0xC6, 0x00, 0x5D]);

    sys.tick();
    sys.tick(); // LDA #$80

    // TSTA: test A (0x80 is negative, not zero)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x80, "A should be unchanged");
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(state.cc & (CcFlag::V as u8), 0, "Overflow always clear");

    sys.tick();
    sys.tick(); // LDB #$00

    // TSTB: test B (0x00 is zero, not negative)
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x00, "B should be unchanged");
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(state.cc & (CcFlag::V as u8), 0, "Overflow always clear");
}
