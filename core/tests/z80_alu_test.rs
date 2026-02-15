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

// --- ADD HL, rr ---

#[test]
fn test_add_hl_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x20; cpu.c = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0x09]); // ADD HL, BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "ADD HL,rr should be 11 T-states");
    assert_eq!(cpu.get_hl(), 0x3000);
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
}

#[test]
fn test_add_hl_de_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x80; cpu.l = 0x00;
    cpu.d = 0x80; cpu.e = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0x19]); // ADD HL, DE

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.get_hl(), 0x0000);
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
}

#[test]
fn test_add_hl_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x40; cpu.l = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0x29]); // ADD HL, HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.get_hl(), 0x8000);
}

#[test]
fn test_add_hl_sp() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x00; cpu.l = 0x10;
    cpu.sp = 0x0020;
    cpu.f = 0x00;
    bus.load(0, &[0x39]); // ADD HL, SP

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.get_hl(), 0x0030);
}

#[test]
fn test_add_hl_half_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x0F; cpu.l = 0xFF;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0x00;
    bus.load(0, &[0x09]); // ADD HL, BC

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x1000);
    assert_ne!(cpu.f & 0x10, 0, "H should be set");
}

#[test]
fn test_add_hl_preserves_szpv() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0xC4; // S=1, Z=1, PV=1
    bus.load(0, &[0x09]); // ADD HL, BC

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0xC4, 0xC4, "S, Z, PV should be preserved");
}

#[test]
fn test_add_hl_memptr() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0x00;
    bus.load(0, &[0x09]); // ADD HL, BC

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.memptr, 0x1001, "MEMPTR should be old HL + 1");
}

// --- ADD IX, rr (DD prefix) ---

#[test]
fn test_add_ix_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1000;
    cpu.b = 0x20; cpu.c = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xDD, 0x09]); // ADD IX, BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "DD + ADD IX,rr = 4+11 = 15T");
    assert_eq!(cpu.ix, 0x3000);
}

// --- INC rr ---

#[test]
fn test_inc_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x12; cpu.c = 0x34;
    cpu.f = 0xFF;
    bus.load(0, &[0x03]); // INC BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 6, "INC rr should be 6 T-states");
    assert_eq!(cpu.get_bc(), 0x1235);
    assert_eq!(cpu.f, 0xFF, "INC rr should not affect flags");
}

#[test]
fn test_inc_de_wrap() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0xFF; cpu.e = 0xFF;
    bus.load(0, &[0x13]); // INC DE

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_de(), 0x0000, "INC DE should wrap around");
}

#[test]
fn test_inc_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x00; cpu.l = 0xFF;
    bus.load(0, &[0x23]); // INC HL

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x0100);
}

#[test]
fn test_inc_sp() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    bus.load(0, &[0x33]); // INC SP

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.sp, 0x1001);
}

// --- DEC rr ---

#[test]
fn test_dec_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x12; cpu.c = 0x34;
    cpu.f = 0xFF;
    bus.load(0, &[0x0B]); // DEC BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 6, "DEC rr should be 6 T-states");
    assert_eq!(cpu.get_bc(), 0x1233);
    assert_eq!(cpu.f, 0xFF, "DEC rr should not affect flags");
}

#[test]
fn test_dec_de_wrap() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x00; cpu.e = 0x00;
    bus.load(0, &[0x1B]); // DEC DE

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_de(), 0xFFFF, "DEC DE should wrap around");
}

#[test]
fn test_dec_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x01; cpu.l = 0x00;
    bus.load(0, &[0x2B]); // DEC HL

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x00FF);
}

#[test]
fn test_dec_sp() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    bus.load(0, &[0x3B]); // DEC SP

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.sp, 0x0FFF);
}
