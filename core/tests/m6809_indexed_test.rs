mod common;

use common::TestBus;
use phosphor_core::core::BusMaster;
use phosphor_core::core::component::BusMasterComponent;
use phosphor_core::cpu::m6809::{CcFlag, M6809};

fn tick(cpu: &mut M6809, bus: &mut TestBus) {
    cpu.tick_with_bus(bus, BusMaster::Cpu(0));
}

fn run_cycles(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        tick(cpu, bus);
    }
}

// ============================================================
// 5-bit constant offset tests
// ============================================================

#[test]
fn test_lda_indexed_5bit_zero_offset() {
    // LDA ,X (5-bit offset = 0, register = X)
    // Postbyte: 0b0_00_00000 = 0x00 (reg=X, offset=0)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x42;

    // Load program: 0xA6 (LDA indexed), 0x00 (,X with 0 offset)
    bus.load(0x0000, &[0xA6, 0x00]);

    // 1 fetch + 1 postbyte resolve (cycle 0) + 1 read operand (cycle 50) = 3 total
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not zero
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0); // not negative
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0); // V always cleared
}

#[test]
fn test_lda_indexed_5bit_positive_offset() {
    // LDA 5,X
    // Postbyte: 0b0_00_00101 = 0x05 (reg=X, offset=+5)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2005] = 0x80; // negative value

    bus.load(0x0000, &[0xA6, 0x05]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x80);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
}

#[test]
fn test_lda_indexed_5bit_negative_offset() {
    // LDA -3,X
    // Postbyte: 0b0_00_11101 = 0x1D (reg=X, offset=-3 in 5-bit two's complement)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2003;
    bus.memory[0x2000] = 0x55;

    bus.load(0x0000, &[0xA6, 0x1D]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x55);
}

#[test]
fn test_lda_indexed_5bit_y_register() {
    // LDA 2,Y
    // Postbyte: 0b0_01_00010 = 0x22 (reg=Y, offset=+2)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.y = 0x3000;
    bus.memory[0x3002] = 0xAA;

    bus.load(0x0000, &[0xA6, 0x22]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xAA);
}

#[test]
fn test_lda_indexed_5bit_u_register() {
    // LDA 1,U
    // Postbyte: 0b0_10_00001 = 0x41 (reg=U, offset=+1)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.u = 0x4000;
    bus.memory[0x4001] = 0x33;

    bus.load(0x0000, &[0xA6, 0x41]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x33);
}

#[test]
fn test_lda_indexed_5bit_s_register() {
    // LDA 0,S
    // Postbyte: 0b0_11_00000 = 0x60 (reg=S, offset=0)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x5000;
    bus.memory[0x5000] = 0x77;

    bus.load(0x0000, &[0xA6, 0x60]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x77);
}

// ============================================================
// Post-increment / Pre-decrement tests
// ============================================================

#[test]
fn test_lda_indexed_post_increment_1() {
    // LDA ,X+ (post-increment by 1)
    // Postbyte: 0b1_00_0_0000 = 0x80
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x42;

    bus.load(0x0000, &[0xA6, 0x80]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.x, 0x2001); // X incremented by 1
}

#[test]
fn test_lda_indexed_post_increment_2() {
    // LDA ,X++ (post-increment by 2)
    // Postbyte: 0b1_00_0_0001 = 0x81
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x55;

    bus.load(0x0000, &[0xA6, 0x81]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.x, 0x2002); // X incremented by 2
}

#[test]
fn test_lda_indexed_pre_decrement_1() {
    // LDA ,-X (pre-decrement by 1)
    // Postbyte: 0b1_00_0_0010 = 0x82
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2001;
    bus.memory[0x2000] = 0x99;

    bus.load(0x0000, &[0xA6, 0x82]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x99);
    assert_eq!(cpu.x, 0x2000); // X decremented by 1
}

#[test]
fn test_lda_indexed_pre_decrement_2() {
    // LDA ,--X (pre-decrement by 2)
    // Postbyte: 0b1_00_0_0011 = 0x83
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2002;
    bus.memory[0x2000] = 0xBB;

    bus.load(0x0000, &[0xA6, 0x83]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xBB);
    assert_eq!(cpu.x, 0x2000); // X decremented by 2
}

// ============================================================
// Accumulator offset tests
// ============================================================

#[test]
fn test_lda_indexed_b_offset() {
    // LDA B,X
    // Postbyte: 0b1_00_0_0101 = 0x85
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.b = 0x05;
    bus.memory[0x2005] = 0xCC;

    bus.load(0x0000, &[0xA6, 0x85]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xCC);
}

#[test]
fn test_lda_indexed_a_offset() {
    // LDA A,X
    // Postbyte: 0b1_00_0_0110 = 0x86
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x0A;
    bus.memory[0x200A] = 0xDD;

    bus.load(0x0000, &[0xA6, 0x86]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xDD);
}

#[test]
fn test_lda_indexed_d_offset() {
    // LDA D,X
    // Postbyte: 0b1_00_0_1011 = 0x8B
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x1000;
    cpu.a = 0x00;
    cpu.b = 0x10;
    bus.memory[0x1010] = 0xEE;

    bus.load(0x0000, &[0xA6, 0x8B]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xEE);
}

#[test]
fn test_lda_indexed_b_offset_negative() {
    // LDA B,X with B = 0xFE (-2 signed)
    // Postbyte: 0b1_00_0_0101 = 0x85
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2002;
    cpu.b = 0xFE; // -2 signed
    bus.memory[0x2000] = 0x44;

    bus.load(0x0000, &[0xA6, 0x85]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x44);
}

// ============================================================
// 8-bit constant offset tests
// ============================================================

#[test]
fn test_lda_indexed_8bit_offset() {
    // LDA $10,X (8-bit offset = 0x10)
    // Postbyte: 0b1_00_0_1000 = 0x88 (n8,R mode, reg=X)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2010] = 0x66;

    // Opcode + postbyte + offset byte
    bus.load(0x0000, &[0xA6, 0x88, 0x10]);

    // 1 fetch + 1 read postbyte + 1 read offset + 1 read operand = 4
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x66);
}

#[test]
fn test_lda_indexed_8bit_negative_offset() {
    // LDA -5,X (8-bit offset = 0xFB = -5)
    // Postbyte: 0x88
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2005;
    bus.memory[0x2000] = 0x77;

    bus.load(0x0000, &[0xA6, 0x88, 0xFB]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x77);
}

// ============================================================
// 16-bit constant offset tests
// ============================================================

#[test]
fn test_lda_indexed_16bit_offset() {
    // LDA $1234,X (16-bit offset)
    // Postbyte: 0b1_00_0_1001 = 0x89 (n16,R mode, reg=X)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x1000;
    bus.memory[0x2234] = 0x88;

    // Opcode + postbyte + offset high + offset low
    bus.load(0x0000, &[0xA6, 0x89, 0x12, 0x34]);

    // 1 fetch + 1 postbyte + 1 offset hi + 1 offset lo + 1 read operand = 5
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.a, 0x88);
}

// ============================================================
// No offset (,R) mode
// ============================================================

#[test]
fn test_lda_indexed_no_offset() {
    // LDA ,X (no offset, extended mode)
    // Postbyte: 0b1_00_0_0100 = 0x84
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x3000;
    bus.memory[0x3000] = 0x11;

    bus.load(0x0000, &[0xA6, 0x84]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x11);
}

// ============================================================
// PC-relative tests
// ============================================================

#[test]
fn test_lda_indexed_8bit_pc_relative() {
    // LDA n8,PCR
    // Postbyte: 0b1_00_0_1100 = 0x8C
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // After reading opcode (at 0x0000) and postbyte (at 0x0001) and offset (at 0x0002),
    // PC = 0x0003. Offset = 0x05, so EA = 0x0003 + 5 = 0x0008
    bus.memory[0x0008] = 0x99;
    bus.load(0x0000, &[0xA6, 0x8C, 0x05]);

    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x99);
}

#[test]
fn test_lda_indexed_16bit_pc_relative() {
    // LDA n16,PCR
    // Postbyte: 0b1_00_0_1101 = 0x8D
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // After reading opcode, postbyte, hi, lo: PC = 0x0004
    // Offset = 0x0100, so EA = 0x0004 + 0x0100 = 0x0104
    bus.memory[0x0104] = 0xAB;
    bus.load(0x0000, &[0xA6, 0x8D, 0x01, 0x00]);

    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.a, 0xAB);
}

// ============================================================
// Store indexed tests
// ============================================================

#[test]
fn test_sta_indexed_5bit_offset() {
    // STA 3,X
    // Postbyte: 0b0_00_00011 = 0x03
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x42;

    bus.load(0x0000, &[0xA7, 0x03]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(bus.memory[0x2003], 0x42);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0); // V always cleared
}

#[test]
fn test_stb_indexed_post_inc() {
    // STB ,Y++
    // Postbyte: 0b1_01_0_0001 = 0xA1
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.y = 0x3000;
    cpu.b = 0xFF;

    bus.load(0x0000, &[0xE7, 0xA1]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(bus.memory[0x3000], 0xFF);
    assert_eq!(cpu.y, 0x3002);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
}

// ============================================================
// 16-bit load/store indexed tests
// ============================================================

#[test]
fn test_ldd_indexed() {
    // LDD ,X (5-bit offset=0)
    // Postbyte: 0x00
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x12;
    bus.memory[0x2001] = 0x34;

    bus.load(0x0000, &[0xEC, 0x00]);

    // 1 fetch + 1 postbyte + 1 read hi (cycle 50) + 1 read lo (cycle 51) = 4
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.b, 0x34);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_std_indexed() {
    // STD ,Y (5-bit offset=0, reg=Y)
    // Postbyte: 0b0_01_00000 = 0x20
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.y = 0x3000;
    cpu.a = 0xAB;
    cpu.b = 0xCD;

    bus.load(0x0000, &[0xED, 0x20]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x3000], 0xAB);
    assert_eq!(bus.memory[0x3001], 0xCD);
}

#[test]
fn test_ldx_indexed() {
    // LDX ,U (5-bit offset=0, reg=U)
    // Postbyte: 0b0_10_00000 = 0x40
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.u = 0x4000;
    bus.memory[0x4000] = 0x56;
    bus.memory[0x4001] = 0x78;

    bus.load(0x0000, &[0xAE, 0x40]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.x, 0x5678);
}

#[test]
fn test_stx_indexed() {
    // STX 1,S (5-bit offset=1, reg=S)
    // Postbyte: 0b0_11_00001 = 0x61
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x5000;
    cpu.x = 0xBEEF;

    bus.load(0x0000, &[0xAF, 0x61]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x5001], 0xBE);
    assert_eq!(bus.memory[0x5002], 0xEF);
}

#[test]
fn test_ldu_indexed() {
    // LDU ,X (5-bit offset=0)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0xCA;
    bus.memory[0x2001] = 0xFE;

    bus.load(0x0000, &[0xEE, 0x00]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.u, 0xCAFE);
}

#[test]
fn test_stu_indexed() {
    // STU ,X++
    // Postbyte: 0b1_00_0_0001 = 0x81
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.u = 0xDEAD;

    bus.load(0x0000, &[0xEF, 0x81]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0xDE);
    assert_eq!(bus.memory[0x2001], 0xAD);
    assert_eq!(cpu.x, 0x2002);
}

// ============================================================
// LEA instruction tests
// ============================================================

#[test]
fn test_leax_5bit_offset() {
    // LEAX 5,X
    // Postbyte: 0b0_00_00101 = 0x05
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;

    bus.load(0x0000, &[0x30, 0x05]);

    // 1 fetch + 1 postbyte = 2 cycles
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.x, 0x2005);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not zero
}

#[test]
fn test_leax_zero_result() {
    // LEAX offset,X where result is 0
    // X=5, offset=-5 (5-bit: 0x1B)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x0005;

    bus.load(0x0000, &[0x30, 0x1B]); // -5 in 5-bit = 0b11011
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.x, 0x0000);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // Z set
}

#[test]
fn test_leay_post_increment() {
    // LEAY ,X++ (loads EA = old X into Y, then X += 2)
    // Postbyte: 0b1_00_0_0001 = 0x81
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x3000;

    bus.load(0x0000, &[0x31, 0x81]);
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.y, 0x3000); // Y gets old X value
    assert_eq!(cpu.x, 0x3002); // X incremented by 2
}

#[test]
fn test_leas_offset() {
    // LEAS 4,S (S = S + 4, deallocate stack frame)
    // Postbyte: 0b0_11_00100 = 0x64
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x7FF0;

    bus.load(0x0000, &[0x32, 0x64]);
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.s, 0x7FF4);
}

#[test]
fn test_leau_negative_offset() {
    // LEAU -2,U (U = U - 2)
    // Postbyte: 0b0_10_11110 = 0x5E  (-2 in 5-bit two's complement)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.u = 0x4000;

    bus.load(0x0000, &[0x33, 0x5E]);
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.u, 0x3FFE);
}

// ============================================================
// ALU indexed tests (representative operations)
// ============================================================

#[test]
fn test_adda_indexed() {
    // ADDA ,X (5-bit offset=0)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x10;
    bus.memory[0x2000] = 0x20;

    bus.load(0x0000, &[0xAB, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_suba_indexed_carry() {
    // SUBA ,X (5-bit offset=0)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x10;
    bus.memory[0x2000] = 0x20; // 0x10 - 0x20 = borrow

    bus.load(0x0000, &[0xA0, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0xF0);
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // carry/borrow set
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
}

#[test]
fn test_cmpa_indexed() {
    // CMPA ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x50;
    bus.memory[0x2000] = 0x50;

    bus.load(0x0000, &[0xA1, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // equal
    assert_eq!(cpu.a, 0x50); // A unchanged
}

#[test]
fn test_anda_indexed() {
    // ANDA ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0xF0;
    bus.memory[0x2000] = 0x0F;

    bus.load(0x0000, &[0xA4, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // zero
}

#[test]
fn test_subd_indexed() {
    // SUBD ,X (16-bit)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x10;
    cpu.b = 0x00;
    bus.memory[0x2000] = 0x01;
    bus.memory[0x2001] = 0x00;

    // 0xA3 SUBD indexed, postbyte 0x00 (,X)
    bus.load(0x0000, &[0xA3, 0x00]);

    // 1 fetch + 1 postbyte + 1 read hi (50) + 1 read lo (51) = 4
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x0F); // D = 0x1000 - 0x0100 = 0x0F00
    assert_eq!(cpu.b, 0x00);
}

#[test]
fn test_addd_indexed() {
    // ADDD ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x10;
    cpu.b = 0x00;
    bus.memory[0x2000] = 0x02;
    bus.memory[0x2001] = 0x34;

    bus.load(0x0000, &[0xE3, 0x00]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.b, 0x34);
}

#[test]
fn test_cmpx_indexed() {
    // CMPX ,Y
    // Postbyte: 0b0_01_00000 = 0x20
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.y = 0x2000;
    cpu.x = 0x1234;
    bus.memory[0x2000] = 0x12;
    bus.memory[0x2001] = 0x34;

    bus.load(0x0000, &[0xAC, 0x20]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // equal
}

// ============================================================
// Memory-modify indexed tests (unary/shift)
// ============================================================

#[test]
fn test_neg_indexed() {
    // NEG ,X (0x60)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x01;

    // Postbyte 0x84 = ,X (no offset, extended mode)
    bus.load(0x0000, &[0x60, 0x84]);

    // 1 fetch + 1 postbyte + 1 read val (50) + 1 write result (51) = 4
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0xFF); // NEG of 0x01 = 0xFF
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // borrow from 0
}

#[test]
fn test_inc_indexed() {
    // INC ,X (0x6C)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x7F;

    bus.load(0x0000, &[0x6C, 0x84]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0x80);
    assert_ne!(cpu.cc & (CcFlag::V as u8), 0); // overflow 0x7F->0x80
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative
}

#[test]
fn test_clr_indexed() {
    // CLR ,X (0x6F)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0xFF;

    bus.load(0x0000, &[0x6F, 0x84]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // zero
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0); // not negative
}

#[test]
fn test_asl_indexed() {
    // ASL ,X (0x68)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x81; // 10000001

    bus.load(0x0000, &[0x68, 0x84]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0x02); // shifted left
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // old bit 7 was 1
}

#[test]
fn test_lsr_indexed() {
    // LSR ,X (0x64)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x03; // 00000011

    bus.load(0x0000, &[0x64, 0x84]);
    run_cycles(&mut cpu, &mut bus, 4);

    assert_eq!(bus.memory[0x2000], 0x01); // shifted right
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // old bit 0 was 1
}

#[test]
fn test_tst_indexed() {
    // TST ,X (0x6D) - read-only, no write-back
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x00;

    bus.load(0x0000, &[0x6D, 0x84]);
    // TST uses rmw_indexed: 1 fetch + 1 postbyte + 1 read + 1 write-back = 4
    run_cycles(&mut cpu, &mut bus, 4);

    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // zero
    assert_eq!(bus.memory[0x2000], 0x00); // unchanged
}

// ============================================================
// JMP / JSR indexed tests
// ============================================================

#[test]
fn test_jmp_indexed() {
    // JMP ,X (0x6E)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x4000;

    // Postbyte 0x84 = ,X (no offset)
    bus.load(0x0000, &[0x6E, 0x84]);
    run_cycles(&mut cpu, &mut bus, 2);

    assert_eq!(cpu.pc, 0x4000);
}

#[test]
fn test_jmp_indexed_8bit_offset() {
    // JMP $10,X (8-bit offset)
    // Postbyte: 0x88, offset: 0x10
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.load(0x0000, &[0x6E, 0x88, 0x10]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.pc, 0x2010);
}

#[test]
fn test_jsr_indexed() {
    // JSR ,X (0xAD)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x4000;
    cpu.s = 0x8000;

    // Postbyte 0x84 = ,X (no offset)
    bus.load(0x0000, &[0xAD, 0x84]);

    // 1 fetch + 1 postbyte + 1 internal (50) + 1 push lo (51) + 1 push hi (52) + 1 jump (53) = 6
    run_cycles(&mut cpu, &mut bus, 6);

    assert_eq!(cpu.pc, 0x4000);
    // Return address (0x0002 = after opcode+postbyte) pushed onto stack
    assert_eq!(cpu.s, 0x7FFE);
    assert_eq!(bus.memory[0x7FFE], 0x00); // PC high
    assert_eq!(bus.memory[0x7FFF], 0x02); // PC low
}

// ============================================================
// Indirect mode tests
// ============================================================

#[test]
fn test_lda_indexed_indirect_no_offset() {
    // LDA [,X] (indirect, no offset)
    // Postbyte: 0b1_00_1_0100 = 0x94 (indirect bit set, mode=0x04)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    // Pointer at 0x2000 points to 0x3000
    bus.memory[0x2000] = 0x30;
    bus.memory[0x2001] = 0x00;
    // Actual data at 0x3000
    bus.memory[0x3000] = 0x42;

    bus.load(0x0000, &[0xA6, 0x94]);

    // 1 fetch + 1 postbyte(→10) + 1 indirect hi(10→11) + 1 indirect lo(11→50) + 1 read(50) = 5
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_lda_indexed_indirect_post_inc_2() {
    // LDA [,X++] (indirect post-increment by 2)
    // Postbyte: 0b1_00_1_0001 = 0x91
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    // Pointer at old X (0x2000) points to 0x5000
    bus.memory[0x2000] = 0x50;
    bus.memory[0x2001] = 0x00;
    bus.memory[0x5000] = 0xBB;

    bus.load(0x0000, &[0xA6, 0x91]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.a, 0xBB);
    assert_eq!(cpu.x, 0x2002);
}

#[test]
fn test_lda_indexed_extended_indirect() {
    // LDA [$1234] (extended indirect)
    // Postbyte: 0b1_00_1_1111 = 0x9F
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Pointer at 0x1234 points to 0x5678
    bus.memory[0x1234] = 0x56;
    bus.memory[0x1235] = 0x78;
    bus.memory[0x5678] = 0xCC;

    // Opcode + postbyte + addr high + addr low
    bus.load(0x0000, &[0xA6, 0x9F, 0x12, 0x34]);

    // 1 fetch + 1 postbyte(→1) + 1 addr hi(1→2) + 1 addr lo(2→10) + 1 ind hi(10→11) + 1 ind lo(11→50) + 1 read(50) = 7
    run_cycles(&mut cpu, &mut bus, 7);

    assert_eq!(cpu.a, 0xCC);
}

// ============================================================
// Page 2 indexed tests (CMPD, CMPY, LDY, STY, LDS, STS)
// ============================================================

#[test]
fn test_cmpd_indexed() {
    // CMPD ,X (0x10, 0xA3)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.a = 0x12;
    cpu.b = 0x34;
    bus.memory[0x2000] = 0x12;
    bus.memory[0x2001] = 0x34;

    // Page 2 prefix + opcode + postbyte (,X no offset = 0x00)
    bus.load(0x0000, &[0x10, 0xA3, 0x00]);

    // 1 fetch(0x10) + 1 prefix decode(→page2) + 1 postbyte + 1 read hi(50) + 1 read lo(51) = 5
    run_cycles(&mut cpu, &mut bus, 5);

    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0); // equal
}

#[test]
fn test_cmpy_indexed() {
    // CMPY ,X (0x10, 0xAC)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.y = 0x5000;
    bus.memory[0x2000] = 0x40;
    bus.memory[0x2001] = 0x00;

    bus.load(0x0000, &[0x10, 0xAC, 0x00]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0); // not equal (0x5000 > 0x4000)
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0); // no borrow
}

#[test]
fn test_ldy_indexed() {
    // LDY ,X (0x10, 0xAE)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0xAB;
    bus.memory[0x2001] = 0xCD;

    bus.load(0x0000, &[0x10, 0xAE, 0x00]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.y, 0xABCD);
}

#[test]
fn test_sty_indexed() {
    // STY ,X (0x10, 0xAF)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.y = 0xFACE;

    bus.load(0x0000, &[0x10, 0xAF, 0x00]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(bus.memory[0x2000], 0xFA);
    assert_eq!(bus.memory[0x2001], 0xCE);
}

#[test]
fn test_lds_indexed() {
    // LDS ,X (0x10, 0xEE)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    bus.memory[0x2000] = 0x80;
    bus.memory[0x2001] = 0x00;

    bus.load(0x0000, &[0x10, 0xEE, 0x00]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.s, 0x8000);
}

#[test]
fn test_sts_indexed() {
    // STS ,X (0x10, 0xEF)
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.s = 0x7FFF;

    bus.load(0x0000, &[0x10, 0xEF, 0x00]);
    run_cycles(&mut cpu, &mut bus, 5);

    assert_eq!(bus.memory[0x2000], 0x7F);
    assert_eq!(bus.memory[0x2001], 0xFF);
}

// ============================================================
// B register indexed ALU tests
// ============================================================

#[test]
fn test_subb_indexed() {
    // SUBB ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.b = 0x30;
    bus.memory[0x2000] = 0x10;

    bus.load(0x0000, &[0xE0, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.b, 0x20);
}

#[test]
fn test_addb_indexed() {
    // ADDB ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.b = 0x10;
    bus.memory[0x2000] = 0x05;

    bus.load(0x0000, &[0xEB, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.b, 0x15);
}

#[test]
fn test_orb_indexed() {
    // ORB ,X
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0x2000;
    cpu.b = 0xF0;
    bus.memory[0x2000] = 0x0F;

    bus.load(0x0000, &[0xEA, 0x00]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.b, 0xFF);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

// ============================================================
// Address wrapping edge case
// ============================================================

#[test]
fn test_indexed_address_wrapping() {
    // LDA ,X with X near end of address space should wrap
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0xFFFF;
    bus.memory[0xFFFF] = 0x42;

    bus.load(0x0000, &[0xA6, 0x00]); // 5-bit offset=0
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_post_increment_wrapping() {
    // LDA ,X+ at end of address space
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.x = 0xFFFF;
    bus.memory[0xFFFF] = 0x11;

    bus.load(0x0000, &[0xA6, 0x80]);
    run_cycles(&mut cpu, &mut bus, 3);

    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.x, 0x0000); // wrapped
}
