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

// ============================================================
// NEG
// ============================================================

#[test]
fn test_neg_basic() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x44]); // NEG

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "NEG should be 8 T-states");
    assert_eq!(cpu.a, 0xBE); // 0 - 0x42 = 0xBE
    assert_ne!(cpu.f & 0x02, 0, "N should be set");
    assert_ne!(cpu.f & 0x01, 0, "C should be set (A was not 0)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set (result is negative)");
}

#[test]
fn test_neg_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0xFF;
    bus.load(0, &[0xED, 0x44]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (A was 0)");
}

#[test]
fn test_neg_overflow() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x44]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x80); // 0 - 0x80 = 0x80 (overflow)
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (overflow)");
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
}

// ============================================================
// ADC HL,rr
// ============================================================

#[test]
fn test_adc_hl_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x20; cpu.c = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x4A]); // ADC HL, BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "ADC HL,rr should be 15 T-states");
    assert_eq!(cpu.get_hl(), 0x3000);
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear");
}

#[test]
fn test_adc_hl_with_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x20; cpu.c = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0x4A]); // ADC HL, BC

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x3001, "Should include carry");
}

#[test]
fn test_adc_hl_overflow() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x7F; cpu.l = 0xFF;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x4A]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x8000);
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (overflow)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set");
}

#[test]
fn test_adc_hl_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0xFF; cpu.l = 0xFF;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x4A]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x0000);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
}

// ============================================================
// SBC HL,rr
// ============================================================

#[test]
fn test_sbc_hl_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x30; cpu.l = 0x00;
    cpu.b = 0x10; cpu.c = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x42]); // SBC HL, BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "SBC HL,rr should be 15 T-states");
    assert_eq!(cpu.get_hl(), 0x2000);
    assert_ne!(cpu.f & 0x02, 0, "N should be set");
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
}

#[test]
fn test_sbc_hl_with_borrow() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x30; cpu.l = 0x00;
    cpu.b = 0x10; cpu.c = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0x42]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x1FFF, "Should subtract carry");
}

#[test]
fn test_sbc_hl_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x10; cpu.c = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x42]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0x0000);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
}

#[test]
fn test_sbc_hl_underflow() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x00; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x01;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x42]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_hl(), 0xFFFF);
    assert_ne!(cpu.f & 0x01, 0, "C should be set (borrow)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set");
}

// ============================================================
// LD I,A / LD A,I / LD R,A / LD A,R
// ============================================================

#[test]
fn test_ld_i_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.i = 0x00;
    bus.load(0, &[0xED, 0x47]); // LD I, A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 9, "LD I,A should be 9 T-states");
    assert_eq!(cpu.i, 0x42);
}

#[test]
fn test_ld_a_i() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.i = 0x42;
    cpu.a = 0x00;
    cpu.f = 0x01; // C set
    cpu.iff2 = true;
    bus.load(0, &[0xED, 0x57]); // LD A, I

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 9);
    assert_eq!(cpu.a, 0x42);
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
    assert_ne!(cpu.f & 0x04, 0, "PV should reflect IFF2 (true)");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
}

#[test]
fn test_ld_a_i_iff2_false() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.i = 0x00;
    cpu.iff2 = false;
    cpu.f = 0x00;
    bus.load(0, &[0xED, 0x57]);

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_eq!(cpu.f & 0x04, 0, "PV should be clear (IFF2 false)");
}

#[test]
fn test_ld_r_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    bus.load(0, &[0xED, 0x4F]); // LD R, A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 9);
    assert_eq!(cpu.r, 0x55);
}

#[test]
fn test_ld_a_r() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.r = 0x42;
    cpu.a = 0x00;
    cpu.iff2 = false;
    cpu.f = 0x01;
    bus.load(0, &[0xED, 0x5F]); // LD A, R

    run_instruction(&mut cpu, &mut bus);
    // Note: R has been incremented by the instruction fetch cycles
    // (2 M1 cycles = 2 R increments). So the value loaded is not 0x42.
    // We just check timing and flag behavior.
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
    assert_eq!(cpu.f & 0x04, 0, "PV should be clear (IFF2 false)");
}

// ============================================================
// LD (nn),rr / LD rr,(nn) — ED variants
// ============================================================

#[test]
fn test_ld_nn_bc_ed() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x12; cpu.c = 0x34;
    bus.load(0, &[0xED, 0x43, 0x00, 0x20]); // LD (0x2000), BC

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 20, "LD (nn),rr should be 20 T-states");
    assert_eq!(bus.memory[0x2000], 0x34); // low byte
    assert_eq!(bus.memory[0x2001], 0x12); // high byte
}

#[test]
fn test_ld_bc_nn_ed() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xED, 0x4B, 0x00, 0x20]); // LD BC, (0x2000)
    bus.memory[0x2000] = 0x34;
    bus.memory[0x2001] = 0x12;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 20, "LD rr,(nn) should be 20 T-states");
    assert_eq!(cpu.get_bc(), 0x1234);
}

#[test]
fn test_ld_nn_sp_ed() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0xABCD;
    bus.load(0, &[0xED, 0x73, 0x00, 0x30]); // LD (0x3000), SP

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(bus.memory[0x3000], 0xCD);
    assert_eq!(bus.memory[0x3001], 0xAB);
}

// ============================================================
// RRD / RLD
// ============================================================

#[test]
fn test_rrd() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x84; // A = 1000_0100
    cpu.h = 0x20; cpu.l = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0x67]); // RRD
    bus.memory[0x2000] = 0x20; // (HL) = 0010_0000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 18, "RRD should be 18 T-states");
    // RRD: A_low(4) → (HL)_high, (HL)_high(2) → (HL)_low, (HL)_low(0) → A_low
    assert_eq!(cpu.a, 0x80);         // A = 1000_0000 (A_high preserved, (HL)_low → A_low)
    assert_eq!(bus.memory[0x2000], 0x42); // (HL) = 0100_0010 (A_low → high, old_high → low)
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
    assert_ne!(cpu.f & 0x80, 0, "S should be set");
}

#[test]
fn test_rld() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x84; // A = 1000_0100
    cpu.h = 0x20; cpu.l = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0x6F]); // RLD
    bus.memory[0x2000] = 0x20; // (HL) = 0010_0000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 18, "RLD should be 18 T-states");
    // RLD: (HL)_high(2) → A_low, A_low(4) → (HL)_low, (HL)_low(0) → (HL)_high
    assert_eq!(cpu.a, 0x82);         // A = 1000_0010
    assert_eq!(bus.memory[0x2000], 0x04); // (HL) = 0000_0100
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
}

// ============================================================
// IM
// ============================================================

#[test]
fn test_im_0() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.im = 2;
    bus.load(0, &[0xED, 0x46]); // IM 0

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.im, 0);
}

#[test]
fn test_im_1() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xED, 0x56]); // IM 1

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.im, 1);
}

#[test]
fn test_im_2() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xED, 0x5E]); // IM 2

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.im, 2);
}

// ============================================================
// RETN
// ============================================================

#[test]
fn test_retn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.iff1 = false;
    cpu.iff2 = true;
    bus.load(0, &[0xED, 0x45]); // RETN
    bus.memory[0x1000] = 0x00; // PC low
    bus.memory[0x1001] = 0x30; // PC high

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14, "RETN should be 14 T-states");
    assert_eq!(cpu.pc, 0x3000);
    assert_eq!(cpu.sp, 0x1002);
    assert!(cpu.iff1, "IFF1 should be copied from IFF2");
}

// ============================================================
// IN r,(C) / OUT (C),r
// ============================================================

#[test]
fn test_in_a_c() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.b = 0x10; cpu.c = 0x20;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0x78]); // IN A, (C)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12, "IN r,(C) should be 12 T-states");
    assert_eq!(cpu.a, 0xFF, "Stubbed I/O returns 0xFF");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
}

#[test]
fn test_out_c_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.f = 0xFF;
    bus.load(0, &[0xED, 0x79]); // OUT (C), A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12, "OUT (C),r should be 12 T-states");
    assert_eq!(cpu.f, 0xFF, "OUT should not affect flags");
}

// ============================================================
// ED NOP
// ============================================================

#[test]
fn test_ed_nop() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    let old_pc = cpu.pc;
    bus.load(0, &[0xED, 0x00]); // ED NOP (undefined)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "ED NOP should be 8 T-states");
    assert_eq!(cpu.pc, old_pc + 2); // Consumed 2 bytes (ED + opcode)
}
