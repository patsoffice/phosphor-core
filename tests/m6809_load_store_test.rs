use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_load_accumulator_immediate() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[0x86, 0x42]); // LDA #$42

    // Run 2 cycles to complete the LDA instruction
    // Cycle 0: Fetch opcode 0x86
    // Cycle 1: Execute - fetch operand 0x42 and load into A
    sys.tick();
    sys.tick();

    // Verify A register loaded with immediate value
    assert_eq!(
        sys.get_cpu_state().a,
        0x42,
        "A register should be 0x42 after LDA #$42"
    );
    assert_eq!(sys.get_cpu_state().pc, 2, "PC should be at 0x02 after LDA");
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::N as u8),
        0,
        "Negative should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::Z as u8),
        0,
        "Zero should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear"
    );
}

#[test]
fn test_reset() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[0x86, 0xFF, 0x97, 0x00]); // LDA #$FF, STA $00

    // Run cycles to execute both instructions
    // LDA #$FF: 2 cycles (fetch opcode, execute and load)
    // STA $00: 2 cycles (fetch address, store)
    for _ in 0..5 {
        sys.tick();
    }

    // Verify the CPU state after execution
    // After LDA #$FF, the A register should contain 0xFF
    assert_eq!(
        sys.get_cpu_state().a,
        0xFF,
        "A register should be 0xFF after LDA #$FF"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::Z as u8),
        0,
        "Zero should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear"
    );

    // After STA $00, RAM[0] should contain 0xFF
    assert_eq!(sys.read_ram(0), 0xFF, "RAM[0] should be 0xFF after STA $00");

    // PC should have advanced past both instructions (2 + 2 = 4 bytes)
    assert_eq!(
        sys.get_cpu_state().pc,
        4,
        "PC should be at 0x04 after both instructions"
    );
}

#[test]
fn test_store_accumulator_direct() {
    let mut sys = Simple6809System::new();
    // Load: LDA #$55, STA $10
    sys.load_rom(0, &[0x86, 0x55, 0x97, 0x10]);

    // Run cycles to execute both instructions
    // LDA #$55: 2 cycles
    // STA $10: 2 cycles
    for _ in 0..5 {
        sys.tick();
    }

    // Verify the value was stored
    assert_eq!(sys.get_cpu_state().a, 0x55, "A register should be 0x55");
    assert_eq!(
        sys.read_ram(0x10),
        0x55,
        "RAM[0x10] should be 0x55 after store"
    );
    assert_eq!(sys.get_cpu_state().pc, 4, "PC should be at 0x04");
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::N as u8),
        0,
        "Negative should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::Z as u8),
        0,
        "Zero should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear"
    );
}

#[test]
fn test_multiple_loads_and_stores() {
    let mut sys = Simple6809System::new();
    // Load multiple values and store them
    sys.load_rom(
        0,
        &[
            0x86, 0x11, // LDA #$11
            0x97, 0x00, // STA $00
            0x86, 0x22, // LDA #$22
            0x97, 0x01, // STA $01
        ],
    );

    // Run enough cycles to execute all 4 instructions (2 cycles each = 8 cycles)
    for _ in 0..10 {
        sys.tick();
    }

    // Verify all values were loaded and stored
    assert_eq!(
        sys.get_cpu_state().a,
        0x22,
        "A register should be 0x22 (last loaded value)"
    );
    assert_eq!(sys.read_ram(0x00), 0x11, "RAM[0x00] should be 0x11");
    assert_eq!(sys.read_ram(0x01), 0x22, "RAM[0x01] should be 0x22");
    assert_eq!(
        sys.get_cpu_state().pc,
        8,
        "PC should be at 0x08 after all instructions"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::N as u8),
        0,
        "Negative should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::Z as u8),
        0,
        "Zero should be clear"
    );
    assert_eq!(
        sys.get_cpu_state().cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear"
    );
}

#[test]
fn test_ldy_immediate() {
    let mut sys = Simple6809System::new();
    // LDY #$1234 (0x10 0x8E 0x12 0x34)
    sys.load_rom(0, &[0x10, 0x8E, 0x12, 0x34]);

    // LDY immediate: 4 cycles (2 prefix + 2 execute)
    for _ in 0..4 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.y, 0x1234, "Y should be 0x1234");
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
    assert_eq!(state.cc & (CcFlag::V as u8), 0, "V should be clear");
}

#[test]
fn test_ldy_immediate_zero() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[0x10, 0x8E, 0x00, 0x00]); // LDY #$0000

    for _ in 0..4 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.y, 0x0000);
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
}

#[test]
fn test_ldy_direct() {
    let mut sys = Simple6809System::new();
    // LDY $20 (0x10 0x9E 0x20) — reads from DP:$20
    sys.load_rom(0, &[0x10, 0x9E, 0x20]);
    sys.write_ram(0x0020, 0xAB);
    sys.write_ram(0x0021, 0xCD);

    // LDY direct: 5 cycles (2 prefix + 3 execute)
    for _ in 0..5 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.y, 0xABCD, "Y should be 0xABCD");
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_ldy_extended() {
    let mut sys = Simple6809System::new();
    // LDY $2000 (0x10 0xBE 0x20 0x00)
    sys.load_rom(0, &[0x10, 0xBE, 0x20, 0x00]);
    sys.write_ram(0x2000, 0x56);
    sys.write_ram(0x2001, 0x78);

    // LDY extended: 6 cycles (2 prefix + 4 execute)
    for _ in 0..6 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.y, 0x5678);
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_sty_direct() {
    let mut sys = Simple6809System::new();
    // LDY #$ABCD, STY $30 (0x10 0x9F 0x30)
    sys.load_rom(0, &[0x10, 0x8E, 0xAB, 0xCD, 0x10, 0x9F, 0x30]);

    // LDY (4 cycles) + STY direct (5 cycles) = 9 cycles
    for _ in 0..9 {
        sys.tick();
    }

    assert_eq!(sys.read_ram(0x0030), 0xAB, "High byte stored");
    assert_eq!(sys.read_ram(0x0031), 0xCD, "Low byte stored");
    let state = sys.get_cpu_state();
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_sty_extended() {
    let mut sys = Simple6809System::new();
    // LDY #$1234, STY $2000 (0x10 0xBF 0x20 0x00)
    sys.load_rom(0, &[0x10, 0x8E, 0x12, 0x34, 0x10, 0xBF, 0x20, 0x00]);

    // LDY (4 cycles) + STY extended (6 cycles) = 10 cycles
    for _ in 0..10 {
        sys.tick();
    }

    assert_eq!(sys.read_ram(0x2000), 0x12, "High byte stored");
    assert_eq!(sys.read_ram(0x2001), 0x34, "Low byte stored");
}

#[test]
fn test_lds_immediate() {
    let mut sys = Simple6809System::new();
    // LDS #$4000 (0x10 0xCE 0x40 0x00)
    sys.load_rom(0, &[0x10, 0xCE, 0x40, 0x00]);

    for _ in 0..4 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.s, 0x4000, "S should be 0x4000");
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
    assert_eq!(state.cc & (CcFlag::V as u8), 0, "V should be clear");
}

#[test]
fn test_sts_direct() {
    let mut sys = Simple6809System::new();
    // LDS #$BEEF, STS $40 (0x10 0xDF 0x40)
    sys.load_rom(0, &[0x10, 0xCE, 0xBE, 0xEF, 0x10, 0xDF, 0x40]);

    // LDS (4 cycles) + STS direct (5 cycles) = 9 cycles
    for _ in 0..9 {
        sys.tick();
    }

    assert_eq!(sys.read_ram(0x0040), 0xBE, "High byte stored");
    assert_eq!(sys.read_ram(0x0041), 0xEF, "Low byte stored");
}

#[test]
fn test_load_16bit_immediate() {
    let mut sys = Simple6809System::new();
    // LDD #$1234, LDX #$5678, LDU #$9ABC
    sys.load_rom(0, &[0xCC, 0x12, 0x34, 0x8E, 0x56, 0x78, 0xCE, 0x9A, 0xBC]);

    // LDD (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x12);
    assert_eq!(state.b, 0x34);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);

    // LDX (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.x, 0x5678);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);

    // LDU (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.u, 0x9ABC);
    // 0x9ABC has bit 15 set, so N should be set
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}
