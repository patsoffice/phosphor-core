/// Tests for M6800 branch, jump, and subroutine instructions.
///
/// Cycle counts:
/// - Conditional branches: 4 cycles (always, taken or not)
/// - BSR: 8 cycles
/// - JMP indexed: 4 cycles, JMP extended: 3 cycles
/// - JSR indexed: 8 cycles, JSR extended: 9 cycles
/// - RTS: 5 cycles
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};

mod common;
use common::TestBus;

fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// BRA (0x20) - Branch always - 4 cycles
// =============================================================================

#[test]
fn test_bra_forward() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // BRA +5: PC after fetch of offset = 2, so target = 2 + 5 = 7
    bus.load(0, &[0x20, 0x05]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 7);
}

#[test]
fn test_bra_backward() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Place BRA at address 0x10 with offset -4 (0xFC)
    // PC after fetch of offset = 0x12, target = 0x12 + (-4) = 0x0E
    bus.load(0x10, &[0x20, 0xFC]);
    cpu.pc = 0x10;
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 0x0E);
}

// =============================================================================
// BHI (0x22) - Branch if higher (C=0 AND Z=0) - 4 cycles
// =============================================================================

#[test]
fn test_bhi_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // C=0, Z=0 → taken
    bus.load(0, &[0x22, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6); // 2 + 4
}

#[test]
fn test_bhi_not_taken_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8; // C=1 → not taken
    bus.load(0, &[0x22, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2); // falls through
}

#[test]
fn test_bhi_not_taken_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::Z as u8; // Z=1 → not taken
    bus.load(0, &[0x22, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BLS (0x23) - Branch if lower or same (C=1 OR Z=1) - 4 cycles
// =============================================================================

#[test]
fn test_bls_taken_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8;
    bus.load(0, &[0x23, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bls_taken_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::Z as u8;
    bus.load(0, &[0x23, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bls_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // C=0, Z=0 → not taken
    bus.load(0, &[0x23, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BCC (0x24) / BCS (0x25) - Branch on carry clear/set - 4 cycles
// =============================================================================

#[test]
fn test_bcc_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // C=0 → taken
    bus.load(0, &[0x24, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bcc_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8;
    bus.load(0, &[0x24, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_bcs_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::C as u8;
    bus.load(0, &[0x25, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bcs_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x25, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BNE (0x26) / BEQ (0x27) - Branch on zero clear/set - 4 cycles
// =============================================================================

#[test]
fn test_bne_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Z=0 → taken
    bus.load(0, &[0x26, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bne_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::Z as u8;
    bus.load(0, &[0x26, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_beq_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::Z as u8;
    bus.load(0, &[0x27, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_beq_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x27, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BVC (0x28) / BVS (0x29) - Branch on overflow clear/set - 4 cycles
// =============================================================================

#[test]
fn test_bvc_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x28, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bvc_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::V as u8;
    bus.load(0, &[0x28, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_bvs_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::V as u8;
    bus.load(0, &[0x29, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bvs_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x29, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BPL (0x2A) / BMI (0x2B) - Branch on plus/minus - 4 cycles
// =============================================================================

#[test]
fn test_bpl_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=0 → taken
    bus.load(0, &[0x2A, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bpl_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::N as u8;
    bus.load(0, &[0x2A, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_bmi_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::N as u8;
    bus.load(0, &[0x2B, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bmi_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x2B, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BGE (0x2C) - Branch if >= signed (N XOR V = 0) - 4 cycles
// =============================================================================

#[test]
fn test_bge_taken_both_clear() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=0, V=0 → N XOR V = 0 → taken
    bus.load(0, &[0x2C, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bge_taken_both_set() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=1, V=1 → N XOR V = 0 → taken
    cpu.cc |= CcFlag::N as u8 | CcFlag::V as u8;
    bus.load(0, &[0x2C, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bge_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=1, V=0 → N XOR V = 1 → not taken
    cpu.cc |= CcFlag::N as u8;
    bus.load(0, &[0x2C, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BLT (0x2D) - Branch if < signed (N XOR V = 1) - 4 cycles
// =============================================================================

#[test]
fn test_blt_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=1, V=0 → N XOR V = 1 → taken
    cpu.cc |= CcFlag::N as u8;
    bus.load(0, &[0x2D, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_blt_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=0, V=0 → N XOR V = 0 → not taken
    bus.load(0, &[0x2D, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BGT (0x2E) - Branch if > signed (Z=0 AND N XOR V = 0) - 4 cycles
// =============================================================================

#[test]
fn test_bgt_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Z=0, N=0, V=0 → taken
    bus.load(0, &[0x2E, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_bgt_not_taken_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Z=1 → not taken (even though N XOR V = 0)
    cpu.cc |= CcFlag::Z as u8;
    bus.load(0, &[0x2E, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_bgt_not_taken_sign() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Z=0, N=1, V=0 → N XOR V = 1 → not taken
    cpu.cc |= CcFlag::N as u8;
    bus.load(0, &[0x2E, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// BLE (0x2F) - Branch if <= signed (Z=1 OR N XOR V = 1) - 4 cycles
// =============================================================================

#[test]
fn test_ble_taken_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.cc |= CcFlag::Z as u8;
    bus.load(0, &[0x2F, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_ble_taken_sign() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // N=0, V=1 → N XOR V = 1 → taken
    cpu.cc |= CcFlag::V as u8;
    bus.load(0, &[0x2F, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_ble_not_taken() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Z=0, N=0, V=0 → not taken
    bus.load(0, &[0x2F, 0x04]);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// JMP indexed (0x6E) - 4 cycles
// =============================================================================

#[test]
fn test_jmp_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1000;
    bus.load(0, &[0x6E, 0x20]); // JMP $20,X → 0x1020
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 0x1020);
}

#[test]
fn test_jmp_idx_zero_offset() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x4000;
    bus.load(0, &[0x6E, 0x00]); // JMP 0,X → 0x4000
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 0x4000);
}

// =============================================================================
// JMP extended (0x7E) - 3 cycles
// =============================================================================

#[test]
fn test_jmp_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x7E, 0x20, 0x00]); // JMP $2000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_jmp_ext_high_addr() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x7E, 0xFF, 0x00]); // JMP $FF00
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0xFF00);
}

// =============================================================================
// BSR (0x8D) - Branch to subroutine - 8 cycles
// =============================================================================

#[test]
fn test_bsr_pushes_return_and_branches() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    // BSR +4: PC after offset fetch = 2, pushes 0x0002, branches to 2+4 = 6
    bus.load(0, &[0x8D, 0x04]);
    tick(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.pc, 6);
    // Stack should contain return address (0x0002) pushed PCL first, then PCH
    // SP started at 0xFF, pushed PCL at 0xFF, then PCH at 0xFE
    assert_eq!(bus.memory[0x00FF], 0x02); // PCL
    assert_eq!(bus.memory[0x00FE], 0x00); // PCH
    assert_eq!(cpu.sp, 0x00FD);
}

#[test]
fn test_bsr_backward() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    cpu.pc = 0x0020;
    // BSR -8 (0xF8): PC after offset fetch = 0x22, target = 0x22 + (-8) = 0x1A
    bus.load(0x20, &[0x8D, 0xF8]);
    tick(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.pc, 0x1A);
    assert_eq!(bus.memory[0x00FF], 0x22); // PCL
    assert_eq!(bus.memory[0x00FE], 0x00); // PCH
    assert_eq!(cpu.sp, 0x00FD);
}

// =============================================================================
// JSR indexed (0xAD) - 8 cycles
// =============================================================================

#[test]
fn test_jsr_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    cpu.x = 0x1000;
    // JSR $10,X: target = 0x1010, return addr = PC after reading offset = 2
    bus.load(0, &[0xAD, 0x10]);
    tick(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.pc, 0x1010);
    // Return address 0x0002 on stack
    assert_eq!(bus.memory[0x00FF], 0x02); // PCL
    assert_eq!(bus.memory[0x00FE], 0x00); // PCH
    assert_eq!(cpu.sp, 0x00FD);
}

// =============================================================================
// JSR extended (0xBD) - 9 cycles
// =============================================================================

#[test]
fn test_jsr_ext() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    // JSR $3000: return addr = PC after reading 2-byte address = 3
    bus.load(0, &[0xBD, 0x30, 0x00]);
    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x3000);
    // Return address 0x0003 on stack
    assert_eq!(bus.memory[0x00FF], 0x03); // PCL
    assert_eq!(bus.memory[0x00FE], 0x00); // PCH
    assert_eq!(cpu.sp, 0x00FD);
}

// =============================================================================
// RTS (0x39) - Return from subroutine - 5 cycles
// =============================================================================

#[test]
fn test_rts() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Simulate stack with return address 0x1234
    cpu.sp = 0x00FD; // two bytes on stack
    bus.memory[0x00FE] = 0x12; // PCH
    bus.memory[0x00FF] = 0x34; // PCL
    bus.load(0, &[0x39]); // RTS
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cpu.sp, 0x00FF);
}

// =============================================================================
// JSR + RTS roundtrip
// =============================================================================

#[test]
fn test_jsr_ext_rts_roundtrip() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    // At 0x0000: JSR $0100 (3 bytes)
    bus.load(0, &[0xBD, 0x01, 0x00]);
    // At 0x0100: RTS
    bus.load(0x0100, &[0x39]);
    // Execute JSR (9 cycles)
    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x0100);
    assert_eq!(cpu.sp, 0x00FD);
    // Execute RTS (5 cycles)
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0003); // returns to instruction after JSR
    assert_eq!(cpu.sp, 0x00FF);
}

#[test]
fn test_bsr_rts_roundtrip() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    // At 0x0000: BSR +0x0E → target = 0x02 + 0x0E = 0x10
    bus.load(0, &[0x8D, 0x0E]);
    // At 0x0010: RTS
    bus.load(0x10, &[0x39]);
    // Execute BSR (8 cycles)
    tick(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.pc, 0x0010);
    // Execute RTS (5 cycles)
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0002); // returns to instruction after BSR
    assert_eq!(cpu.sp, 0x00FF);
}

#[test]
fn test_jsr_idx_rts_roundtrip() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    cpu.x = 0x0200;
    // At 0x0000: JSR $10,X → target = 0x0210
    bus.load(0, &[0xAD, 0x10]);
    // At 0x0210: RTS
    bus.load(0x0210, &[0x39]);
    // Execute JSR indexed (8 cycles)
    tick(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.pc, 0x0210);
    // Execute RTS (5 cycles)
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0002);
    assert_eq!(cpu.sp, 0x00FF);
}

// =============================================================================
// Nested JSR/RTS
// =============================================================================

#[test]
fn test_nested_jsr_rts() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x00FF;
    // At 0x0000: JSR $0100
    bus.load(0, &[0xBD, 0x01, 0x00]);
    // At 0x0100: JSR $0200
    bus.load(0x0100, &[0xBD, 0x02, 0x00]);
    // At 0x0200: RTS
    bus.load(0x0200, &[0x39]);
    // At 0x0103: RTS (after inner JSR returns)
    bus.load(0x0103, &[0x39]);

    // Execute outer JSR (9 cycles)
    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x0100);
    assert_eq!(cpu.sp, 0x00FD);

    // Execute inner JSR (9 cycles)
    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x0200);
    assert_eq!(cpu.sp, 0x00FB);

    // Execute inner RTS (5 cycles) — returns to 0x0103
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0103);
    assert_eq!(cpu.sp, 0x00FD);

    // Execute outer RTS (5 cycles) — returns to 0x0003
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0003);
    assert_eq!(cpu.sp, 0x00FF);
}

// =============================================================================
// Branch with ALU instruction (integration)
// =============================================================================

#[test]
fn test_branch_after_compare() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // CMPA #5; BEQ +2; LDAA #0xFF; ...
    // If A == 5, skip the LDAA and land at PC=7
    cpu.a = 5;
    bus.load(
        0,
        &[
            0x81, 0x05, // CMPA #5 (2 cycles) → Z=1
            0x27, 0x02, // BEQ +2 (4 cycles) → taken, target = 4+2 = 6
            0x86, 0xFF, // LDAA #0xFF (skipped)
            0x01, // NOP (target)
        ],
    );
    tick(&mut cpu, &mut bus, 2); // CMPA
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    tick(&mut cpu, &mut bus, 4); // BEQ
    assert_eq!(cpu.pc, 6);
}

#[test]
fn test_branch_loop_decrement() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    // Simple loop: DECA; BNE -3 (loops back to DECA)
    // At addr 0: DECA (2 cycles)
    // At addr 1: BNE offset (4 cycles) — offset = -3 (0xFD) → PC after offset = 3, 3+(-3)=0
    cpu.a = 3;
    bus.load(0, &[0x4A, 0x26, 0xFD]);

    // Iteration 1: A=3→2
    tick(&mut cpu, &mut bus, 2); // DECA
    assert_eq!(cpu.a, 2);
    tick(&mut cpu, &mut bus, 4); // BNE → taken (Z=0)
    assert_eq!(cpu.pc, 0);

    // Iteration 2: A=2→1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 1);
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 0);

    // Iteration 3: A=1→0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0);
    tick(&mut cpu, &mut bus, 4); // BNE → not taken (Z=1)
    assert_eq!(cpu.pc, 3); // falls through
}
