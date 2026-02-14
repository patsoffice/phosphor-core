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
// Branch instructions — condition testing
// =============================================================================

#[test]
fn test_beq_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::Z as u8; // Z=1
    bus.load(0, &[0xF0, 0x04]); // BEQ +4 → PC=2+4=6
    tick(&mut cpu, &mut bus, 3); // Taken, no page cross = 3 cycles
    assert_eq!(cpu.pc, 0x06);
}

#[test]
fn test_beq_not_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::Z as u8); // Z=0
    bus.load(0, &[0xF0, 0x04, 0xEA]); // BEQ +4; NOP
    tick(&mut cpu, &mut bus, 2); // Not taken = 2 cycles
    assert_eq!(cpu.pc, 0x02); // Past the 2-byte BEQ instruction
}

#[test]
fn test_bne_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::Z as u8); // Z=0
    bus.load(0, &[0xD0, 0x02]); // BNE +2 → PC=2+2=4
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x04);
}

#[test]
fn test_bpl_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::N as u8); // N=0
    bus.load(0, &[0x10, 0x03]); // BPL +3 → PC=2+3=5
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x05);
}

#[test]
fn test_bmi_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::N as u8; // N=1
    bus.load(0, &[0x30, 0x06]); // BMI +6 → PC=2+6=8
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x08);
}

#[test]
fn test_bmi_not_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::N as u8); // N=0
    bus.load(0, &[0x30, 0x06]); // BMI +6
    tick(&mut cpu, &mut bus, 2); // Not taken
    assert_eq!(cpu.pc, 0x02);
}

#[test]
fn test_bcc_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::C as u8); // C=0
    bus.load(0, &[0x90, 0x05]); // BCC +5 → PC=2+5=7
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x07);
}

#[test]
fn test_bcs_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::C as u8; // C=1
    bus.load(0, &[0xB0, 0x02]); // BCS +2 → PC=2+2=4
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x04);
}

#[test]
fn test_bvc_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::V as u8); // V=0
    bus.load(0, &[0x50, 0x02]); // BVC +2 → PC=2+2=4
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x04);
}

#[test]
fn test_bvs_taken() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::V as u8; // V=1
    bus.load(0, &[0x70, 0x02]); // BVS +2 → PC=2+2=4
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x04);
}

// =============================================================================
// Branch timing
// =============================================================================

#[test]
fn test_branch_not_taken_2_cycles() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::Z as u8); // Z=0, BEQ not taken
    bus.load(0, &[0xF0, 0x04, 0xEA]); // BEQ +4; NOP
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x02, "Not taken branch should be 2 cycles");
}

#[test]
fn test_branch_taken_no_page_cross_3_cycles() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::Z as u8; // Z=1, BEQ taken
    bus.load(0, &[0xF0, 0x04]); // BEQ +4 → target $06 (same page)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(
        cpu.pc, 0x06,
        "Taken branch (no page cross) should be 3 cycles"
    );
}

#[test]
fn test_branch_taken_page_cross_4_cycles() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x00FD;
    cpu.p |= StatusFlag::Z as u8; // Z=1, BEQ taken
    // BEQ at $00FD: fetch reads opcode, PC=$00FE
    // Cycle 0: read offset +5 from $00FE, PC=$00FF, target=$00FF+5=$0104 (page cross!)
    bus.memory[0x00FD] = 0xF0; // BEQ
    bus.memory[0x00FE] = 0x05; // +5
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(
        cpu.pc, 0x0104,
        "Taken branch (page cross) should be 4 cycles"
    );
}

#[test]
fn test_branch_backward() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x0010;
    cpu.p &= !(StatusFlag::Z as u8); // Z=0, BNE taken
    bus.memory[0x0010] = 0xD0; // BNE
    bus.memory[0x0011] = 0xFC; // -4 (signed)
    // Target: PC after reading offset = $0012, $0012 + (-4) = $000E
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x000E);
}

#[test]
fn test_branch_backward_page_cross_4_cycles() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x0100;
    cpu.p &= !(StatusFlag::Z as u8); // Z=0, BNE taken
    bus.memory[0x0100] = 0xD0; // BNE
    bus.memory[0x0101] = 0xFC; // -4
    // Target: $0102 + (-4) = $00FE (crosses page boundary backward)
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.pc, 0x00FE);
}

// =============================================================================
// JMP
// =============================================================================

#[test]
fn test_jmp_abs() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x4C, 0x00, 0x20]); // JMP $2000
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_jmp_abs_cycle_count() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x4C, 0x00, 0x20]); // JMP $2000
    bus.memory[0x2000] = 0xEA; // NOP at target
    tick(&mut cpu, &mut bus, 3);
    // After 3 cycles, JMP is done and CPU is in Fetch state at $2000
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_jmp_ind() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x6C, 0x00, 0x10]); // JMP ($1000)
    bus.memory[0x1000] = 0x34;
    bus.memory[0x1001] = 0x12;
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn test_jmp_ind_page_wrap_bug() {
    // NMOS 6502 bug: JMP ($10FF) reads low byte from $10FF and high byte from $1000
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x6C, 0xFF, 0x10]); // JMP ($10FF)
    bus.memory[0x10FF] = 0x34; // Low byte of target
    bus.memory[0x1100] = 0x12; // WRONG: correct hardware would NOT read here
    bus.memory[0x1000] = 0x56; // RIGHT: NMOS wraps to $1000 for high byte
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(
        cpu.pc, 0x5634,
        "JMP indirect should wrap within page (NMOS bug)"
    );
}

// =============================================================================
// JSR / RTS
// =============================================================================

#[test]
fn test_jsr_jumps_to_target() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x20, 0x00, 0x20]); // JSR $2000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_jsr_pushes_return_address_minus_1() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // JSR at $0000: opcode at $0000, addr_lo at $0001, addr_hi at $0002
    // Should push $0002 (address of last byte of JSR instruction)
    bus.load(0, &[0x20, 0x00, 0x20]); // JSR $2000
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.sp, 0xFB); // Pushed 2 bytes
    // Stack: $01FD = PCH, $01FC = PCL (pushed address = $0002)
    assert_eq!(bus.memory[0x01FD], 0x00); // PCH
    assert_eq!(bus.memory[0x01FC], 0x02); // PCL
}

#[test]
fn test_rts_returns_after_jsr() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x20, 0x00, 0x20]); // JSR $2000
    bus.memory[0x2000] = 0x60; // RTS
    tick(&mut cpu, &mut bus, 6); // JSR
    assert_eq!(cpu.pc, 0x2000);
    tick(&mut cpu, &mut bus, 6); // RTS
    assert_eq!(cpu.pc, 0x0003); // Byte after JSR instruction
}

#[test]
fn test_jsr_rts_nested() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // Main: JSR $1000
    bus.load(0, &[0x20, 0x00, 0x10]); // JSR $1000 at $0000
    // Subroutine at $1000: JSR $2000
    bus.memory[0x1000] = 0x20;
    bus.memory[0x1001] = 0x00;
    bus.memory[0x1002] = 0x20; // JSR $2000
    // Subroutine at $2000: RTS
    bus.memory[0x2000] = 0x60; // RTS
    // Subroutine at $1003: RTS
    bus.memory[0x1003] = 0x60; // RTS

    tick(&mut cpu, &mut bus, 6); // JSR $1000
    assert_eq!(cpu.pc, 0x1000);
    tick(&mut cpu, &mut bus, 6); // JSR $2000
    assert_eq!(cpu.pc, 0x2000);
    tick(&mut cpu, &mut bus, 6); // RTS from $2000
    assert_eq!(cpu.pc, 0x1003); // Returns to $1003
    tick(&mut cpu, &mut bus, 6); // RTS from $1000
    assert_eq!(cpu.pc, 0x0003); // Returns to $0003
}

// =============================================================================
// RTI
// =============================================================================

#[test]
fn test_rti_restores_p_and_pc() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // Simulate interrupt: push PCH, PCL, P onto stack
    cpu.sp = 0xFA; // 3 bytes pushed from $FD
    bus.memory[0x01FB] = 0x42; // P value (with some flags set)
    bus.memory[0x01FC] = 0x00; // PCL
    bus.memory[0x01FD] = 0x20; // PCH → return to $2000
    bus.load(0, &[0x40]); // RTI
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.pc, 0x2000);
    // P restored: bit 5 (U) forced set, bit 4 (B) forced clear
    assert_eq!(cpu.p & 0xCF, 0x42 & 0xCF); // Compare meaningful flag bits
    assert_eq!(cpu.p & (StatusFlag::U as u8), StatusFlag::U as u8);
    assert_eq!(cpu.p & (StatusFlag::B as u8), 0);
}

#[test]
fn test_rti_does_not_add_1_to_pc() {
    // RTI returns to the exact address pulled (unlike RTS which adds 1)
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFA;
    bus.memory[0x01FB] = 0x24; // P
    bus.memory[0x01FC] = 0x05; // PCL
    bus.memory[0x01FD] = 0x30; // PCH → $3005
    bus.load(0, &[0x40]); // RTI
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.pc, 0x3005); // Exact address, no +1
}

#[test]
fn test_rti_restores_sp() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xFA; // 3 bytes pushed
    bus.memory[0x01FB] = 0x24; // P
    bus.memory[0x01FC] = 0x00; // PCL
    bus.memory[0x01FD] = 0x20; // PCH
    bus.load(0, &[0x40]); // RTI
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.sp, 0xFD); // SP restored (3 pulls)
}
