use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6502::{M6502, StatusFlag};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` cycles
fn tick(cpu: &mut M6502, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// Flag instructions - CLC, SEC, CLI, SEI, CLV, CLD, SED
// =============================================================================

#[test]
fn test_clc() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::C as u8; // Set carry first
    bus.load(0, &[0x18]); // CLC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_sec() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::C as u8); // Clear carry first
    bus.load(0, &[0x38]); // SEC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_cli() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::I as u8; // Set interrupt disable first
    bus.load(0, &[0x58]); // CLI
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::I as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_sei() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Clear I first
    bus.load(0, &[0x78]); // SEI
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::I as u8), StatusFlag::I as u8);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_clv() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::V as u8; // Set overflow first
    bus.load(0, &[0xB8]); // CLV
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::V as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_cld() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::D as u8; // Set decimal first
    bus.load(0, &[0xD8]); // CLD
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::D as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_sed() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::D as u8); // Clear decimal first
    bus.load(0, &[0xF8]); // SED
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::D as u8), StatusFlag::D as u8);
    assert_eq!(cpu.pc, 1);
}

// Verify flag instructions don't affect other flags
#[test]
fn test_clc_preserves_other_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = 0xFF; // All flags set
    bus.load(0, &[0x18]); // CLC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p, !(StatusFlag::C as u8)); // Only C cleared
}

#[test]
fn test_sec_preserves_other_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = 0x00; // All flags clear
    bus.load(0, &[0x38]); // SEC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p, StatusFlag::C as u8); // Only C set
}

// Verify flag instructions are idempotent
#[test]
fn test_clc_when_already_clear() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::C as u8); // Already clear
    bus.load(0, &[0x18]); // CLC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0);
}

#[test]
fn test_sec_when_already_set() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::C as u8; // Already set
    bus.load(0, &[0x38]); // SEC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
}

// =============================================================================
// Transfer instructions - TAX, TAY, TXA, TYA, TSX, TXS
// =============================================================================

#[test]
fn test_tax_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0xAA]); // TAX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x42);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_tax_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0xAA]); // TAX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_tax_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    bus.load(0, &[0xAA]); // TAX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_tay_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    bus.load(0, &[0xA8]); // TAY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x55);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_tay_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0xA8]); // TAY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_txa_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x33;
    bus.load(0, &[0x8A]); // TXA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x33);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_txa_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0xFF;
    bus.load(0, &[0x8A]); // TXA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

#[test]
fn test_tya_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x77;
    bus.load(0, &[0x98]); // TYA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x77);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_tya_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x00;
    cpu.a = 0xFF; // Make sure A actually changes
    bus.load(0, &[0x98]); // TYA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_tsx_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFD; // Default SP
    bus.load(0, &[0xBA]); // TSX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0xFD);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8); // 0xFD is negative
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_tsx_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00;
    bus.load(0, &[0xBA]); // TSX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_txs_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0xFF;
    bus.load(0, &[0x9A]); // TXS
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.sp, 0xFF);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_txs_does_not_set_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    let original_p = cpu.p;
    cpu.x = 0x00; // Zero value — should NOT set Z flag
    bus.load(0, &[0x9A]); // TXS
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.sp, 0x00);
    assert_eq!(cpu.p, original_p); // Flags unchanged
}

#[test]
fn test_txs_negative_value_no_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    let original_p = cpu.p;
    cpu.x = 0x80; // Negative value — should NOT set N flag
    bus.load(0, &[0x9A]); // TXS
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.sp, 0x80);
    assert_eq!(cpu.p, original_p); // Flags unchanged
}

// =============================================================================
// Register increment/decrement - INX, INY, DEX, DEY
// =============================================================================

#[test]
fn test_inx_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xE8]); // INX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x06);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_inx_wrap_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0xFF;
    bus.load(0, &[0xE8]); // INX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_inx_to_negative() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x7F;
    bus.load(0, &[0xE8]); // INX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_iny_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x10;
    bus.load(0, &[0xC8]); // INY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x11);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_iny_wrap_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0xFF;
    bus.load(0, &[0xC8]); // INY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_dex_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x05;
    bus.load(0, &[0xCA]); // DEX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x04);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_dex_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x01;
    bus.load(0, &[0xCA]); // DEX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_dex_wrap_to_ff() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x00;
    bus.load(0, &[0xCA]); // DEX
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.x, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_dey_basic() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x10;
    bus.load(0, &[0x88]); // DEY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x0F);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_dey_to_zero() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x01;
    bus.load(0, &[0x88]); // DEY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
}

#[test]
fn test_dey_wrap_to_ff() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.y = 0x00;
    bus.load(0, &[0x88]); // DEY
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.y, 0xFF);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
}

// =============================================================================
// NOP
// =============================================================================

#[test]
fn test_nop() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    let original_p = cpu.p;
    cpu.a = 0x42;
    cpu.x = 0x33;
    cpu.y = 0x77;
    bus.load(0, &[0xEA]); // NOP
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42); // A unchanged
    assert_eq!(cpu.x, 0x33); // X unchanged
    assert_eq!(cpu.y, 0x77); // Y unchanged
    assert_eq!(cpu.p, original_p); // Flags unchanged
    assert_eq!(cpu.pc, 1);
}

// =============================================================================
// Cycle count verification
// =============================================================================

#[test]
fn test_implied_ops_are_2_cycles() {
    // All implied instructions should complete in exactly 2 cycles (1 fetch + 1 execute)
    let opcodes: &[(u8, &str)] = &[
        (0x18, "CLC"),
        (0x38, "SEC"),
        (0x58, "CLI"),
        (0x78, "SEI"),
        (0xB8, "CLV"),
        (0xD8, "CLD"),
        (0xF8, "SED"),
        (0xAA, "TAX"),
        (0xA8, "TAY"),
        (0x8A, "TXA"),
        (0x98, "TYA"),
        (0xBA, "TSX"),
        (0x9A, "TXS"),
        (0xE8, "INX"),
        (0xC8, "INY"),
        (0xCA, "DEX"),
        (0x88, "DEY"),
        (0xEA, "NOP"),
    ];

    for &(opcode, name) in opcodes {
        let mut cpu = M6502::new();
        let mut bus = TestBus::new();
        cpu.a = 0x42;
        cpu.x = 0x33;
        cpu.y = 0x77;
        cpu.sp = 0xFD;
        // Load opcode followed by a second NOP
        bus.load(0, &[opcode, 0xEA]);
        // After 2 cycles, the instruction should be complete (PC = 1)
        tick(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.pc, 1, "{name} (0x{opcode:02X}) should advance PC by 1");
    }
}

// =============================================================================
// Sequence tests - multiple implied ops in a row
// =============================================================================

#[test]
fn test_inx_dex_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.x = 0x42;
    bus.load(0, &[0xE8, 0xCA]); // INX; DEX
    tick(&mut cpu, &mut bus, 4); // 2 cycles each
    assert_eq!(cpu.x, 0x42); // Back to original
}

#[test]
fn test_tax_txa_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    cpu.x = 0x00;
    bus.load(0, &[0xAA, 0x8A]); // TAX; TXA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.x, 0x55);
}

#[test]
fn test_clc_sec_sequence() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x18, 0x38]); // CLC; SEC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), 0); // After CLC
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8); // After SEC
}

#[test]
fn test_transfer_chain_a_to_x_to_sp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0xAA, 0x9A]); // TAX; TXS
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.x, 0xFF);
    assert_eq!(cpu.sp, 0xFF);
}
