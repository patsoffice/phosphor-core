use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

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
