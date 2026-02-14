use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

fn tick(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

#[test]
fn test_bra_forward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x00: BRA $02 (skip next 2 bytes)
    // 0x02: NOP (0x12) - skipped
    // 0x03: NOP (0x12) - skipped
    // 0x04: LDA #$42
    bus.load(0, &[0x20, 0x02, 0x12, 0x12, 0x86, 0x42]);

    // BRA (3 cycles)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.pc, 0x04);

    // LDA (2 cycles)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_bra_backward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x00: BRA $00 (infinite loop to self)
    bus.load(0, &[0x20, 0xFE]); // 0xFE is -2

    // Execute BRA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // PC should be back at 0x00 (0x02 + (-2) = 0x00)
    assert_eq!(cpu.pc, 0x00);
}

#[test]
fn test_beq_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$00 (sets Z), BEQ $02
    bus.load(0, &[0x86, 0x00, 0x27, 0x02, 0x12, 0x12, 0x86, 0x42]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BEQ
    assert_eq!(cpu.pc, 0x06); // 0x04 + 2 = 0x06
}

#[test]
fn test_beq_not_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01 (clears Z), BEQ $02
    bus.load(0, &[0x86, 0x01, 0x27, 0x02, 0x86, 0x42]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BEQ (not taken)
    assert_eq!(cpu.pc, 0x04); // 0x04 + 0 (not taken) -> 0x04

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // Next instruction (LDA #$42)
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_bne_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01 (clears Z), BNE $02
    bus.load(0, &[0x86, 0x01, 0x26, 0x02, 0x12, 0x12, 0x86, 0x42]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BNE
    assert_eq!(cpu.pc, 0x06);
}

#[test]
fn test_bmi_taken() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$80 (sets N), BMI $02
    bus.load(0, &[0x86, 0x80, 0x2B, 0x02, 0x12, 0x12, 0x86, 0x42]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BMI
    assert_eq!(cpu.pc, 0x06);
}

#[test]
fn test_brn_never() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // BRN $02 (should not branch)
    bus.load(0, &[0x21, 0x02, 0x86, 0x42]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BRN
    assert_eq!(cpu.pc, 0x02); // 0x02 (next instruction)

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$42
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_bsr_and_rts() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // 0x00: BSR $04      -> branch to 0x06 (0x02 + 0x04), push return addr 0x0002
    // 0x02: LDA #$42     -> executed after RTS returns here
    // 0x04: BRA $FE      -> infinite loop (sentinel, should not reach)
    // 0x06: LDA #$99     -> subroutine body
    // 0x08: RTS           -> return to 0x0002
    bus.load(
        0,
        &[
            0x8D, 0x04, // BSR $04
            0x86, 0x42, // LDA #$42
            0x20, 0xFE, // BRA self (sentinel)
            0x86, 0x99, // LDA #$99 (subroutine)
            0x39, // RTS
        ],
    );

    // BSR: 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x06, "PC should be at subroutine");
    assert_eq!(cpu.s, 0x7EFE, "S should have decremented by 2");
    // Stack should contain return address 0x0002 (high at lower addr)
    assert_eq!(bus.memory[0x7EFE], 0x00, "Stack high byte of return addr");
    assert_eq!(bus.memory[0x7EFF], 0x02, "Stack low byte of return addr");

    // LDA #$99: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x99);

    // RTS: 5 cycles
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x02, "PC should return to 0x0002");
    assert_eq!(cpu.s, 0x7F00, "S should be restored");

    // LDA #$42: 2 cycles (instruction after BSR)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_bsr_backward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // 0x00: LDA #$11      -> first instruction
    // 0x02: BRA $03        -> skip to 0x07
    // 0x04: LDA #$22       -> subroutine (backward target)
    // 0x06: RTS
    // 0x07: BSR $FB        -> branch backward to 0x04 (0x09 + (-5) = 0x04)
    // 0x09: LDA #$33
    bus.load(
        0,
        &[
            0x86, 0x11, // LDA #$11
            0x20, 0x03, // BRA $03 (skip to 0x07)
            0x86, 0x22, // LDA #$22 (subroutine at 0x04)
            0x39, // RTS (at 0x06)
            0x8D, 0xFB, // BSR $FB (at 0x07) -> 0x09 + (-5) = 0x04
            0x86, 0x33, // LDA #$33 (at 0x09, after return)
        ],
    );

    // LDA #$11: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x11);

    // BRA $03: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.pc, 0x07, "Should jump past subroutine");

    // BSR backward: 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x04, "Should branch backward to subroutine");

    // LDA #$22: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x22);

    // RTS: 5 cycles
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x09, "Should return after BSR");

    // LDA #$33: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x33);
}

#[test]
fn test_jsr_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // DP=0 (default), so JSR $20 jumps to 0x0020
    // 0x00: JSR $20       -> jump to subroutine at 0x0020, push return addr 0x0002
    // 0x02: LDA #$42      -> executed after RTS
    // ...
    // 0x20: LDA #$99      -> subroutine body
    // 0x22: RTS            -> return to 0x0002
    let mut rom = [0u8; 0x30];
    rom[0x00] = 0x9D; // JSR direct
    rom[0x01] = 0x20; // address $20
    rom[0x02] = 0x86; // LDA immediate
    rom[0x03] = 0x42; // #$42
    rom[0x20] = 0x86; // LDA immediate
    rom[0x21] = 0x99; // #$99
    rom[0x22] = 0x39; // RTS
    bus.load(0, &rom);

    // JSR direct: 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x20, "PC should be at subroutine");
    assert_eq!(cpu.s, 0x7EFE, "S should have decremented by 2");
    assert_eq!(bus.memory[0x7EFE], 0x00, "Stack high byte of return addr");
    assert_eq!(bus.memory[0x7EFF], 0x02, "Stack low byte of return addr");

    // LDA #$99: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x99);

    // RTS: 5 cycles
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x02, "PC should return to caller");
    assert_eq!(cpu.s, 0x7F00, "S should be restored");

    // LDA #$42: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_nested_bsr() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // Test nested subroutine calls:
    // 0x00: BSR $04       -> call sub1 at 0x06
    // 0x02: LDA #$33      -> final result
    // 0x04: BRA $FE        -> sentinel
    // 0x06: BSR $01        -> sub1: call sub2 at 0x09 (0x08 + 0x01)
    // 0x08: RTS            -> sub1: return
    // 0x09: LDA #$77       -> sub2: body
    // 0x0B: RTS            -> sub2: return
    bus.load(
        0,
        &[
            0x8D, 0x04, // BSR $04 -> 0x06
            0x86, 0x33, // LDA #$33
            0x20, 0xFE, // BRA self
            0x8D, 0x01, // BSR $01 -> 0x09
            0x39, // RTS
            0x86, 0x77, // LDA #$77
            0x39, // RTS
        ],
    );

    // BSR to sub1: 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x06);
    assert_eq!(cpu.s, 0x7EFE);

    // BSR to sub2 (nested): 7 cycles
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x09);
    assert_eq!(cpu.s, 0x7EFC, "S should decrement by 4 total");

    // LDA #$77: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x77);

    // RTS from sub2: 5 cycles -> return to 0x08
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x08);
    assert_eq!(cpu.s, 0x7EFE);

    // RTS from sub1: 5 cycles -> return to 0x02
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }
    assert_eq!(cpu.pc, 0x02);
    assert_eq!(cpu.s, 0x7F00);

    // LDA #$33: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x33);
}

// ============================================================
// LBRA (0x16) - Long Branch Always
// ============================================================

#[test]
fn test_lbra_forward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x00: LBRA $0100 -> branch to 0x0003 + 0x0100 = 0x0103
    bus.load(0, &[0x16, 0x01, 0x00]);
    bus.memory[0x0103] = 0x86; // LDA #$42
    bus.memory[0x0104] = 0x42;

    // LBRA: 5 cycles (1 fetch + 4 execute)
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0103, "PC should branch forward to 0x0103");

    // Verify we can execute at the target
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_lbra_backward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Start at 0x0200, LBRA with offset -0x0100 (0xFF00)
    cpu.pc = 0x0200;
    bus.memory[0x0200] = 0x16; // LBRA
    bus.memory[0x0201] = 0xFF; // offset high (-256 = 0xFF00)
    bus.memory[0x0202] = 0x00; // offset low
    // Target: 0x0203 + 0xFF00 = 0x0103
    bus.memory[0x0103] = 0x86; // LDA #$99
    bus.memory[0x0104] = 0x99;

    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x0103, "PC should branch backward to 0x0103");

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x99);
}

#[test]
fn test_lbra_zero_offset() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LBRA $0000 -> effectively a 5-cycle NOP
    bus.load(0, &[0x16, 0x00, 0x00, 0x86, 0x42]);

    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x03, "PC should be at 0x03 (after the 3-byte LBRA)");

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
}

// ============================================================
// LBSR (0x17) - Long Branch to Subroutine
// ============================================================

#[test]
fn test_lbsr_forward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // 0x00: LBSR $00FD -> branch to 0x0003 + 0x00FD = 0x0100
    // 0x03: LDA #$42   -> return target
    bus.load(0, &[0x17, 0x00, 0xFD, 0x86, 0x42]);
    bus.memory[0x0100] = 0x86; // LDA #$99 (subroutine)
    bus.memory[0x0101] = 0x99;
    bus.memory[0x0102] = 0x39; // RTS

    // LBSR: 9 cycles (1 fetch + 8 execute)
    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x0100, "PC should be at subroutine");
    assert_eq!(cpu.s, 0x7EFE, "S should have decremented by 2");
    // Return address 0x0003 should be on stack
    assert_eq!(bus.memory[0x7EFE], 0x00, "Stack high byte of return addr");
    assert_eq!(bus.memory[0x7EFF], 0x03, "Stack low byte of return addr");

    // Execute subroutine: LDA #$99 (2 cycles) + RTS (5 cycles)
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x99);
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.pc, 0x03, "PC should return to 0x0003");
    assert_eq!(cpu.s, 0x7F00, "S should be restored");

    // Execute instruction after LBSR
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_lbsr_backward() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x7F00;

    // Subroutine at 0x0010
    bus.memory[0x0010] = 0x86; // LDA #$77
    bus.memory[0x0011] = 0x77;
    bus.memory[0x0012] = 0x39; // RTS

    // Start at 0x0100: LBSR with backward offset to 0x0010
    // offset = 0x0010 - 0x0103 = -0x00F3 = 0xFF0D
    cpu.pc = 0x0100;
    bus.memory[0x0100] = 0x17; // LBSR
    bus.memory[0x0101] = 0xFF; // offset high
    bus.memory[0x0102] = 0x0D; // offset low
    bus.memory[0x0103] = 0x86; // LDA #$33 (return target)
    bus.memory[0x0104] = 0x33;

    tick(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.pc, 0x0010, "PC should branch backward to subroutine");
    // Return address should be 0x0103
    assert_eq!(bus.memory[0x7EFE], 0x01, "Stack high byte");
    assert_eq!(bus.memory[0x7EFF], 0x03, "Stack low byte");

    // LDA #$77 + RTS
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.a, 0x77);
    assert_eq!(cpu.pc, 0x0103);

    // LDA #$33 after return
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x33);
}

// ============================================================
// Undocumented opcode aliases
// ============================================================

#[test]
fn test_neg_direct_alias_0x01() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x01 is undocumented alias for NEG direct (0x00)
    bus.load(0, &[0x01, 0x10]); // NEG $10
    bus.memory[0x10] = 0x05;

    tick(&mut cpu, &mut bus, 6); // RMW direct: 6 cycles
    assert_eq!(bus.memory[0x10], 0xFB, "NEG $05 = $FB (-5)");
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0, "N should be set");
    assert_ne!(
        cpu.cc & (CcFlag::C as u8),
        0,
        "C should be set (non-zero negate)"
    );
}

#[test]
fn test_lsr_direct_alias_0x05() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x05 is undocumented alias for LSR direct (0x04)
    bus.load(0, &[0x05, 0x10]); // LSR $10
    bus.memory[0x10] = 0x04;

    tick(&mut cpu, &mut bus, 6); // RMW direct: 6 cycles
    assert_eq!(bus.memory[0x10], 0x02, "LSR $04 = $02");
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0, "C should be clear");
}

#[test]
fn test_dec_direct_alias_0x0b() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // 0x0B is undocumented alias for DEC direct (0x0A)
    bus.load(0, &[0x0B, 0x10]); // DEC $10
    bus.memory[0x10] = 0x01;

    tick(&mut cpu, &mut bus, 6); // RMW direct: 6 cycles
    assert_eq!(bus.memory[0x10], 0x00, "DEC $01 = $00");
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be set");
}

#[test]
fn test_nega_alias_0x41() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x03;
    bus.load(0, &[0x41]); // Undocumented NEGA alias

    tick(&mut cpu, &mut bus, 2); // Inherent: 2 cycles
    assert_eq!(cpu.a, 0xFD, "NEG $03 = $FD (-3)");
}

#[test]
fn test_lsra_alias_0x45() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x08;
    bus.load(0, &[0x45]); // Undocumented LSRA alias

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x04, "LSR $08 = $04");
}

#[test]
fn test_deca_alias_0x4b() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01;
    bus.load(0, &[0x4B]); // Undocumented DECA alias

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00, "DEC $01 = $00");
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be set");
}

#[test]
fn test_negb_alias_0x51() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    bus.load(0, &[0x51]); // Undocumented NEGB alias

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0xF0, "NEG $10 = $F0");
}

#[test]
fn test_lsrb_alias_0x55() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x03;
    bus.load(0, &[0x55]); // Undocumented LSRB alias

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x01, "LSR $03 = $01");
    assert_ne!(
        cpu.cc & (CcFlag::C as u8),
        0,
        "C should be set (bit 0 shifted out)"
    );
}

#[test]
fn test_decb_alias_0x5b() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x80;
    bus.load(0, &[0x5B]); // Undocumented DECB alias

    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x7F, "DEC $80 = $7F");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_ne!(
        cpu.cc & (CcFlag::V as u8),
        0,
        "V should be set (sign change)"
    );
}
