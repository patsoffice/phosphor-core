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
// RLC r
// ============================================================

#[test]
fn test_rlc_b() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x00]); // RLC B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "CB RLC B should be 8 T-states");
    assert_eq!(cpu.b, 0x0B); // 00001011 (bit 7 rotated to bit 0)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
    assert_eq!(
        cpu.f & 0x04,
        0,
        "PV should be clear (odd parity: 0x0B has 3 bits set)"
    );
}

#[test]
fn test_rlc_a_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0xFF;
    bus.load(0, &[0xCB, 0x07]); // RLC A

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_eq!(cpu.f & 0x80, 0, "S should be clear");
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (even parity for 0)");
}

// ============================================================
// RRC r
// ============================================================

#[test]
fn test_rrc_c() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.c = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x09]); // RRC C

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.c, 0xC2); // 11000010 (bit 0 rotated to bit 7)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set (bit 7 set)");
}

#[test]
fn test_rrc_no_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x42; // 01000010
    cpu.f = 0xFF;
    bus.load(0, &[0xCB, 0x0A]); // RRC D

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.d, 0x21); // 00100001
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 0 was 0)");
}

// ============================================================
// RL r
// ============================================================

#[test]
fn test_rl_e() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.e = 0x85; // 10000101
    cpu.f = 0x00; // C clear
    bus.load(0, &[0xCB, 0x13]); // RL E

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.e, 0x0A); // 00001010 (old C=0 to bit 0)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
}

#[test]
fn test_rl_with_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x42; // 01000010
    cpu.f = 0x01; // C set
    bus.load(0, &[0xCB, 0x14]); // RL H

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.h, 0x85); // 10000101 (old C=1 to bit 0)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 7 was 0)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set");
}

// ============================================================
// RR r
// ============================================================

#[test]
fn test_rr_l() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.l = 0x85; // 10000101
    cpu.f = 0x00; // C clear
    bus.load(0, &[0xCB, 0x1D]); // RR L

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.l, 0x42); // 01000010 (old C=0 to bit 7)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
}

#[test]
fn test_rr_with_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x01; // C set
    bus.load(0, &[0xCB, 0x1F]); // RR A

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0xA1); // 10100001 (old C=1 to bit 7)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 0 was 0)");
}

// ============================================================
// SLA r
// ============================================================

#[test]
fn test_sla_b() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x20]); // SLA B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.b, 0x0A); // 00001010 (bit 0 = 0)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
}

#[test]
fn test_sla_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.c = 0x80; // 10000000
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x21]); // SLA C

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.c, 0x00);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
}

// ============================================================
// SRA r
// ============================================================

#[test]
fn test_sra_sign_preserved() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x2A]); // SRA D

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.d, 0xC2); // 11000010 (sign bit preserved)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
    assert_ne!(cpu.f & 0x80, 0, "S should be set");
}

#[test]
fn test_sra_positive() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.e = 0x42; // 01000010
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x2B]); // SRA E

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.e, 0x21); // 00100001 (sign bit 0 preserved)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
}

// ============================================================
// SLL r (undocumented)
// ============================================================

#[test]
fn test_sll_undocumented() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x37]); // SLL A (undocumented)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.a, 0x85); // 10000101 (bit 0 set to 1)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 7 was 0)");
}

// ============================================================
// SRL r
// ============================================================

#[test]
fn test_srl_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x3F]); // SRL A

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8);
    assert_eq!(cpu.a, 0x42); // 01000010 (bit 7 = 0)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
    assert_eq!(cpu.f & 0x80, 0, "S should be clear");
}

// ============================================================
// BIT b,r
// ============================================================

#[test]
fn test_bit_0_b_set() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01; // bit 0 is set
    cpu.f = 0x01; // C was set
    bus.load(0, &[0xCB, 0x40]); // BIT 0, B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "BIT b,r should be 8 T-states");
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear (bit is set)");
    assert_ne!(cpu.f & 0x10, 0, "H should be set");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
}

#[test]
fn test_bit_0_b_clear() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFE; // bit 0 is clear
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x40]); // BIT 0, B

    run_instruction(&mut cpu, &mut bus);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (bit is clear)");
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (= Z)");
}

#[test]
fn test_bit_7_a_sign() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // bit 7 is set
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x7F]); // BIT 7, A

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear");
    assert_ne!(cpu.f & 0x80, 0, "S should be set (bit 7 test)");
}

#[test]
fn test_bit_7_a_clear() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x7F; // bit 7 is clear
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x7F]); // BIT 7, A

    run_instruction(&mut cpu, &mut bus);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_eq!(cpu.f & 0x80, 0, "S should be clear");
}

#[test]
fn test_bit_3_c() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.c = 0x08; // bit 3 set
    cpu.f = 0x01; // C set
    bus.load(0, &[0xCB, 0x59]); // BIT 3, C

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear (bit 3 set)");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
}

// ============================================================
// BIT b,(HL)
// ============================================================

#[test]
fn test_bit_0_hl_set() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xCB, 0x46]); // BIT 0, (HL)
    bus.memory[0x2000] = 0x01; // bit 0 is set

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12, "BIT b,(HL) should be 12 T-states");
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear");
    assert_ne!(cpu.f & 0x10, 0, "H should be set");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
}

#[test]
fn test_bit_7_hl_clear() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x7E]); // BIT 7, (HL)
    bus.memory[0x2000] = 0x7F; // bit 7 is clear

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (= Z)");
    assert_eq!(cpu.f & 0x80, 0, "S should be clear");
}

// ============================================================
// SET b,r
// ============================================================

#[test]
fn test_set_0_b() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    cpu.f = 0xFF;
    bus.load(0, &[0xCB, 0xC0]); // SET 0, B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "SET b,r should be 8 T-states");
    assert_eq!(cpu.b, 0x01);
    assert_eq!(cpu.f, 0xFF, "SET should not affect flags");
}

#[test]
fn test_set_7_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0xFF]); // SET 7, A

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.f, 0x00, "SET should not affect flags");
}

#[test]
fn test_set_already_set() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.c = 0xFF;
    bus.load(0, &[0xCB, 0xC9]); // SET 1, C

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.c, 0xFF, "SET on already-set bit should be no-op");
}

// ============================================================
// RES b,r
// ============================================================

#[test]
fn test_res_0_b() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFF;
    cpu.f = 0xFF;
    bus.load(0, &[0xCB, 0x80]); // RES 0, B

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "RES b,r should be 8 T-states");
    assert_eq!(cpu.b, 0xFE);
    assert_eq!(cpu.f, 0xFF, "RES should not affect flags");
}

#[test]
fn test_res_7_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0xBF]); // RES 7, A

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.f, 0x00, "RES should not affect flags");
}

#[test]
fn test_res_already_clear() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x00;
    bus.load(0, &[0xCB, 0x92]); // RES 2, D

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.d, 0x00, "RES on already-clear bit should be no-op");
}

// ============================================================
// SET/RES b,(HL)
// ============================================================

#[test]
fn test_set_0_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0xFF;
    bus.load(0, &[0xCB, 0xC6]); // SET 0, (HL)
    bus.memory[0x2000] = 0x00;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "SET b,(HL) should be 15 T-states");
    assert_eq!(bus.memory[0x2000], 0x01);
    assert_eq!(cpu.f, 0xFF, "SET should not affect flags");
}

#[test]
fn test_res_7_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0xBE]); // RES 7, (HL)
    bus.memory[0x2000] = 0xFF;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "RES b,(HL) should be 15 T-states");
    assert_eq!(bus.memory[0x2000], 0x7F);
}

// ============================================================
// Rotate/shift (HL)
// ============================================================

#[test]
fn test_rlc_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x06]); // RLC (HL)
    bus.memory[0x2000] = 0x85;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "RLC (HL) should be 15 T-states");
    assert_eq!(bus.memory[0x2000], 0x0B);
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
}

#[test]
fn test_srl_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x20;
    cpu.l = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0xCB, 0x3E]); // SRL (HL)
    bus.memory[0x2000] = 0x85;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15);
    assert_eq!(bus.memory[0x2000], 0x42);
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
    assert_eq!(cpu.f & 0x80, 0, "S should be clear");
}

// ============================================================
// All 8 registers for a single CB op
// ============================================================

#[test]
fn test_rlc_all_registers() {
    // RLC B=0x00, C=0x01, D=0x02, E=0x03, H=0x04, L=0x05, A=0x07
    let regs = [
        (0u8, 0x00u8),
        (1, 0x01),
        (2, 0x02),
        (3, 0x03),
        (4, 0x04),
        (5, 0x05),
        (7, 0x07),
    ];

    for &(reg_idx, opcode_low) in &regs {
        let mut cpu = Z80::new();
        let mut bus = TestBus::new();
        cpu.set_reg8(reg_idx, 0x80);
        cpu.f = 0x00;
        bus.load(0, &[0xCB, opcode_low]); // RLC reg

        run_instruction(&mut cpu, &mut bus);
        assert_eq!(
            cpu.get_reg8(reg_idx),
            0x01,
            "RLC r{} should rotate 0x80 to 0x01",
            reg_idx
        );
        assert_ne!(cpu.f & 0x01, 0, "C should be set for r{}", reg_idx);
    }
}
