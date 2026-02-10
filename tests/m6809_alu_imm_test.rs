use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

#[test]
fn test_cmpa_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$10, CMPA #$10, CMPA #$20
    bus.load(0, &[0x86, 0x10, 0x81, 0x10, 0x81, 0x20]);

    // LDA #$10
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    // CMPA #$10 (10 - 10 = 0) -> Z=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x10);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);

    // CMPA #$20 (10 - 20 = -16 = F0) -> N=1, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x10);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_sbca_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$00, SUBA #$01 (sets C), SBCA #$01
    bus.load(0, &[0x86, 0x00, 0x80, 0x01, 0x82, 0x01]);

    // LDA #$00
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // SUBA #$01 -> A=FF, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);

    // SBCA #$01 -> A = FF - 01 - 1 = FD
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0xFD);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_logical_ops() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$CC, ANDA #$F0, ORA #$03, EORA #$FF
    bus.load(0, &[0x86, 0xCC, 0x84, 0xF0, 0x8A, 0x03, 0x88, 0xFF]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ANDA #$F0 -> C0
    assert_eq!(cpu.a, 0xC0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8); // C0 is neg
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ORA #$03 -> C3
    assert_eq!(cpu.a, 0xC3);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // EORA #$FF -> 3C
    assert_eq!(cpu.a, 0x3C);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_bita_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$FF, BITA #$00, BITA #$80
    bus.load(0, &[0x86, 0xFF, 0x85, 0x00, 0x85, 0x80]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BITA #$00 -> Z=1
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BITA #$80 -> N=1
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_adca_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$FF, ADDA #$01 (sets C), ADCA #$00
    bus.load(0, &[0x86, 0xFF, 0x8B, 0x01, 0x89, 0x00]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADDA -> 00, C=1

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADCA #$00 -> 00 + 00 + 1 = 01
    assert_eq!(cpu.a, 0x01);
}

#[test]
fn test_b_register_alu() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$10, ADDB #$10, SUBB #$05
    bus.load(0, &[0xC6, 0x10, 0xCB, 0x10, 0xC0, 0x05]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB
    assert_eq!(cpu.b, 0x10);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADDB
    assert_eq!(cpu.b, 0x20);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // SUBB
    assert_eq!(cpu.b, 0x1B);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpb_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$10, CMPB #$10, CMPB #$20
    bus.load(0, &[0xC6, 0x10, 0xC1, 0x10, 0xC1, 0x20]);

    // LDB #$10
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    // CMPB #$10 (10 - 10 = 0) -> Z=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x10);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);

    // CMPB #$20 (10 - 20 = -16 = F0) -> N=1, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x10);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_sbcb_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$00, SUBB #$01 (sets C), SBCB #$01
    bus.load(0, &[0xC6, 0x00, 0xC0, 0x01, 0xC2, 0x01]);

    // LDB #$00
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // SUBB #$01 -> B=FF, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);

    // SBCB #$01 -> B = FF - 01 - 1 = FD
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0xFD);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_logical_ops_b() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$CC, ANDB #$F0, ORB #$03, EORB #$FF
    bus.load(0, &[0xC6, 0xCC, 0xC4, 0xF0, 0xCA, 0x03, 0xC8, 0xFF]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ANDB #$F0 -> C0
    assert_eq!(cpu.b, 0xC0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8); // C0 is neg
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ORB #$03 -> C3
    assert_eq!(cpu.b, 0xC3);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // EORB #$FF -> 3C
    assert_eq!(cpu.b, 0x3C);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_bitb_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$FF, BITB #$00, BITB #$80
    bus.load(0, &[0xC6, 0xFF, 0xC5, 0x00, 0xC5, 0x80]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BITB #$00 -> Z=1
    assert_eq!(cpu.b, 0xFF);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // BITB #$80 -> N=1
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_adcb_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$FF, ADDB #$01 (sets C), ADCB #$00
    bus.load(0, &[0xC6, 0xFF, 0xCB, 0x01, 0xC9, 0x00]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADDB -> 00, C=1

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADCB #$00 -> 00 + 00 + 1 = 01
    assert_eq!(cpu.b, 0x01);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}
