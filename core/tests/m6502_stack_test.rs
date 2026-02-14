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
// PHA / PLA
// =============================================================================

#[test]
fn test_pha_pushes_a() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x48]); // PHA
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(bus.memory[0x01FD], 0x42);
    assert_eq!(cpu.sp, 0xFC);
}

#[test]
fn test_pla_pulls_a() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFC; // One byte on stack
    bus.memory[0x01FD] = 0x42;
    bus.load(0, &[0x68]); // PLA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn test_pha_pla_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    bus.load(0, &[0x48, 0x68]); // PHA; PLA
    tick(&mut cpu, &mut bus, 3); // PHA
    cpu.a = 0x00; // Clear A
    tick(&mut cpu, &mut bus, 4); // PLA
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.sp, 0xFD);
}

#[test]
fn test_pla_sets_zero_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFC;
    bus.memory[0x01FD] = 0x00;
    bus.load(0, &[0x68]); // PLA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}

#[test]
fn test_pla_sets_negative_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFC;
    bus.memory[0x01FD] = 0x80;
    bus.load(0, &[0x68]); // PLA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
}

#[test]
fn test_pha_does_not_modify_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    let p_before = cpu.p;
    bus.load(0, &[0x48]); // PHA
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.p, p_before); // PHA does not set any flags
}

// =============================================================================
// PHP / PLP
// =============================================================================

#[test]
fn test_php_pushes_p_with_b_and_u_set() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = 0x00; // All flags clear
    bus.load(0, &[0x08]); // PHP
    tick(&mut cpu, &mut bus, 3);
    let pushed = bus.memory[0x01FD];
    assert_eq!(
        pushed & (StatusFlag::B as u8),
        StatusFlag::B as u8,
        "B should be set in pushed value"
    );
    assert_eq!(
        pushed & (StatusFlag::U as u8),
        StatusFlag::U as u8,
        "U should be set in pushed value"
    );
    assert_eq!(cpu.sp, 0xFC);
}

#[test]
fn test_php_preserves_all_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = StatusFlag::C as u8 | StatusFlag::N as u8 | StatusFlag::V as u8;
    bus.load(0, &[0x08]); // PHP
    tick(&mut cpu, &mut bus, 3);
    let pushed = bus.memory[0x01FD];
    // Pushed value should have all CPU flags plus B and U
    let expected = StatusFlag::C as u8
        | StatusFlag::N as u8
        | StatusFlag::V as u8
        | StatusFlag::B as u8
        | StatusFlag::U as u8;
    assert_eq!(pushed, expected);
}

#[test]
fn test_plp_restores_flags() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFC;
    let flags = StatusFlag::C as u8
        | StatusFlag::Z as u8
        | StatusFlag::V as u8
        | StatusFlag::N as u8
        | StatusFlag::B as u8
        | StatusFlag::U as u8;
    bus.memory[0x01FD] = flags;
    bus.load(0, &[0x28]); // PLP
    tick(&mut cpu, &mut bus, 4);
    // B should be clear in P, U should be set
    assert_eq!(cpu.p & (StatusFlag::C as u8), StatusFlag::C as u8);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), StatusFlag::Z as u8);
    assert_eq!(cpu.p & (StatusFlag::V as u8), StatusFlag::V as u8);
    assert_eq!(cpu.p & (StatusFlag::N as u8), StatusFlag::N as u8);
    assert_eq!(cpu.p & (StatusFlag::B as u8), 0); // B always clear in P
    assert_eq!(cpu.p & (StatusFlag::U as u8), StatusFlag::U as u8); // U always set
}

#[test]
fn test_plp_ignores_b_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFC;
    bus.memory[0x01FD] = 0xFF; // All bits set including B
    bus.load(0, &[0x28]); // PLP
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.p & (StatusFlag::B as u8), 0, "B should always be clear");
    assert_eq!(
        cpu.p & (StatusFlag::U as u8),
        StatusFlag::U as u8,
        "U should always be set"
    );
}

#[test]
fn test_php_plp_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = StatusFlag::C as u8 | StatusFlag::I as u8 | StatusFlag::U as u8;
    let original_p = cpu.p;
    bus.load(0, &[0x08, 0x28]); // PHP; PLP
    tick(&mut cpu, &mut bus, 3); // PHP
    cpu.p = 0x00; // Clobber P
    tick(&mut cpu, &mut bus, 4); // PLP
    // Should be restored (B clear, U set)
    assert_eq!(
        cpu.p,
        (original_p | StatusFlag::U as u8) & !(StatusFlag::B as u8)
    );
}

// =============================================================================
// BRK
// =============================================================================

#[test]
fn test_brk_vectors_through_fffe() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80; // Vector = $8000
    bus.load(0, &[0x00, 0xEA]); // BRK, padding byte
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x8000);
}

#[test]
fn test_brk_pushes_pc_plus_2() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0x00, 0xEA]); // BRK at $0000
    tick(&mut cpu, &mut bus, 7);
    // BRK pushes PC+2: opcode at $0000, padding at $0001, so pushed PC = $0002
    assert_eq!(bus.memory[0x01FD], 0x00); // PCH
    assert_eq!(bus.memory[0x01FC], 0x02); // PCL
}

#[test]
fn test_brk_pushes_p_with_b_set() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    cpu.p = StatusFlag::U as u8; // Only U set
    bus.load(0, &[0x00, 0xEA]);
    tick(&mut cpu, &mut bus, 7);
    let pushed_p = bus.memory[0x01FB];
    assert_eq!(
        pushed_p & (StatusFlag::B as u8),
        StatusFlag::B as u8,
        "B should be set in pushed P"
    );
    assert_eq!(
        pushed_p & (StatusFlag::U as u8),
        StatusFlag::U as u8,
        "U should be set in pushed P"
    );
}

#[test]
fn test_brk_sets_i_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    cpu.p &= !(StatusFlag::I as u8); // I=0 before BRK
    bus.load(0, &[0x00, 0xEA]);
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(
        cpu.p & (StatusFlag::I as u8),
        StatusFlag::I as u8,
        "I should be set after BRK"
    );
}

#[test]
fn test_brk_sp_decremented_by_3() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    let sp_before = cpu.sp; // 0xFD
    bus.load(0, &[0x00, 0xEA]);
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.sp, sp_before.wrapping_sub(3)); // 3 pushes: PCH, PCL, P
}

// =============================================================================
// BRK / RTI round trip
// =============================================================================

#[test]
fn test_brk_rti_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80; // BRK vector = $8000
    bus.memory[0x8000] = 0x40; // RTI at handler
    // BRK at $0000, padding at $0001
    bus.load(0, &[0x00, 0xEA, 0xEA]); // BRK; padding; NOP (return target)
    let p_before = cpu.p;
    tick(&mut cpu, &mut bus, 7); // BRK
    assert_eq!(cpu.pc, 0x8000);
    tick(&mut cpu, &mut bus, 6); // RTI
    assert_eq!(cpu.pc, 0x0002); // Returns past BRK + padding
    // P should be restored (I was changed by BRK but restored by RTI)
    assert_eq!(
        cpu.p & 0xCF,
        p_before & 0xCF,
        "RTI should restore flags from stack"
    );
}

// =============================================================================
// Stack wrapping
// =============================================================================

#[test]
fn test_pha_stack_wraps() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00; // Bottom of stack
    cpu.a = 0x42;
    bus.load(0, &[0x48]); // PHA
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(bus.memory[0x0100], 0x42); // Written to $0100 (SP=0x00)
    assert_eq!(cpu.sp, 0xFF); // Wraps to $FF
}

#[test]
fn test_pla_stack_wraps() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFF; // Top of stack
    bus.memory[0x0100] = 0x42; // Value at $0100 (pulled when SP wraps to $00)
    bus.load(0, &[0x68]); // PLA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.sp, 0x00);
}
