use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_tfr_8bit() {
    let mut sys = Simple6809System::new();
    // LDA #$42, TFR A,B
    // TFR op: 1F, operand: 89 (A=8, B=9)
    sys.load_rom(0, &[0x86, 0x42, 0x1F, 0x89]);

    sys.tick();
    sys.tick(); // LDA
    assert_eq!(sys.get_cpu_state().a, 0x42);
    assert_eq!(sys.get_cpu_state().b, 0x00);

    sys.tick();
    sys.tick(); // TFR
    assert_eq!(sys.get_cpu_state().b, 0x42);
    assert_eq!(sys.get_cpu_state().a, 0x42); // Source unchanged
}

#[test]
fn test_tfr_16bit() {
    let mut sys = Simple6809System::new();
    // LDX #$1234, TFR X,Y
    // TFR op: 1F, operand: 12 (X=1, Y=2)
    sys.load_rom(0, &[0x8E, 0x12, 0x34, 0x1F, 0x12]);

    sys.tick();
    sys.tick();
    sys.tick(); // LDX
    assert_eq!(sys.get_cpu_state().x, 0x1234);

    sys.tick();
    sys.tick(); // TFR
    assert_eq!(sys.get_cpu_state().y, 0x1234);
}

#[test]
fn test_exg_8bit() {
    let mut sys = Simple6809System::new();
    // LDA #$AA, LDB #$55, EXG A,B
    // EXG op: 1E, operand: 89
    sys.load_rom(0, &[0x86, 0xAA, 0xC6, 0x55, 0x1E, 0x89]);

    sys.tick();
    sys.tick(); // LDA
    sys.tick();
    sys.tick(); // LDB
    assert_eq!(sys.get_cpu_state().a, 0xAA);
    assert_eq!(sys.get_cpu_state().b, 0x55);

    sys.tick();
    sys.tick(); // EXG
    assert_eq!(sys.get_cpu_state().a, 0x55);
    assert_eq!(sys.get_cpu_state().b, 0xAA);
}
