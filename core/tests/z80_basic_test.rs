use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::z80::Z80;
mod common;
use common::TestBus;

/// Helper: tick CPU until instruction boundary, return T-state count
fn run_instruction(cpu: &mut Z80, bus: &mut TestBus) -> u32 {
    let mut cycles = 0;
    loop {
        let done = cpu.tick_with_bus(bus, BusMaster::Cpu(0));
        cycles += 1;
        if done {
            return cycles;
        }
    }
}

#[test]
fn test_nop() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x00]); // NOP

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "NOP should be 4 T-states");
    assert_eq!(cpu.pc, 1);
}

#[test]
fn test_ld_a_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x3E, 0x42]); // LD A, 0x42

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7, "LD A,n should be 7 T-states");
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_ld_r_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // LD B, 0x55 then LD C, B
    bus.load(0, &[0x06, 0x55, 0x48]);

    let cycles = run_instruction(&mut cpu, &mut bus); // LD B, 0x55
    assert_eq!(cycles, 7);
    assert_eq!(cpu.b, 0x55);

    let cycles = run_instruction(&mut cpu, &mut bus); // LD C, B
    assert_eq!(cycles, 4, "LD r,r' should be 4 T-states");
    assert_eq!(cpu.c, 0x55);
}

#[test]
fn test_ld_r_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // Set HL to 0x1000, put 0xAB there, then LD A, (HL)
    bus.load(0, &[0x21, 0x00, 0x10, 0x7E]); // LD HL,0x1000; LD A,(HL)
    bus.memory[0x1000] = 0xAB;

    // For now we only have LD r,n — skip LD HL,nn (not yet implemented)
    // Manually set HL and PC
    cpu.h = 0x10;
    cpu.l = 0x00;
    cpu.pc = 3; // Point to LD A,(HL)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7, "LD A,(HL) should be 7 T-states");
    assert_eq!(cpu.a, 0xAB);
}

#[test]
fn test_ld_hl_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.a = 0xCD;
    bus.load(0, &[0x77]); // LD (HL), A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7, "LD (HL),r should be 7 T-states");
    assert_eq!(bus.memory[0x2000], 0xCD);
}

#[test]
fn test_add_a_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.b = 0x20;
    bus.load(0, &[0x80]); // ADD A, B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "ADD A,r should be 4 T-states");
    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_add_a_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    bus.load(0, &[0xC6, 0x05]); // ADD A, 0x05

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7, "ADD A,n should be 7 T-states");
    assert_eq!(cpu.a, 0x15);
}

#[test]
fn test_inc_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x0F;
    bus.load(0, &[0x04]); // INC B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "INC r should be 4 T-states");
    assert_eq!(cpu.b, 0x10);
}

#[test]
fn test_dec_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.c = 0x01;
    bus.load(0, &[0x0D]); // DEC C

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "DEC r should be 4 T-states");
    assert_eq!(cpu.c, 0x00);
}

#[test]
fn test_inc_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x30;
    cpu.l = 0x00;
    bus.memory[0x3000] = 0x7F;
    bus.load(0, &[0x34]); // INC (HL)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "INC (HL) should be 11 T-states");
    assert_eq!(bus.memory[0x3000], 0x80);
}

#[test]
fn test_halt() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x76]); // HALT

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "HALT should be 4 T-states");
    assert!(cpu.halted);
}

#[test]
fn test_dd_prefix_timing() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x0000;
    // DD 3E 42 = LD A, 0x42 (DD prefix + LD A,n — prefix is 4T, LD A,n is 7T)
    bus.load(0, &[0xDD, 0x3E, 0x42]);

    // DD prefix (4T) + LD A,n (7T) = 11T total, counted as one instruction
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "DD prefix + LD A,n should be 11 T-states");
    assert_eq!(cpu.a, 0x42);
}
