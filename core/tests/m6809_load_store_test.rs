use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

fn tick(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

#[test]
fn test_load_accumulator_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0x42]); // LDA #$42

    // Run 2 cycles to complete the LDA instruction
    // Cycle 0: Fetch opcode 0x86
    // Cycle 1: Execute - fetch operand 0x42 and load into A
    tick(&mut cpu, &mut bus, 2);

    // Verify A register loaded with immediate value
    assert_eq!(cpu.a, 0x42, "A register should be 0x42 after LDA #$42");
    assert_eq!(cpu.pc, 2, "PC should be at 0x02 after LDA");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow should be clear");
}

#[test]
fn test_reset() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x86, 0xFF, 0x97, 0x00]); // LDA #$FF, STA $00

    // Run cycles to execute both instructions
    // LDA #$FF: 2 cycles (fetch opcode, execute and load)
    // STA $00: 3 cycles (fetch opcode, fetch address, store)
    tick(&mut cpu, &mut bus, 5);

    // Verify the CPU state after execution
    // After LDA #$FF, the A register should contain 0xFF
    assert_eq!(cpu.a, 0xFF, "A register should be 0xFF after LDA #$FF");
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow should be clear");

    // After STA $00, memory[0] should contain 0xFF
    assert_eq!(
        bus.memory[0], 0xFF,
        "memory[0] should be 0xFF after STA $00"
    );

    // PC should have advanced past both instructions (2 + 2 = 4 bytes)
    assert_eq!(cpu.pc, 4, "PC should be at 0x04 after both instructions");
}

#[test]
fn test_store_accumulator_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Load: LDA #$55, STA $10
    bus.load(0, &[0x86, 0x55, 0x97, 0x10]);

    // LDA #$55: 2 cycles + STA $10: 3 cycles = 5 cycles
    tick(&mut cpu, &mut bus, 5);

    // Verify the value was stored
    assert_eq!(cpu.a, 0x55, "A register should be 0x55");
    assert_eq!(
        bus.memory[0x10], 0x55,
        "memory[0x10] should be 0x55 after store"
    );
    assert_eq!(cpu.pc, 4, "PC should be at 0x04");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow should be clear");
}

#[test]
fn test_multiple_loads_and_stores() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Load multiple values and store them
    bus.load(
        0,
        &[
            0x86, 0x11, // LDA #$11
            0x97, 0x00, // STA $00
            0x86, 0x22, // LDA #$22
            0x97, 0x01, // STA $01
        ],
    );

    // Run enough cycles to execute all 4 instructions (2+3+2+3 = 10 cycles)
    tick(&mut cpu, &mut bus, 10);

    // Verify all values were loaded and stored
    assert_eq!(cpu.a, 0x22, "A register should be 0x22 (last loaded value)");
    assert_eq!(bus.memory[0x00], 0x11, "memory[0x00] should be 0x11");
    assert_eq!(bus.memory[0x01], 0x22, "memory[0x01] should be 0x22");
    assert_eq!(cpu.pc, 8, "PC should be at 0x08 after all instructions");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow should be clear");
}

#[test]
fn test_ldy_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDY #$1234 (0x10 0x8E 0x12 0x34)
    bus.load(0, &[0x10, 0x8E, 0x12, 0x34]);

    // LDY immediate: 4 cycles (2 prefix + 2 execute)
    tick(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.y, 0x1234, "Y should be 0x1234");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "V should be clear");
}

#[test]
fn test_ldy_immediate_zero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x10, 0x8E, 0x00, 0x00]); // LDY #$0000

    tick(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.y, 0x0000);
    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
}

#[test]
fn test_ldy_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDY $20 (0x10 0x9E 0x20) — reads from DP:$20
    bus.load(0, &[0x10, 0x9E, 0x20]);
    bus.memory[0x0020] = 0xAB;
    bus.memory[0x0021] = 0xCD;

    // LDY direct: 5 cycles (2 prefix + 3 execute)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.y, 0xABCD, "Y should be 0xABCD");
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_ldy_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDY $2000 (0x10 0xBE 0x20 0x00)
    bus.load(0, &[0x10, 0xBE, 0x20, 0x00]);
    bus.memory[0x2000] = 0x56;
    bus.memory[0x2001] = 0x78;

    // LDY extended: 6 cycles (2 prefix + 4 execute)
    tick(&mut cpu, &mut bus, 6);

    assert_eq!(cpu.y, 0x5678);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_sty_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDY #$ABCD, STY $30 (0x10 0x9F 0x30)
    bus.load(0, &[0x10, 0x8E, 0xAB, 0xCD, 0x10, 0x9F, 0x30]);

    // LDY (4 cycles) + STY direct (5 cycles) = 9 cycles
    tick(&mut cpu, &mut bus, 9);

    assert_eq!(bus.memory[0x0030], 0xAB, "High byte stored");
    assert_eq!(bus.memory[0x0031], 0xCD, "Low byte stored");
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_sty_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDY #$1234, STY $2000 (0x10 0xBF 0x20 0x00)
    bus.load(0, &[0x10, 0x8E, 0x12, 0x34, 0x10, 0xBF, 0x20, 0x00]);

    // LDY (4 cycles) + STY extended (6 cycles) = 10 cycles
    tick(&mut cpu, &mut bus, 10);

    assert_eq!(bus.memory[0x2000], 0x12, "High byte stored");
    assert_eq!(bus.memory[0x2001], 0x34, "Low byte stored");
}

#[test]
fn test_lds_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDS #$4000 (0x10 0xCE 0x40 0x00)
    bus.load(0, &[0x10, 0xCE, 0x40, 0x00]);

    tick(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.s, 0x4000, "S should be 0x4000");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "V should be clear");
}

#[test]
fn test_sts_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDS #$BEEF, STS $40 (0x10 0xDF 0x40)
    bus.load(0, &[0x10, 0xCE, 0xBE, 0xEF, 0x10, 0xDF, 0x40]);

    // LDS (4 cycles) + STS direct (5 cycles) = 9 cycles
    tick(&mut cpu, &mut bus, 9);

    assert_eq!(bus.memory[0x0040], 0xBE, "High byte stored");
    assert_eq!(bus.memory[0x0041], 0xEF, "Low byte stored");
}

#[test]
fn test_load_16bit_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1234, LDX #$5678, LDU #$9ABC
    bus.load(0, &[0xCC, 0x12, 0x34, 0x8E, 0x56, 0x78, 0xCE, 0x9A, 0xBC]);

    // LDD (3 cycles)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.b, 0x34);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    // LDX (3 cycles)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x5678);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    // LDU (3 cycles)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.u, 0x9ABC);
    // 0x9ABC has bit 15 set, so N should be set
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}
