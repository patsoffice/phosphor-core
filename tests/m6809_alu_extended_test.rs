use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_adda_extended() {
    let mut sys = Simple6809System::new();
    // LDA #$20
    // ADDA $1000
    sys.load_rom(
        0,
        &[
            0x86, 0x20, // LDA #$20
            0xBB, 0x10, 0x00, // ADDA $1000
        ],
    );
    sys.write_ram(0x1000, 0x30);

    // Run 6 cycles (2 for LDA, 4 for ADDA)
    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x50);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_subb_extended() {
    let mut sys = Simple6809System::new();
    // LDB #$50
    // SUBB $0500
    sys.load_rom(
        0,
        &[
            0xC6, 0x50, // LDB #$50
            0xF0, 0x05, 0x00, // SUBB $0500
        ],
    );
    sys.write_ram(0x0500, 0x10);

    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x40);
}

#[test]
fn test_cmpa_extended() {
    let mut sys = Simple6809System::new();
    // LDA #$40
    // CMPA $2000 (Value $40) -> Z=1
    sys.load_rom(
        0,
        &[
            0x86, 0x40, // LDA #$40
            0xB1, 0x20, 0x00, // CMPA $2000
        ],
    );
    sys.write_ram(0x2000, 0x40);

    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x40); // A should not change
    assert_ne!(state.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_anda_extended() {
    let mut sys = Simple6809System::new();
    // LDA #$FF
    // ANDA $3000 (Value $0F) -> A=$0F
    sys.load_rom(
        0,
        &[
            0x86, 0xFF, // LDA #$FF
            0xB4, 0x30, 0x00, // ANDA $3000
        ],
    );
    sys.write_ram(0x3000, 0x0F);

    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x0F);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_adcb_extended_with_carry() {
    let mut sys = Simple6809System::new();
    // LDB #$00
    // COMA (Sets Carry)
    // ADCB $4000 (Value $10) -> B = $00 + $10 + 1 = $11
    sys.load_rom(
        0,
        &[
            0xC6, 0x00, // LDB #$00 (2 cycles)
            0x43, // COMA (2 cycles) - Sets C=1
            0xF9, 0x40, 0x00, // ADCB $4000 (4 cycles)
        ],
    );
    sys.write_ram(0x4000, 0x10);

    // Total 8 cycles
    for _ in 0..8 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x11);
}

#[test]
fn test_orb_extended() {
    let mut sys = Simple6809System::new();
    // LDB #$F0
    // ORB $5000 (Value $0F) -> B=$FF
    sys.load_rom(
        0,
        &[
            0xC6, 0xF0, // LDB #$F0
            0xFA, 0x50, 0x00, // ORB $5000
        ],
    );
    sys.write_ram(0x5000, 0x0F);

    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0xFF);
    assert_ne!(state.cc & (CcFlag::N as u8), 0);
}
