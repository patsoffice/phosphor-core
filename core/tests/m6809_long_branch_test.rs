use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

#[test]
fn test_lbeq_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::Z as u8;
    // LBEQ $0010 (0x10 0x27 0x00 0x10)
    bus.load(0, &[0x10, 0x27, 0x00, 0x10]);

    // 6 cycles (taken: 2 prefix + 4 execute)
    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // PC = 0x04 (past instruction) + 0x0010 (offset) = 0x0014
    assert_eq!(cpu.pc, 0x0014, "PC should branch forward");
}

#[test]
fn test_lbeq_not_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Z=0 (default), so LBEQ should not be taken
    bus.load(0, &[0x10, 0x27, 0x00, 0x10]);

    // 5 cycles (not taken: 2 prefix + 3 execute)
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // PC should be just past the instruction (4 bytes)
    assert_eq!(cpu.pc, 0x0004, "PC should not branch");
}

#[test]
fn test_lbne_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Z=0 (default), so LBNE should be taken
    // LBNE $0020 (0x10 0x26 0x00 0x20)
    bus.load(0, &[0x10, 0x26, 0x00, 0x20]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x0024, "PC should branch forward");
}

#[test]
fn test_lbrn_never() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LBRN $1000 — never branches regardless of flags
    bus.load(0, &[0x10, 0x21, 0x10, 0x00]);

    // 5 cycles (never taken)
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x0004, "LBRN should never branch");
}

#[test]
fn test_lbmi_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::N as u8;
    // LBMI $0008 (0x10 0x2B 0x00 0x08)
    bus.load(0, &[0x10, 0x2B, 0x00, 0x08]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x000C, "PC should branch to 0x04 + 0x08");
}

#[test]
fn test_lbcs_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::C as u8;
    // LBCS $0004 (0x10 0x25 0x00 0x04)
    bus.load(0, &[0x10, 0x25, 0x00, 0x04]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x0008);
}

#[test]
fn test_lbhi_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // C=0 and Z=0 (default), so LBHI should be taken
    // LBHI $0010 (0x10 0x22 0x00 0x10)
    bus.load(0, &[0x10, 0x22, 0x00, 0x10]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x0014);
}

#[test]
fn test_lbge_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // N=0, V=0 → N==V → taken
    // LBGE $0010 (0x10 0x2C 0x00 0x10)
    bus.load(0, &[0x10, 0x2C, 0x00, 0x10]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.pc, 0x0014);
}

#[test]
fn test_lbeq_large_forward_offset() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::Z as u8;
    // LBEQ $0200 — offset larger than 8-bit max
    bus.load(0, &[0x10, 0x27, 0x02, 0x00]);

    for _ in 0..6 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // PC = 0x04 + 0x0200 = 0x0204
    assert_eq!(cpu.pc, 0x0204, "16-bit offset should work");
}

#[test]
fn test_lbeq_backward_offset() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::Z as u8;
    // Place LBEQ at offset 0x10, branch backward by 0x08
    // 0xFFF8 = -8 as i16
    bus.load(0x10, &[0x10, 0x27, 0xFF, 0xF8]);

    // Start execution at 0x10
    // We need to set PC to 0x10 first — use a JMP or set directly
    // Easiest: fill ROM from 0 with NOPs (we don't have NOP, use BRN as 3-byte NOP)
    // Actually, just load at 0 and pad with known instructions
    // Simpler: load instruction at address 0 and use a negative offset
    let mut cpu2 = M6809::new();
    let mut bus2 = TestBus::new();
    cpu2.cc = CcFlag::Z as u8;
    // LBEQ $FFF0 at address 0 — offset = -16 = 0xFFF0
    // PC after instruction = 0x04, so target = 0x04 + 0xFFF0 = 0xFFF4 (wraps)
    bus2.load(0, &[0x10, 0x27, 0xFF, 0xF0]);

    for _ in 0..6 {
        cpu2.tick_with_bus(&mut bus2, BusMaster::Cpu(0));
    }

    assert_eq!(cpu2.pc, 0xFFF4, "Negative offset should wrap PC");
}
