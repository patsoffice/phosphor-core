use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::z80::Z80;
mod common;
use common::TestBus;

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

// --- PUSH ---

#[test]
fn test_push_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.b = 0x12;
    cpu.c = 0x34;
    bus.load(0, &[0xC5]); // PUSH BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "PUSH should be 11 T-states");
    assert_eq!(cpu.sp, 0x0FFE);
    assert_eq!(bus.memory[0x0FFF], 0x12); // high byte (B)
    assert_eq!(bus.memory[0x0FFE], 0x34); // low byte (C)
}

#[test]
fn test_push_de() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x2000;
    cpu.d = 0xAB;
    cpu.e = 0xCD;
    bus.load(0, &[0xD5]); // PUSH DE

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.sp, 0x1FFE);
    assert_eq!(bus.memory[0x1FFF], 0xAB);
    assert_eq!(bus.memory[0x1FFE], 0xCD);
}

#[test]
fn test_push_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x3000;
    cpu.h = 0x56;
    cpu.l = 0x78;
    bus.load(0, &[0xE5]); // PUSH HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.sp, 0x2FFE);
    assert_eq!(bus.memory[0x2FFF], 0x56);
    assert_eq!(bus.memory[0x2FFE], 0x78);
}

#[test]
fn test_push_af() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x4000;
    cpu.a = 0x11;
    cpu.f = 0x22;
    bus.load(0, &[0xF5]); // PUSH AF

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.sp, 0x3FFE);
    assert_eq!(bus.memory[0x3FFF], 0x11); // A
    assert_eq!(bus.memory[0x3FFE], 0x22); // F
}

#[test]
fn test_push_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x5000;
    cpu.ix = 0xBEEF;
    bus.load(0, &[0xDD, 0xE5]); // PUSH IX

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "DD + PUSH IX = 4+11 = 15T");
    assert_eq!(cpu.sp, 0x4FFE);
    assert_eq!(bus.memory[0x4FFF], 0xBE);
    assert_eq!(bus.memory[0x4FFE], 0xEF);
}

// --- POP ---

#[test]
fn test_pop_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    bus.memory[0x1000] = 0x34; // low byte (C)
    bus.memory[0x1001] = 0x12; // high byte (B)
    bus.load(0, &[0xC1]); // POP BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "POP should be 10 T-states");
    assert_eq!(cpu.b, 0x12);
    assert_eq!(cpu.c, 0x34);
    assert_eq!(cpu.sp, 0x1002);
}

#[test]
fn test_pop_de() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x2000;
    bus.memory[0x2000] = 0xCD;
    bus.memory[0x2001] = 0xAB;
    bus.load(0, &[0xD1]); // POP DE

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.d, 0xAB);
    assert_eq!(cpu.e, 0xCD);
    assert_eq!(cpu.sp, 0x2002);
}

#[test]
fn test_pop_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x3000;
    bus.memory[0x3000] = 0x78;
    bus.memory[0x3001] = 0x56;
    bus.load(0, &[0xE1]); // POP HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.h, 0x56);
    assert_eq!(cpu.l, 0x78);
    assert_eq!(cpu.sp, 0x3002);
}

#[test]
fn test_pop_af() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x4000;
    bus.memory[0x4000] = 0x22; // F
    bus.memory[0x4001] = 0x11; // A
    bus.load(0, &[0xF1]); // POP AF

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.f, 0x22);
    assert_eq!(cpu.sp, 0x4002);
}

#[test]
fn test_pop_iy() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x5000;
    bus.memory[0x5000] = 0xEF;
    bus.memory[0x5001] = 0xBE;
    bus.load(0, &[0xFD, 0xE1]); // POP IY

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14, "FD + POP IY = 4+10 = 14T");
    assert_eq!(cpu.iy, 0xBEEF);
    assert_eq!(cpu.sp, 0x5002);
}

// --- PUSH/POP round-trip ---

#[test]
fn test_push_pop_roundtrip() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.h = 0xAB;
    cpu.l = 0xCD;
    // PUSH HL, POP DE
    bus.load(0, &[0xE5, 0xD1]);

    run_instruction(&mut cpu, &mut bus); // PUSH HL
    assert_eq!(cpu.sp, 0x0FFE);

    run_instruction(&mut cpu, &mut bus); // POP DE
    assert_eq!(cpu.sp, 0x1000);
    assert_eq!(cpu.d, 0xAB);
    assert_eq!(cpu.e, 0xCD);
}

// --- SP wrapping ---

#[test]
fn test_push_sp_wrap() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x0001; // Near bottom of memory
    cpu.b = 0x12;
    cpu.c = 0x34;
    bus.load(0, &[0xC5]); // PUSH BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.sp, 0xFFFF); // Wraps around
    assert_eq!(bus.memory[0x0000], 0x12); // High byte at 0x0000
    assert_eq!(bus.memory[0xFFFF], 0x34); // Low byte at 0xFFFF
}
