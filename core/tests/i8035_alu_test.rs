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
// ADD A,Rn (0x68-0x6F) — 1 cycle
// =============================================================================

#[test]
fn test_add_a_r0() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x15;
    cpu.ram[0] = 0x30; // R0 = 0x30
    bus.load(0, &[0x68]); // ADD A,R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x45);
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0);
    assert_eq!(cpu.psw & (PswFlag::AC as u8), 0);
}

#[test]
fn test_add_a_rn_carry() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF0;
    cpu.ram[3] = 0x20; // R3 = 0x20
    bus.load(0, &[0x6B]); // ADD A,R3
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x10);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0); // carry set
    assert_eq!(cpu.psw & (PswFlag::AC as u8), 0);
}

#[test]
fn test_add_a_rn_aux_carry() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    cpu.ram[1] = 0x01; // R1 = 0x01
    bus.load(0, &[0x69]); // ADD A,R1
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x10);
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0);
    assert_ne!(cpu.psw & (PswFlag::AC as u8), 0); // aux carry set
}

// =============================================================================
// ADD A,@Ri (0x60-0x61) — 1 cycle
// =============================================================================

#[test]
fn test_add_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.ram[0] = 0x20; // R0 = 0x20 (pointer)
    cpu.ram[0x20] = 0x05; // RAM[0x20] = 0x05
    bus.load(0, &[0x60]); // ADD A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x15);
}

// =============================================================================
// ADD A,#data (0x03) — 2 cycles
// =============================================================================

#[test]
fn test_add_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x25;
    bus.load(0, &[0x03, 0x1A]); // ADD A,#0x1A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x3F);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_add_a_imm_overflow() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x03, 0x01]); // ADD A,#0x01
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0);
    assert_ne!(cpu.psw & (PswFlag::AC as u8), 0);
}

// =============================================================================
// ADDC A,Rn (0x78-0x7F) — 1 cycle
// =============================================================================

#[test]
fn test_addc_a_rn_no_carry_in() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.ram[2] = 0x05; // R2 = 0x05
    bus.load(0, &[0x7A]); // ADDC A,R2
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x15);
}

#[test]
fn test_addc_a_rn_with_carry_in() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.psw = PswFlag::CY as u8; // CY set
    cpu.ram[2] = 0x05; // R2 = 0x05
    bus.load(0, &[0x7A]); // ADDC A,R2
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x16); // 0x10 + 0x05 + 1
}

#[test]
fn test_addc_a_rn_carry_out() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.psw = PswFlag::CY as u8;
    cpu.ram[0] = 0x00; // R0 = 0x00
    bus.load(0, &[0x78]); // ADDC A,R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0);
}

// =============================================================================
// ADDC A,@Ri (0x70-0x71) — 1 cycle
// =============================================================================

#[test]
fn test_addc_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.psw = PswFlag::CY as u8;
    cpu.ram[1] = 0x30; // R1 = 0x30 (pointer)
    cpu.ram[0x30] = 0x0F;
    bus.load(0, &[0x71]); // ADDC A,@R1
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x20); // 0x10 + 0x0F + 1
    assert_ne!(cpu.psw & (PswFlag::AC as u8), 0);
}

// =============================================================================
// ADDC A,#data (0x13) — 2 cycles
// =============================================================================

#[test]
fn test_addc_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0x13, 0x30]); // ADDC A,#0x30
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x81); // 0x50 + 0x30 + 1
}

// =============================================================================
// ANL A,Rn (0x58-0x5F) — 1 cycle
// =============================================================================

#[test]
fn test_anl_a_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF5;
    cpu.ram[0] = 0x0F; // R0 = 0x0F
    bus.load(0, &[0x58]); // ANL A,R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x05);
}

// =============================================================================
// ANL A,@Ri (0x50-0x51) — 1 cycle
// =============================================================================

#[test]
fn test_anl_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAB;
    cpu.ram[0] = 0x20; // R0 = 0x20 (pointer)
    cpu.ram[0x20] = 0xF0;
    bus.load(0, &[0x50]); // ANL A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xA0);
}

// =============================================================================
// ANL A,#data (0x53) — 2 cycles
// =============================================================================

#[test]
fn test_anl_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x53, 0x3C]); // ANL A,#0x3C
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x3C);
}

// =============================================================================
// ORL A,Rn (0x48-0x4F) — 1 cycle
// =============================================================================

#[test]
fn test_orl_a_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xA0;
    cpu.ram[5] = 0x05; // R5 = 0x05
    bus.load(0, &[0x4D]); // ORL A,R5
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xA5);
}

// =============================================================================
// ORL A,@Ri (0x40-0x41) — 1 cycle
// =============================================================================

#[test]
fn test_orl_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.ram[1] = 0x30; // R1 = 0x30
    cpu.ram[0x30] = 0x42;
    bus.load(0, &[0x41]); // ORL A,@R1
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x42);
}

// =============================================================================
// ORL A,#data (0x43) — 2 cycles
// =============================================================================

#[test]
fn test_orl_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    bus.load(0, &[0x43, 0xF0]); // ORL A,#0xF0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
}

// =============================================================================
// XRL A,Rn (0xD8-0xDF) — 1 cycle
// =============================================================================

#[test]
fn test_xrl_a_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.ram[7] = 0x0F; // R7 = 0x0F
    bus.load(0, &[0xDF]); // XRL A,R7
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xF0);
}

// =============================================================================
// XRL A,@Ri (0xD0-0xD1) — 1 cycle
// =============================================================================

#[test]
fn test_xrl_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    cpu.ram[0] = 0x20;
    cpu.ram[0x20] = 0x55;
    bus.load(0, &[0xD0]); // XRL A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xFF);
}

// =============================================================================
// XRL A,#data (0xD3) — 2 cycles
// =============================================================================

#[test]
fn test_xrl_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    bus.load(0, &[0xD3, 0xAA]); // XRL A,#0xAA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
}

// =============================================================================
// INC A (0x17) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_inc_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x41;
    bus.load(0, &[0x17]); // INC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_inc_a_wrap() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x17]); // INC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00);
    // INC does not affect flags
}

// =============================================================================
// INC Rn (0x18-0x1F) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_inc_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[3] = 0x10; // R3 = 0x10
    bus.load(0, &[0x1B]); // INC R3
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ram[3], 0x11);
}

// =============================================================================
// INC @Ri (0x10-0x11) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_inc_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x25; // R0 = 0x25 (pointer)
    cpu.ram[0x25] = 0x7F;
    bus.load(0, &[0x10]); // INC @R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ram[0x25], 0x80);
}

// =============================================================================
// DEC A (0x07) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_dec_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    bus.load(0, &[0x07]); // DEC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x0F);
}

#[test]
fn test_dec_a_wrap() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x07]); // DEC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xFF);
}

// =============================================================================
// DEC Rn (0xC8-0xCF) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_dec_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[4] = 0x01; // R4 = 0x01
    bus.load(0, &[0xCC]); // DEC R4
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ram[4], 0x00);
}

// =============================================================================
// DA A (0x57) — 1 cycle, CY affected
// =============================================================================

#[test]
fn test_da_basic() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // BCD add: 0x15 + 0x27 = 0x3C, then DA adjusts to 0x42
    cpu.a = 0x15;
    cpu.ram[0] = 0x27;
    bus.load(0, &[0x68, 0x57]); // ADD A,R0; DA A
    tick(&mut cpu, &mut bus, 1); // ADD
    assert_eq!(cpu.a, 0x3C);
    tick(&mut cpu, &mut bus, 1); // DA
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0);
}

#[test]
fn test_da_bcd_carry() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // BCD: 0x99 + 0x01 = 0x9A, DA adjusts to 0x00 with CY
    cpu.a = 0x99;
    cpu.ram[0] = 0x01;
    bus.load(0, &[0x68, 0x57]); // ADD A,R0; DA A
    tick(&mut cpu, &mut bus, 1); // ADD
    tick(&mut cpu, &mut bus, 1); // DA
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0);
}

// =============================================================================
// CLR A (0x27) — 1 cycle
// =============================================================================

#[test]
fn test_clr_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAB;
    bus.load(0, &[0x27]); // CLR A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00);
}

// =============================================================================
// CPL A (0x37) — 1 cycle
// =============================================================================

#[test]
fn test_cpl_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    bus.load(0, &[0x37]); // CPL A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xAA);
}

#[test]
fn test_cpl_a_zero() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x37]); // CPL A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00);
}

// =============================================================================
// RL A (0xE7) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_rl_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x81; // 1000_0001
    bus.load(0, &[0xE7]); // RL A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x03); // 0000_0011 (bit 7 wraps to bit 0)
}

// =============================================================================
// RLC A (0xF7) — 1 cycle, CY affected
// =============================================================================

#[test]
fn test_rlc_a_carry_out() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // 1000_0000
    bus.load(0, &[0xF7]); // RLC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00); // bit7 went to CY, old CY (0) to bit 0
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0);
}

#[test]
fn test_rlc_a_carry_in() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0xF7]); // RLC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x01); // old CY (1) to bit 0
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0); // bit7 was 0
}

// =============================================================================
// RR A (0x77) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_rr_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x03; // 0000_0011
    bus.load(0, &[0x77]); // RR A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x81); // 1000_0001 (bit 0 wraps to bit 7)
}

// =============================================================================
// RRC A (0x67) — 1 cycle, CY affected
// =============================================================================

#[test]
fn test_rrc_a_carry_out() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01; // 0000_0001
    bus.load(0, &[0x67]); // RRC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00); // bit0 went to CY
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0);
}

#[test]
fn test_rrc_a_carry_in() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0x67]); // RRC A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x80); // old CY (1) to bit 7
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0);
}

// =============================================================================
// SWAP A (0x47) — 1 cycle, no flags
// =============================================================================

#[test]
fn test_swap_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xA5; // 1010_0101
    bus.load(0, &[0x47]); // SWAP A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x5A); // 0101_1010
}

#[test]
fn test_swap_a_zero() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    bus.load(0, &[0x47]); // SWAP A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x00);
}

// =============================================================================
// NOP (0x00) — 1 cycle
// =============================================================================

#[test]
fn test_nop() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x00]); // NOP
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc, 1);
}

// =============================================================================
// Status flag ops — 1 cycle
// =============================================================================

#[test]
fn test_clr_c() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::CY as u8;
    bus.load(0, &[0x97]); // CLR C
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.psw & (PswFlag::CY as u8), 0);
}

#[test]
fn test_cpl_c() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = 0; // CY clear
    bus.load(0, &[0xA7]); // CPL C
    tick(&mut cpu, &mut bus, 1);
    assert_ne!(cpu.psw & (PswFlag::CY as u8), 0); // CY now set
}

#[test]
fn test_clr_f0() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = PswFlag::F0 as u8;
    bus.load(0, &[0x85]); // CLR F0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.psw & (PswFlag::F0 as u8), 0);
}

#[test]
fn test_cpl_f0() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = 0;
    bus.load(0, &[0x95]); // CPL F0
    tick(&mut cpu, &mut bus, 1);
    assert_ne!(cpu.psw & (PswFlag::F0 as u8), 0);
}

#[test]
fn test_clr_f1() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.f1 = true;
    bus.load(0, &[0xA5]); // CLR F1
    tick(&mut cpu, &mut bus, 1);
    assert!(!cpu.f1);
}

#[test]
fn test_cpl_f1() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.f1 = false;
    bus.load(0, &[0xB5]); // CPL F1
    tick(&mut cpu, &mut bus, 1);
    assert!(cpu.f1);
}

// =============================================================================
// Register bank select — 1 cycle
// =============================================================================

#[test]
fn test_sel_rb1_and_add() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.ram[0] = 0x01;  // Bank 0: R0 = 0x01
    cpu.ram[0x18] = 0xFF; // Bank 1: R0 = 0xFF
    bus.load(0, &[0xD5, 0x68]); // SEL RB1; ADD A,R0
    tick(&mut cpu, &mut bus, 1); // SEL RB1
    assert_ne!(cpu.psw & (PswFlag::BS as u8), 0);
    tick(&mut cpu, &mut bus, 1); // ADD A,R0 (from bank 1)
    assert_eq!(cpu.a, 0x0F); // 0x10 + 0xFF = 0x0F (with carry)
}
