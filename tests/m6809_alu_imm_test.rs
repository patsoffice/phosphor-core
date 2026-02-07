use phosphor_core::machine::simple6809::Simple6809System;
use phosphor_core::cpu::m6809::CcFlag;

#[test]
fn test_cmpa_immediate() {
    let mut sys = Simple6809System::new();
    // LDA #$10, CMPA #$10, CMPA #$20
    sys.load_rom(0, &[0x86, 0x10, 0x81, 0x10, 0x81, 0x20]);

    // LDA #$10
    sys.tick(); sys.tick();

    // CMPA #$10 (10 - 10 = 0) -> Z=1
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x10);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);

    // CMPA #$20 (10 - 20 = -16 = F0) -> N=1, C=1
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x10);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_sbca_immediate() {
    let mut sys = Simple6809System::new();
    // LDA #$00, SUBA #$01 (sets C), SBCA #$01
    sys.load_rom(0, &[0x86, 0x00, 0x80, 0x01, 0x82, 0x01]);

    // LDA #$00
    sys.tick(); sys.tick();
    // SUBA #$01 -> A=FF, C=1
    sys.tick(); sys.tick();
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::C as u8), CcFlag::C as u8);

    // SBCA #$01 -> A = FF - 01 - 1 = FD
    sys.tick(); sys.tick();
    assert_eq!(sys.get_cpu_state().a, 0xFD);
}

#[test]
fn test_logical_ops() {
    let mut sys = Simple6809System::new();
    // LDA #$CC, ANDA #$F0, ORA #$03, EORA #$FF
    sys.load_rom(0, &[0x86, 0xCC, 0x84, 0xF0, 0x8A, 0x03, 0x88, 0xFF]);

    sys.tick(); sys.tick(); // LDA

    sys.tick(); sys.tick(); // ANDA #$F0 -> C0
    assert_eq!(sys.get_cpu_state().a, 0xC0);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), CcFlag::N as u8); // C0 is neg

    sys.tick(); sys.tick(); // ORA #$03 -> C3
    assert_eq!(sys.get_cpu_state().a, 0xC3);

    sys.tick(); sys.tick(); // EORA #$FF -> 3C
    assert_eq!(sys.get_cpu_state().a, 0x3C);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_bita_immediate() {
    let mut sys = Simple6809System::new();
    // LDA #$FF, BITA #$00, BITA #$80
    sys.load_rom(0, &[0x86, 0xFF, 0x85, 0x00, 0x85, 0x80]);

    sys.tick(); sys.tick(); // LDA

    sys.tick(); sys.tick(); // BITA #$00 -> Z=1
    assert_eq!(sys.get_cpu_state().a, 0xFF);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    sys.tick(); sys.tick(); // BITA #$80 -> N=1
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), CcFlag::N as u8);
}

#[test]
fn test_adca_immediate() {
    let mut sys = Simple6809System::new();
    // LDA #$FF, ADDA #$01 (sets C), ADCA #$00
    sys.load_rom(0, &[0x86, 0xFF, 0x8B, 0x01, 0x89, 0x00]);

    sys.tick(); sys.tick(); // LDA
    sys.tick(); sys.tick(); // ADDA -> 00, C=1

    sys.tick(); sys.tick(); // ADCA #$00 -> 00 + 00 + 1 = 01
    assert_eq!(sys.get_cpu_state().a, 0x01);
}

#[test]
fn test_b_register_alu() {
    let mut sys = Simple6809System::new();
    // LDB #$10, ADDB #$10, SUBB #$05
    sys.load_rom(0, &[0xC6, 0x10, 0xCB, 0x10, 0xC0, 0x05]);

    sys.tick(); sys.tick(); // LDB
    assert_eq!(sys.get_cpu_state().b, 0x10);

    sys.tick(); sys.tick(); // ADDB
    assert_eq!(sys.get_cpu_state().b, 0x20);

    sys.tick(); sys.tick(); // SUBB
    assert_eq!(sys.get_cpu_state().b, 0x1B);
}

#[test]
fn test_cmpb_immediate() {
    let mut sys = Simple6809System::new();
    // LDB #$10, CMPB #$10, CMPB #$20
    sys.load_rom(0, &[0xC6, 0x10, 0xC1, 0x10, 0xC1, 0x20]);

    // LDB #$10
    sys.tick(); sys.tick();

    // CMPB #$10 (10 - 10 = 0) -> Z=1
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x10);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);

    // CMPB #$20 (10 - 20 = -16 = F0) -> N=1, C=1
    sys.tick(); sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x10);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_sbcb_immediate() {
    let mut sys = Simple6809System::new();
    // LDB #$00, SUBB #$01 (sets C), SBCB #$01
    sys.load_rom(0, &[0xC6, 0x00, 0xC0, 0x01, 0xC2, 0x01]);

    // LDB #$00
    sys.tick(); sys.tick();
    // SUBB #$01 -> B=FF, C=1
    sys.tick(); sys.tick();
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::C as u8), CcFlag::C as u8);

    // SBCB #$01 -> B = FF - 01 - 1 = FD
    sys.tick(); sys.tick();
    assert_eq!(sys.get_cpu_state().b, 0xFD);
}

#[test]
fn test_logical_ops_b() {
    let mut sys = Simple6809System::new();
    // LDB #$CC, ANDB #$F0, ORB #$03, EORB #$FF
    sys.load_rom(0, &[0xC6, 0xCC, 0xC4, 0xF0, 0xCA, 0x03, 0xC8, 0xFF]);

    sys.tick(); sys.tick(); // LDB

    sys.tick(); sys.tick(); // ANDB #$F0 -> C0
    assert_eq!(sys.get_cpu_state().b, 0xC0);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), CcFlag::N as u8); // C0 is neg

    sys.tick(); sys.tick(); // ORB #$03 -> C3
    assert_eq!(sys.get_cpu_state().b, 0xC3);

    sys.tick(); sys.tick(); // EORB #$FF -> 3C
    assert_eq!(sys.get_cpu_state().b, 0x3C);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_bitb_immediate() {
    let mut sys = Simple6809System::new();
    // LDB #$FF, BITB #$00, BITB #$80
    sys.load_rom(0, &[0xC6, 0xFF, 0xC5, 0x00, 0xC5, 0x80]);

    sys.tick(); sys.tick(); // LDB

    sys.tick(); sys.tick(); // BITB #$00 -> Z=1
    assert_eq!(sys.get_cpu_state().b, 0xFF);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    sys.tick(); sys.tick(); // BITB #$80 -> N=1
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), CcFlag::N as u8);
}

#[test]
fn test_adcb_immediate() {
    let mut sys = Simple6809System::new();
    // LDB #$FF, ADDB #$01 (sets C), ADCB #$00
    sys.load_rom(0, &[0xC6, 0xFF, 0xCB, 0x01, 0xC9, 0x00]);

    sys.tick(); sys.tick(); // LDB
    sys.tick(); sys.tick(); // ADDB -> 00, C=1

    sys.tick(); sys.tick(); // ADCB #$00 -> 00 + 00 + 1 = 01
    assert_eq!(sys.get_cpu_state().b, 0x01);
}