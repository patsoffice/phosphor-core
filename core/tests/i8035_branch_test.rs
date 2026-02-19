use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8035::{I8035, PswFlag};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` machine cycles.
fn tick(cpu: &mut I8035, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// JMP addr11 (0x04/0x24/0x44/0x64/0x84/0xA4/0xC4/0xE4) — 2 cycles
// =============================================================================

#[test]
fn test_jmp_page0() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // JMP to 0x050: opcode has bits[7:5]=000, a11=0
    // Target = (0 << 11) | (0x04 & 0xE0) << 3 | addr_byte
    // 0x04 & 0xE0 = 0x00, so target = 0x000 | addr_byte
    bus.load(0, &[0x04, 0x50]); // JMP 0x050
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x050);
}

#[test]
fn test_jmp_page2() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // 0x44: bits[7:5] = 010 → page bits = 0x200
    bus.load(0, &[0x44, 0x30]); // JMP 0x230
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x230);
}

#[test]
fn test_jmp_with_mb1() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // SEL MB1 (0xF5), then JMP: A11 should be set
    bus.load(0, &[0xF5, 0x04, 0x10]); // SEL MB1; JMP 0x010
    tick(&mut cpu, &mut bus, 1); // SEL MB1
    tick(&mut cpu, &mut bus, 2); // JMP
    assert_eq!(cpu.pc, 0x810); // 0x800 | 0x010
}

// =============================================================================
// CALL addr11 (0x14/0x34/...) — 2 cycles
// =============================================================================

#[test]
fn test_call_and_ret() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // CALL 0x100, then at 0x100 place a RET
    // 0x14: bits[7:5] = 000, so target page bits = 0x000
    // Wait, 0x14 & 0xE0 = 0x00, so target = 0x000 | addr_byte
    // We need the CALL to go to page 1: opcode 0x34 → bits[7:5]=001 → page=0x100
    bus.load(0, &[0x34, 0x00]); // CALL 0x100
    bus.load(0x100, &[0x83]); // RET
    tick(&mut cpu, &mut bus, 2); // CALL
    assert_eq!(cpu.pc, 0x100);
    // Stack should have return address (PC after CALL = 0x002) and PSW
    assert_eq!(cpu.psw & 0x07, 1); // SP incremented to 1

    tick(&mut cpu, &mut bus, 2); // RET
    assert_eq!(cpu.pc, 0x002); // return address
    assert_eq!(cpu.psw & 0x07, 0); // SP decremented
}

#[test]
fn test_call_preserves_psw_on_stack() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::CY as u8 | PswFlag::F0 as u8; // CY + F0 = 0xA0
    bus.load(0, &[0x14, 0x50]); // CALL 0x050
    tick(&mut cpu, &mut bus, 2);
    // Stack entry at RAM[8..10]: byte0=PC_lo, byte1=PSW_hi|PC_hi
    assert_eq!(cpu.ram[8], 0x02); // return PC low byte
    assert_eq!(cpu.ram[9], 0xA0); // PSW[7:4]|PC[11:8] = 0xA0|0x00
}

// =============================================================================
// RET (0x83) — 2 cycles
// =============================================================================

#[test]
fn test_ret_restores_pc() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // Manually push a return address
    cpu.ram[8] = 0x42; // PC low
    cpu.ram[9] = 0x03; // PC[11:8] = 0x3, PSW upper nibble doesn't matter for RET
    cpu.psw = 0x01; // SP = 1
    bus.load(0, &[0x83]); // RET
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x342);
    assert_eq!(cpu.psw & 0x07, 0); // SP decremented
}

// =============================================================================
// RETR (0x93) — 2 cycles
// =============================================================================

#[test]
fn test_retr_restores_pc_and_psw() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // Push return with CY+F0 in PSW upper nibble
    cpu.ram[8] = 0x10; // PC low
    cpu.ram[9] = 0xA2; // PSW[7:4]=0xA0 (CY+F0), PC[11:8]=0x02
    cpu.psw = 0x01; // SP = 1
    cpu.in_interrupt = true;
    bus.load(0, &[0x93]); // RETR
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x210);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0); // CY restored
    assert_ne!(cpu.psw & (PswFlag::F0 as u8), 0); // F0 restored
    assert!(!cpu.in_interrupt); // cleared by RETR
}

// =============================================================================
// JMPP @A (0xB3) — 2 cycles
// =============================================================================

#[test]
fn test_jmpp() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    // PC after fetching opcode at 0x000 is 0x001, page = 0x000
    // Lookup: bus.read(0x000 | 0x10) = bus.read(0x010)
    bus.memory[0x10] = 0x42;
    bus.load(0, &[0xB3]); // JMPP @A
    tick(&mut cpu, &mut bus, 2);
    // PC = page | lookup_result = 0x000 | 0x42 = 0x042
    assert_eq!(cpu.pc, 0x042);
}

#[test]
fn test_jmpp_page_relative() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x200;
    cpu.a = 0x05;
    // page = 0x200, lookup at 0x200 | 0x05 = 0x205
    bus.memory[0x205] = 0x80;
    bus.load(0x200, &[0xB3]); // JMPP @A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x280); // 0x200 | 0x80
}

// =============================================================================
// DJNZ Rn,addr (0xE8-0xEF) — 2 cycles
// =============================================================================

#[test]
fn test_djnz_branches() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x02; // R0 = 2
    bus.load(0, &[0xE8, 0x00]); // DJNZ R0, 0x00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ram[0], 0x01); // R0 decremented
    assert_eq!(cpu.pc, 0x00); // jumped (R0 != 0)
}

#[test]
fn test_djnz_falls_through() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x01; // R0 = 1
    bus.load(0, &[0xE8, 0x00]); // DJNZ R0, 0x00
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ram[0], 0x00); // R0 decremented to 0
    assert_eq!(cpu.pc, 0x02); // fell through (R0 == 0)
}

#[test]
fn test_djnz_loop() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // Small loop: INC A; DJNZ R0, 0x00
    cpu.ram[0] = 0x03; // R0 = 3 iterations
    bus.load(0, &[0x17, 0xE8, 0x00]); // INC A; DJNZ R0, 0x00
    // Iteration 1: INC A (1 cycle), DJNZ (2 cycles) → R0=2, jump to 0x00
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 1);
    assert_eq!(cpu.ram[0], 2);
    // Iteration 2
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 2);
    assert_eq!(cpu.ram[0], 1);
    // Iteration 3: R0 becomes 0, falls through
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 3);
    assert_eq!(cpu.ram[0], 0);
    assert_eq!(cpu.pc, 3); // past the loop
}

// =============================================================================
// JC addr (0xF6) — 2 cycles
// =============================================================================

#[test]
fn test_jc_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0xF6, 0x50]); // JC 0x50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x050);
}

#[test]
fn test_jc_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // CY clear
    bus.load(0, &[0xF6, 0x50]); // JC 0x50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002); // fell through
}

// =============================================================================
// JNC addr (0xE6) — 2 cycles
// =============================================================================

#[test]
fn test_jnc_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // CY clear
    bus.load(0, &[0xE6, 0x30]); // JNC 0x30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x030);
}

#[test]
fn test_jnc_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0xE6, 0x30]); // JNC 0x30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JZ addr (0xC6) — 2 cycles
// =============================================================================

#[test]
fn test_jz_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0;
    bus.load(0, &[0xC6, 0x40]); // JZ 0x40
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x040);
}

#[test]
fn test_jz_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 1;
    bus.load(0, &[0xC6, 0x40]); // JZ 0x40
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JNZ addr (0x96) — 2 cycles
// =============================================================================

#[test]
fn test_jnz_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x96, 0x20]); // JNZ 0x20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x020);
}

#[test]
fn test_jnz_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0;
    bus.load(0, &[0x96, 0x20]); // JNZ 0x20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JF0 addr (0xB6) — 2 cycles
// =============================================================================

#[test]
fn test_jf0_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::F0 as u8;
    bus.load(0, &[0xB6, 0x60]); // JF0 0x60
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x060);
}

#[test]
fn test_jf0_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xB6, 0x60]); // JF0 0x60
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JF1 addr (0x76) — 2 cycles
// =============================================================================

#[test]
fn test_jf1_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.f1 = true;
    bus.load(0, &[0x76, 0x70]); // JF1 0x70
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x070);
}

#[test]
fn test_jf1_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x76, 0x70]); // JF1 0x70
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JT0 addr (0x36) / JNT0 addr (0x26) — 2 cycles
// =============================================================================

#[test]
fn test_jt0_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // PORT_T0 = 0x110; TestBus maps io_read to read, so set memory[0x110]
    bus.memory[0x110] = 1; // T0 high
    bus.load(0, &[0x36, 0x80]); // JT0 0x80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x080);
}

#[test]
fn test_jt0_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x110] = 0; // T0 low
    bus.load(0, &[0x36, 0x80]); // JT0 0x80
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

#[test]
fn test_jnt0_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x110] = 0; // T0 low
    bus.load(0, &[0x26, 0x90]); // JNT0 0x90
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x090);
}

// =============================================================================
// JT1 addr (0x56) / JNT1 addr (0x46) — 2 cycles
// =============================================================================

#[test]
fn test_jt1_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // PORT_T1 = 0x111
    bus.memory[0x111] = 1; // T1 high
    bus.load(0, &[0x56, 0xA0]); // JT1 0xA0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x0A0);
}

#[test]
fn test_jnt1_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x111] = 0; // T1 low
    bus.load(0, &[0x46, 0xB0]); // JNT1 0xB0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x0B0);
}

// =============================================================================
// JTF addr (0x16) — 2 cycles, auto-clears timer flag
// =============================================================================

#[test]
fn test_jtf_taken_and_clears() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.timer_overflow = true;
    bus.load(0, &[0x16, 0x20]); // JTF 0x20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x020);
    assert!(!cpu.timer_overflow); // auto-cleared
}

#[test]
fn test_jtf_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.timer_overflow = false;
    bus.load(0, &[0x16, 0x20]); // JTF 0x20
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JNI addr (0x86) — 2 cycles
// =============================================================================

#[test]
fn test_jni_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.irq = true; // INT asserted
    bus.load(0, &[0x86, 0x50]); // JNI 0x50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x050);
}

#[test]
fn test_jni_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.irq = false;
    bus.load(0, &[0x86, 0x50]); // JNI 0x50
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

// =============================================================================
// JBb addr (0x12/0x32/0x52/0x72/0x92/0xB2/0xD2/0xF2) — 2 cycles
// =============================================================================

#[test]
fn test_jb0_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01; // bit 0 set
    bus.load(0, &[0x12, 0x40]); // JB0 0x40
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x040);
}

#[test]
fn test_jb0_not_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFE; // bit 0 clear
    bus.load(0, &[0x12, 0x40]); // JB0 0x40
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x002);
}

#[test]
fn test_jb7_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // bit 7 set
    // 0xF2: opcode >> 5 = 7
    bus.load(0, &[0xF2, 0x30]); // JB7 0x30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x030);
}

#[test]
fn test_jb3_taken() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x08; // bit 3 set
    // 0x72: opcode >> 5 = 3
    bus.load(0, &[0x72, 0x60]); // JB3 0x60
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x060);
}

// =============================================================================
// Jump page boundary: conditional jump target is page-relative
// =============================================================================

#[test]
fn test_jump_page_relative() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x1FE; // near end of page 1
    cpu.a = 0;
    // JZ at 0x1FE with target byte 0x05
    // page = 0x1FE & 0xF00 = 0x100
    // After opcode fetch, PC = 0x1FF
    // After addr byte read, PC = 0x200 (but we use the captured page)
    // Target = 0x100 | 0x05 = 0x105
    bus.load(0x1FE, &[0xC6, 0x05]); // JZ 0x05
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x105);
}

// =============================================================================
// Nested CALL/RET
// =============================================================================

#[test]
fn test_nested_calls() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // main: CALL 0x100
    bus.load(0, &[0x34, 0x00]); // CALL to page 1 = 0x100
    // sub1: CALL 0x200
    bus.load(0x100, &[0x54, 0x00]); // CALL to page 2 = 0x200
    // sub2: RET
    bus.load(0x200, &[0x83]);
    // sub1 cont: RET
    bus.load(0x102, &[0x83]);

    tick(&mut cpu, &mut bus, 2); // CALL sub1
    assert_eq!(cpu.pc, 0x100);
    assert_eq!(cpu.psw & 0x07, 1); // SP=1

    tick(&mut cpu, &mut bus, 2); // CALL sub2
    assert_eq!(cpu.pc, 0x200);
    assert_eq!(cpu.psw & 0x07, 2); // SP=2

    tick(&mut cpu, &mut bus, 2); // RET from sub2
    assert_eq!(cpu.pc, 0x102);
    assert_eq!(cpu.psw & 0x07, 1); // SP=1

    tick(&mut cpu, &mut bus, 2); // RET from sub1
    assert_eq!(cpu.pc, 0x002);
    assert_eq!(cpu.psw & 0x07, 0); // SP=0
}
