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

// --- LD rr, nn ---

#[test]
fn test_ld_bc_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x01, 0x34, 0x12]); // LD BC, 0x1234

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "LD BC,nn should be 10 T-states");
    assert_eq!(cpu.b, 0x12);
    assert_eq!(cpu.c, 0x34);
    assert_eq!(cpu.pc, 3);
}

#[test]
fn test_ld_de_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x11, 0xCD, 0xAB]); // LD DE, 0xABCD

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.d, 0xAB);
    assert_eq!(cpu.e, 0xCD);
}

#[test]
fn test_ld_hl_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x21, 0x00, 0x80]); // LD HL, 0x8000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.h, 0x80);
    assert_eq!(cpu.l, 0x00);
}

#[test]
fn test_ld_sp_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x31, 0xFF, 0xFF]); // LD SP, 0xFFFF

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.sp, 0xFFFF);
}

#[test]
fn test_ld_ix_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // DD 21 34 12 = LD IX, 0x1234
    bus.load(0, &[0xDD, 0x21, 0x34, 0x12]);

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14, "DD prefix (4T) + LD IX,nn (10T) = 14T");
    assert_eq!(cpu.ix, 0x1234);
}

#[test]
fn test_ld_iy_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // FD 21 78 56 = LD IY, 0x5678
    bus.load(0, &[0xFD, 0x21, 0x78, 0x56]);

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14);
    assert_eq!(cpu.iy, 0x5678);
}

// --- LD A, (rr) / LD (rr), A ---

#[test]
fn test_ld_a_bc_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    cpu.c = 0x00;
    bus.memory[0x1000] = 0x42;
    bus.load(0, &[0x0A]); // LD A, (BC)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.memptr, 0x1001);
}

#[test]
fn test_ld_a_de_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x20;
    cpu.e = 0x00;
    bus.memory[0x2000] = 0xAB;
    bus.load(0, &[0x1A]); // LD A, (DE)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(cpu.a, 0xAB);
    assert_eq!(cpu.memptr, 0x2001);
}

#[test]
fn test_ld_bc_a_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    cpu.b = 0x30;
    cpu.c = 0x00;
    bus.load(0, &[0x02]); // LD (BC), A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(bus.memory[0x3000], 0x55);
}

#[test]
fn test_ld_de_a_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x77;
    cpu.d = 0x40;
    cpu.e = 0x00;
    bus.load(0, &[0x12]); // LD (DE), A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(bus.memory[0x4000], 0x77);
}

// --- LD A, (nn) / LD (nn), A ---

#[test]
fn test_ld_a_nn_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.memory[0x5000] = 0xEE;
    bus.load(0, &[0x3A, 0x00, 0x50]); // LD A, (0x5000)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 13, "LD A,(nn) should be 13 T-states");
    assert_eq!(cpu.a, 0xEE);
    assert_eq!(cpu.memptr, 0x5001);
}

#[test]
fn test_ld_nn_a_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0xDD;
    bus.load(0, &[0x32, 0x00, 0x60]); // LD (0x6000), A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 13);
    assert_eq!(bus.memory[0x6000], 0xDD);
}

// --- LD SP,HL ---

#[test]
fn test_ld_sp_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x50;
    cpu.l = 0x00;
    bus.load(0, &[0xF9]); // LD SP, HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 6, "LD SP,HL should be 6 T-states");
    assert_eq!(cpu.sp, 0x5000);
}

#[test]
fn test_ld_sp_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1234;
    bus.load(0, &[0xDD, 0xF9]); // LD SP, IX

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "DD + LD SP,IX = 4+6 = 10T");
    assert_eq!(cpu.sp, 0x1234);
}

// --- LD (nn), HL / LD HL, (nn) ---

#[test]
fn test_ld_nn_hl_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0xAB;
    cpu.l = 0xCD;
    bus.load(0, &[0x22, 0x00, 0x70]); // LD (0x7000), HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "LD (nn),HL should be 16 T-states");
    assert_eq!(bus.memory[0x7000], 0xCD); // low byte
    assert_eq!(bus.memory[0x7001], 0xAB); // high byte
}

#[test]
fn test_ld_hl_nn_indirect() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.memory[0x8000] = 0x34;
    bus.memory[0x8001] = 0x12;
    bus.load(0, &[0x2A, 0x00, 0x80]); // LD HL, (0x8000)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "LD HL,(nn) should be 16 T-states");
    assert_eq!(cpu.h, 0x12);
    assert_eq!(cpu.l, 0x34);
}

// --- Exchange instructions ---

#[test]
fn test_ex_af_af() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x11;
    cpu.f = 0x22;
    cpu.a_prime = 0x33;
    cpu.f_prime = 0x44;
    bus.load(0, &[0x08]); // EX AF, AF'

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.a, 0x33);
    assert_eq!(cpu.f, 0x44);
    assert_eq!(cpu.a_prime, 0x11);
    assert_eq!(cpu.f_prime, 0x22);
}

#[test]
fn test_exx() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01; cpu.c = 0x02;
    cpu.d = 0x03; cpu.e = 0x04;
    cpu.h = 0x05; cpu.l = 0x06;
    cpu.b_prime = 0x11; cpu.c_prime = 0x12;
    cpu.d_prime = 0x13; cpu.e_prime = 0x14;
    cpu.h_prime = 0x15; cpu.l_prime = 0x16;
    bus.load(0, &[0xD9]); // EXX

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.b, 0x11); assert_eq!(cpu.c, 0x12);
    assert_eq!(cpu.d, 0x13); assert_eq!(cpu.e, 0x14);
    assert_eq!(cpu.h, 0x15); assert_eq!(cpu.l, 0x16);
    assert_eq!(cpu.b_prime, 0x01); assert_eq!(cpu.c_prime, 0x02);
    assert_eq!(cpu.d_prime, 0x03); assert_eq!(cpu.e_prime, 0x04);
    assert_eq!(cpu.h_prime, 0x05); assert_eq!(cpu.l_prime, 0x06);
}

#[test]
fn test_ex_de_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x11; cpu.e = 0x22;
    cpu.h = 0x33; cpu.l = 0x44;
    bus.load(0, &[0xEB]); // EX DE, HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.d, 0x33); assert_eq!(cpu.e, 0x44);
    assert_eq!(cpu.h, 0x11); assert_eq!(cpu.l, 0x22);
}

#[test]
fn test_ex_sp_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.h = 0xAB;
    cpu.l = 0xCD;
    bus.memory[0x1000] = 0x34; // low byte on stack
    bus.memory[0x1001] = 0x12; // high byte on stack
    bus.load(0, &[0xE3]); // EX (SP), HL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19, "EX (SP),HL should be 19 T-states");
    // HL should now contain the value from the stack
    assert_eq!(cpu.h, 0x12);
    assert_eq!(cpu.l, 0x34);
    // Stack should contain the old HL value
    assert_eq!(bus.memory[0x1000], 0xCD);
    assert_eq!(bus.memory[0x1001], 0xAB);
    assert_eq!(cpu.sp, 0x1000); // SP unchanged
}

// --- LD (HL), n ---

#[test]
fn test_ld_hl_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x90;
    cpu.l = 0x00;
    bus.load(0, &[0x36, 0x42]); // LD (HL), 0x42

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "LD (HL),n should be 10 T-states");
    assert_eq!(bus.memory[0x9000], 0x42);
}
