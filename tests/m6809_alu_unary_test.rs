use phosphor_core::machine::simple6809::Simple6809System;
use phosphor_core::cpu::m6809::CcFlag;

#[test]
fn test_negate() {
    let mut sys = Simple6809System::new();
    // LDA #$01, NEGA, LDB #$80, NEGB
    sys.load_rom(0, &[0x86, 0x01, 0x40, 0xC6, 0x80, 0x50]);

    sys.tick(); sys.tick(); // LDA #$01

    // NEGA: 0 - 1 = -1 (0xFF)
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0xFF);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8); // Borrow occurred
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick(); sys.tick(); // LDB #$80 (-128)

    // NEGB: 0 - (-128) = +128 (Overflow!)
    sys.tick(); sys.tick();
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

    sys.tick(); sys.tick(); // LDA #$AA

    // COMA: ~0xAA = 0x55
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x55);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8); // C always set
    assert_eq!(state.cc & (CcFlag::V as u8), 0); // V always clear
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);

    sys.tick(); sys.tick(); // LDB #$00

    // COMB: ~0x00 = 0xFF
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0xFF);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}