use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_pshs_puls_all() {
    let mut sys = Simple6809System::new();
    // LDS #$1000
    // LDA #$AA, LDB #$BB, LDX #$1234
    // PSHS A,B,X (Mask: X=bit4, B=bit2, A=bit1 -> 00010110 = 0x16)
    // CLRA, CLRB, LDX #$0000
    // PULS A,B,X
    sys.load_rom(
        0,
        &[
            // Setup S using U and TFR
            // Reset S is 0.
            // Let's implement LDS/LDU properly or use TFR.
            // TFR U,S is 1F 34.
            0xCE, 0x10, 0x00, // LDU #$1000
            0x1F, 0x34, // TFR U,S
            0x86, 0xAA, // LDA #$AA
            0xC6, 0xBB, // LDB #$BB
            0x8E, 0x12, 0x34, // LDX #$1234
            0x34, 0x16, // PSHS A,B,X
            0x4F, // CLRA
            0x5F, // CLRB
            0x8E, 0x00, 0x00, // LDX #$0000
            0x35, 0x16, // PULS A,B,X
        ],
    );

    // Execute setup
    // LDU(3) + TFR(2) + LDA(2) + LDB(2) + LDX(3) = 12 cycles
    for _ in 0..12 {
        sys.tick();
    }

    // Check state before PSHS
    assert_eq!(sys.get_cpu_state().s, 0x1000);
    assert_eq!(sys.get_cpu_state().a, 0xAA);
    assert_eq!(sys.get_cpu_state().x, 0x1234);

    // Execute PSHS
    // PSHS A,B,X:
    // Implementation takes 7 cycles (Fetch + ReadMask + 4 pushes + DoneCheck)
    for _ in 0..7 {
        sys.tick();
    }

    // S should be 0x1000 - 4 = 0x0FFC
    assert_eq!(sys.get_cpu_state().s, 0x0FFC);
    // Memory check:
    // 0x0FFF: X Low (34)
    // 0x0FFE: X High (12)
    // 0x0FFD: B (BB)
    // 0x0FFC: A (AA)
    assert_eq!(sys.read_ram(0x0FFF), 0x34);
    assert_eq!(sys.read_ram(0x0FFE), 0x12);
    assert_eq!(sys.read_ram(0x0FFD), 0xBB);
    assert_eq!(sys.read_ram(0x0FFC), 0xAA);

    // Execute clears
    // CLRA(2) + CLRB(2) + LDX(3) = 7 cycles
    for _ in 0..7 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().a, 0x00);
    assert_eq!(sys.get_cpu_state().b, 0x00);
    assert_eq!(sys.get_cpu_state().x, 0x0000);

    // Execute PULS (7 cycles)
    for _ in 0..7 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().s, 0x1000);
    assert_eq!(sys.get_cpu_state().a, 0xAA);
    assert_eq!(sys.get_cpu_state().b, 0xBB);
    assert_eq!(sys.get_cpu_state().x, 0x1234);
}
