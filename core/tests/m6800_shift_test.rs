use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` cycles
fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// ASL (Arithmetic Shift Left) - 0x48 ASLA, 0x58 ASLB
// =============================================================================

#[test]
fn test_asla_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x15; // 0001_0101
    bus.load(0, &[0x48]); // ASLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x2A); // 0010_1010
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 7 was 0
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::Z as u8, 0);
}

#[test]
fn test_asla_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // 1000_0000
    bus.load(0, &[0x48]); // ASLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8); // bit 7 was 1
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_asla_overflow() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x40; // 0100_0000 → 0x80 (N=1, C=0, V = 1 XOR 0 = 1)
    bus.load(0, &[0x48]); // ASLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8); // V = N XOR C = 1 XOR 0
}

#[test]
fn test_asla_ff() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    bus.load(0, &[0x48]); // ASLA → 0xFE, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFE);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    // V = N XOR C = 1 XOR 1 = 0
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

#[test]
fn test_aslb_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01;
    bus.load(0, &[0x58]); // ASLB
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x02);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

#[test]
fn test_aslb_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xC0; // 1100_0000
    bus.load(0, &[0x58]); // ASLB → 0x80, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x80);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    // V = N XOR C = 1 XOR 1 = 0
    assert_eq!(cpu.cc & CcFlag::V as u8, 0);
}

// =============================================================================
// ASR (Arithmetic Shift Right) - 0x47 ASRA, 0x57 ASRB
// =============================================================================

#[test]
fn test_asra_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x04; // 0000_0100
    bus.load(0, &[0x47]); // ASRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x02); // 0000_0010
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 0 was 0
}

#[test]
fn test_asra_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x03; // 0000_0011
    bus.load(0, &[0x47]); // ASRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x01); // 0000_0001
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8); // bit 0 was 1
}

#[test]
fn test_asra_sign_extension() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // 1000_0000 → sign extended: 1100_0000
    bus.load(0, &[0x47]); // ASRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xC0);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8); // still negative
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

#[test]
fn test_asra_negative_odd() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF; // 1111_1111 → 1111_1111, C=1
    bus.load(0, &[0x47]); // ASRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_asrb_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x40; // 0100_0000 → 0010_0000
    bus.load(0, &[0x57]); // ASRB
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x20);
}

// =============================================================================
// LSR (Logical Shift Right) - 0x44 LSRA, 0x54 LSRB
// =============================================================================

#[test]
fn test_lsra_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x04; // 0000_0100 → 0000_0010
    bus.load(0, &[0x44]); // LSRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x02);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0); // N always clear
}

#[test]
fn test_lsra_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01;
    bus.load(0, &[0x44]); // LSRA → 0x00, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
    // V = N XOR C = 0 XOR 1 = 1
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8);
}

#[test]
fn test_lsra_high_bit_cleared() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // 1000_0000 → 0100_0000
    bus.load(0, &[0x44]); // LSRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x40);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0); // N always clear for LSR
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

#[test]
fn test_lsrb_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xFE; // 1111_1110 → 0111_1111
    bus.load(0, &[0x54]); // LSRB
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x7F);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
}

#[test]
fn test_lsrb_carry_and_zero() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01;
    bus.load(0, &[0x54]); // LSRB → 0x00, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

// =============================================================================
// ROL (Rotate Left through Carry) - 0x49 ROLA, 0x59 ROLB
// =============================================================================

#[test]
fn test_rola_no_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55; // 0101_0101, C=0
    bus.load(0, &[0x49]); // ROLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAA); // 1010_1010 (bit 0 gets old C=0)
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 7 was 0
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_rola_with_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55; // 0101_0101, C=1
    cpu.cc = CcFlag::C as u8;
    bus.load(0, &[0x49]); // ROLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAB); // 1010_1011 (bit 0 gets old C=1)
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 7 was 0
}

#[test]
fn test_rola_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    bus.load(0, &[0x49]); // ROLA → 0x00, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_rolb_basic() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01;
    cpu.cc = CcFlag::C as u8; // carry in
    bus.load(0, &[0x59]); // ROLB
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x03); // 0000_0010 | 1 = 0000_0011
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
}

// =============================================================================
// ROR (Rotate Right through Carry) - 0x46 RORA, 0x56 RORB
// =============================================================================

#[test]
fn test_rora_no_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA; // 1010_1010, C=0
    bus.load(0, &[0x46]); // RORA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x55); // 0101_0101 (bit 7 gets old C=0)
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 0 was 0
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
}

#[test]
fn test_rora_with_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA; // 1010_1010, C=1
    cpu.cc = CcFlag::C as u8;
    bus.load(0, &[0x46]); // RORA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xD5); // 1101_0101 (bit 7 gets old C=1)
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 0 was 0
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

#[test]
fn test_rora_carry_out() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x01; // 0000_0001, C=0
    bus.load(0, &[0x46]); // RORA → 0x00, C=1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::Z as u8, CcFlag::Z as u8);
}

#[test]
fn test_rorb_with_carry_in() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    cpu.cc = CcFlag::C as u8; // carry in
    bus.load(0, &[0x56]); // RORB → 0x80 (C goes into bit 7)
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.b, 0x80);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0); // bit 0 was 0
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
}

// =============================================================================
// V flag (N XOR C) verification
// =============================================================================

#[test]
fn test_asl_v_flag_n1_c1() {
    // N=1, C=1 → V = 1 XOR 1 = 0
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xC0; // 1100_0000 → 1000_0000, C=1, N=1
    bus.load(0, &[0x48]); // ASLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & CcFlag::N as u8, CcFlag::N as u8);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V = 1 XOR 1 = 0
}

#[test]
fn test_asl_v_flag_n0_c1() {
    // N=0, C=1 → V = 0 XOR 1 = 1
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80; // 1000_0000 → 0000_0000, C=1, N=0
    bus.load(0, &[0x48]); // ASLA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & CcFlag::N as u8, 0);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8); // V = 0 XOR 1 = 1
}

#[test]
fn test_lsr_v_equals_c() {
    // LSR: N always 0, so V = 0 XOR C = C
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x03; // bit 0 = 1 → C=1, V=1
    bus.load(0, &[0x44]); // LSRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.cc & CcFlag::C as u8, CcFlag::C as u8);
    assert_eq!(cpu.cc & CcFlag::V as u8, CcFlag::V as u8); // V = C = 1
}

#[test]
fn test_lsr_v_clear_when_c_clear() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x02; // bit 0 = 0 → C=0, V=0
    bus.load(0, &[0x44]); // LSRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.cc & CcFlag::C as u8, 0);
    assert_eq!(cpu.cc & CcFlag::V as u8, 0); // V = C = 0
}

// =============================================================================
// Multi-instruction sequences
// =============================================================================

#[test]
fn test_asl_twice_multiply_by_4() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x05; // 5 * 4 = 20
    bus.load(0, &[0x48, 0x48]); // ASLA; ASLA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x14); // 20
}

#[test]
fn test_lsr_divide_by_2() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x14; // 20 / 2 = 10
    bus.load(0, &[0x44]); // LSRA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x0A); // 10
}

#[test]
fn test_rol_ror_roundtrip() {
    // ROL then ROR should restore the original value (if starting with same carry)
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.cc = 0; // C=0
    bus.load(0, &[0x49, 0x46]); // ROLA; RORA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_asl_asr_roundtrip_positive() {
    // ASL then ASR on a positive number should restore it
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x15; // positive, no info lost in bit 7
    bus.load(0, &[0x48, 0x47]); // ASLA; ASRA
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x15);
}

#[test]
fn test_8_rotates_restores_original() {
    // 8 ROL operations with no external carry change should cycle back
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.cc = 0; // C=0 — this is the 9th bit in the rotate
    // 9 ROLs would restore if we treat carry as the 9th bit
    // 8 ROLs is not a full cycle, but we can verify behavior
    bus.load(0, &[0x49, 0x49, 0x49, 0x49, 0x49, 0x49, 0x49, 0x49, 0x49]);
    tick(&mut cpu, &mut bus, 18); // 9 ROLs × 2 cycles
    assert_eq!(cpu.a, 0x42);
}
