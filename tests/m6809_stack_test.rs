use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

#[test]
fn test_pshs_puls_all() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDS #$1000
    // LDA #$AA, LDB #$BB, LDX #$1234
    // PSHS A,B,X (Mask: X=bit4, B=bit2, A=bit1 -> 00010110 = 0x16)
    // CLRA, CLRB, LDX #$0000
    // PULS A,B,X
    bus.load(
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
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // Check state before PSHS
    assert_eq!(cpu.s, 0x1000);
    assert_eq!(cpu.a, 0xAA);
    assert_eq!(cpu.x, 0x1234);

    // Execute PSHS
    // PSHS A,B,X:
    // Implementation takes 7 cycles (Fetch + ReadMask + 4 pushes + DoneCheck)
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // S should be 0x1000 - 4 = 0x0FFC
    assert_eq!(cpu.s, 0x0FFC);
    // Memory check:
    // 0x0FFF: X Low (34)
    // 0x0FFE: X High (12)
    // 0x0FFD: B (BB)
    // 0x0FFC: A (AA)
    assert_eq!(bus.memory[0x0FFF], 0x34);
    assert_eq!(bus.memory[0x0FFE], 0x12);
    assert_eq!(bus.memory[0x0FFD], 0xBB);
    assert_eq!(bus.memory[0x0FFC], 0xAA);

    // Execute clears
    // CLRA(2) + CLRB(2) + LDX(3) = 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.b, 0x00);
    assert_eq!(cpu.x, 0x0000);

    // Execute PULS (7 cycles)
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.s, 0x1000);
    assert_eq!(cpu.a, 0xAA);
    assert_eq!(cpu.b, 0xBB);
    assert_eq!(cpu.x, 0x1234);
}
