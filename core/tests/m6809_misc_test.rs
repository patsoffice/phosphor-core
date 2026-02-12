use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

fn tick(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ===== NOP (0x12) =====

#[test]
fn test_nop() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.b = 0x55;
    cpu.cc = CcFlag::N as u8;
    bus.load(0, &[0x12]); // NOP

    tick(&mut cpu, &mut bus, 2); // 1 fetch + 1 execute

    assert_eq!(cpu.pc, 1);
    assert_eq!(cpu.a, 0x42); // unchanged
    assert_eq!(cpu.b, 0x55); // unchanged
    assert_eq!(cpu.cc, CcFlag::N as u8); // unchanged
}

// ===== SEX (0x1D) =====

#[test]
fn test_sex_positive() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x42; // positive (bit 7 clear)
    bus.load(0, &[0x1D]); // SEX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.a, 0x00); // sign-extended: positive → 0x00
    assert_eq!(cpu.b, 0x42); // unchanged
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0); // not negative
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not zero (D = 0x0042)
}

#[test]
fn test_sex_negative() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x80; // negative (bit 7 set)
    bus.load(0, &[0x1D]); // SEX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.a, 0xFF); // sign-extended: negative → 0xFF
    assert_eq!(cpu.b, 0x80); // unchanged
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8); // negative
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not zero (D = 0xFF80)
}

#[test]
fn test_sex_zero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    bus.load(0, &[0x1D]); // SEX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8); // D = 0x0000, zero
}

#[test]
fn test_sex_ff() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFF;
    bus.load(0, &[0x1D]); // SEX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // D = 0xFFFF, not zero
}

// ===== ABX (0x3A) =====

#[test]
fn test_abx_basic() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1000;
    cpu.b = 0x10;
    bus.load(0, &[0x3A]); // ABX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.x, 0x1010);
    assert_eq!(cpu.b, 0x10); // B unchanged
}

#[test]
fn test_abx_unsigned() {
    // B is treated as unsigned (0xFF = 255, not -1)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1000;
    cpu.b = 0xFF;
    bus.load(0, &[0x3A]); // ABX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.x, 0x10FF); // 0x1000 + 255 = 0x10FF
}

#[test]
fn test_abx_wrapping() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.x = 0xFFF0;
    cpu.b = 0x20;
    bus.load(0, &[0x3A]); // ABX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.x, 0x0010); // wraps around
}

#[test]
fn test_abx_no_flags() {
    // ABX should not affect any flags
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.x = 0x1000;
    cpu.b = 0x10;
    cpu.cc = CcFlag::Z as u8 | CcFlag::N as u8;
    bus.load(0, &[0x3A]); // ABX

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc, CcFlag::Z as u8 | CcFlag::N as u8); // flags unchanged
}

// ===== DAA (0x19) =====

#[test]
fn test_daa_basic() {
    // 0x15 + 0x27 = 0x3C → DAA → 0x42 (15 + 27 = 42 in BCD)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // ADDA #$27 then DAA
    cpu.a = 0x15;
    bus.load(0, &[0x8B, 0x27, 0x19]); // ADDA #$27, DAA

    tick(&mut cpu, &mut bus, 2); // ADDA #$27
    assert_eq!(cpu.a, 0x3C); // binary result before DAA

    tick(&mut cpu, &mut bus, 2); // DAA
    assert_eq!(cpu.a, 0x42); // corrected BCD result
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // no BCD carry
}

#[test]
fn test_daa_carry() {
    // 0x99 + 0x01 = 0x9A → DAA → 0x00 with C set (99 + 01 = 100 in BCD)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x99;
    bus.load(0, &[0x8B, 0x01, 0x19]); // ADDA #$01, DAA

    tick(&mut cpu, &mut bus, 2); // ADDA #$01
    assert_eq!(cpu.a, 0x9A);

    tick(&mut cpu, &mut bus, 2); // DAA
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // BCD carry
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8); // zero result
}

#[test]
fn test_daa_lower_nibble_correction() {
    // 0x08 + 0x04 = 0x0C → DAA → 0x12 (8 + 4 = 12 in BCD)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x08;
    bus.load(0, &[0x8B, 0x04, 0x19]); // ADDA #$04, DAA

    tick(&mut cpu, &mut bus, 2); // ADDA #$04
    assert_eq!(cpu.a, 0x0C);

    tick(&mut cpu, &mut bus, 2); // DAA
    assert_eq!(cpu.a, 0x12);
}

#[test]
fn test_daa_upper_nibble_correction() {
    // 0x91 + 0x20 = 0xB1 → DAA → 0x11 with carry (91 + 20 = 111 in BCD)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.a = 0x91;
    bus.load(0, &[0x8B, 0x20, 0x19]); // ADDA #$20, DAA

    tick(&mut cpu, &mut bus, 2); // ADDA #$20
    assert_eq!(cpu.a, 0xB1);

    tick(&mut cpu, &mut bus, 2); // DAA
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

// ===== ORCC (0x1A) =====

#[test]
fn test_orcc_set_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = 0x00;
    bus.load(0, &[0x1A, CcFlag::C as u8]); // ORCC #$01

    tick(&mut cpu, &mut bus, 2); // 1 fetch + 1 execute

    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_orcc_set_multiple() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::Z as u8;
    let mask = CcFlag::I as u8 | CcFlag::F as u8;
    bus.load(0, &[0x1A, mask]); // ORCC #$50

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc & mask, mask);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8); // preserved
}

#[test]
fn test_orcc_no_change() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::C as u8 | CcFlag::Z as u8;
    bus.load(0, &[0x1A, CcFlag::C as u8]); // ORCC #$01 (C already set)

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc, CcFlag::C as u8 | CcFlag::Z as u8); // unchanged
}

// ===== ANDCC (0x1C) =====

#[test]
fn test_andcc_clear_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::C as u8 | CcFlag::Z as u8 | CcFlag::N as u8;
    bus.load(0, &[0x1C, !(CcFlag::C as u8)]); // ANDCC #$FE (clear C)

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // C cleared
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8); // Z preserved
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8); // N preserved
}

#[test]
fn test_andcc_clear_interrupts() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = CcFlag::I as u8 | CcFlag::F as u8 | CcFlag::Z as u8;
    let mask = !(CcFlag::I as u8 | CcFlag::F as u8);
    bus.load(0, &[0x1C, mask]); // ANDCC #$AF (clear I and F)

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc & (CcFlag::I as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::F as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_andcc_clear_all() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.cc = 0xFF;
    bus.load(0, &[0x1C, 0x00]); // ANDCC #$00

    tick(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.cc, 0x00);
}

// ===== CMPU immediate (0x11, 0x83) =====

#[test]
fn test_cmpu_imm_equal() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0x1234;
    bus.load(0, &[0x11, 0x83, 0x12, 0x34]); // CMPU #$1234

    // 1 fetch + 1 prefix decode + 3 execute (high, low, compare)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.u, 0x1234); // unchanged
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_cmpu_imm_greater() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0x2000;
    bus.load(0, &[0x11, 0x83, 0x10, 0x00]); // CMPU #$1000

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not equal
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // no borrow (U > operand)
}

#[test]
fn test_cmpu_imm_less() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0x1000;
    bus.load(0, &[0x11, 0x83, 0x20, 0x00]); // CMPU #$2000

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // borrow (U < operand)
}

// ===== CMPU direct (0x11, 0x93) =====

#[test]
fn test_cmpu_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0x5678;
    cpu.dp = 0x00;
    bus.memory[0x0050] = 0x56;
    bus.memory[0x0051] = 0x78;
    bus.load(0, &[0x11, 0x93, 0x50]); // CMPU $50

    // 1 fetch + 1 prefix + 3 execute (addr, high read, low read)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8); // equal
}

// ===== CMPU extended (0x11, 0xB3) =====

#[test]
fn test_cmpu_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0x3000;
    bus.memory[0x2000] = 0x30;
    bus.memory[0x2001] = 0x00;
    bus.load(0, &[0x11, 0xB3, 0x20, 0x00]); // CMPU $2000

    // 1 fetch + 1 prefix + 4 execute (addr hi, addr lo, data hi, data lo)
    tick(&mut cpu, &mut bus, 6);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

// ===== CMPU indexed (0x11, 0xA3) =====

#[test]
fn test_cmpu_indexed() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.u = 0xAAAA;
    cpu.x = 0x3000;
    bus.memory[0x3000] = 0xAA;
    bus.memory[0x3001] = 0xAA;
    // CMPU ,X (no offset, postbyte = 0x84: bit7=1, reg=X(00), indirect=0, mode=0x04)
    bus.load(0, &[0x11, 0xA3, 0x84]); // CMPU ,X

    // 1 fetch + 1 prefix + 1 postbyte + 2 sentinel (hi+lo read)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

// ===== CMPS immediate (0x11, 0x8C) =====

#[test]
fn test_cmps_imm_equal() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0xABCD;
    bus.load(0, &[0x11, 0x8C, 0xAB, 0xCD]); // CMPS #$ABCD

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.s, 0xABCD); // unchanged
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmps_imm_less() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x0100;
    bus.load(0, &[0x11, 0x8C, 0x02, 0x00]); // CMPS #$0200

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // borrow
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_cmps_imm_greater() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x8000;
    bus.load(0, &[0x11, 0x8C, 0x40, 0x00]); // CMPS #$4000

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // no borrow
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

// ===== CMPS direct (0x11, 0x9C) =====

#[test]
fn test_cmps_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x1234;
    cpu.dp = 0x00;
    bus.memory[0x0020] = 0x12;
    bus.memory[0x0021] = 0x34;
    bus.load(0, &[0x11, 0x9C, 0x20]); // CMPS $20

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

// ===== CMPS extended (0x11, 0xBC) =====

#[test]
fn test_cmps_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0x5000;
    bus.memory[0x4000] = 0x60;
    bus.memory[0x4001] = 0x00;
    bus.load(0, &[0x11, 0xBC, 0x40, 0x00]); // CMPS $4000

    tick(&mut cpu, &mut bus, 6);

    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // S < operand
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8); // negative result
}

// ===== CMPS indexed (0x11, 0xAC) =====

#[test]
fn test_cmps_indexed() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.s = 0xBEEF;
    cpu.x = 0x5000;
    bus.memory[0x5000] = 0xBE;
    bus.memory[0x5001] = 0xEF;
    // CMPS ,X (postbyte 0x84)
    bus.load(0, &[0x11, 0xAC, 0x84]); // CMPS ,X

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}
