use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

#[test]
fn test_negate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01, NEGA, LDB #$80, NEGB
    bus.load(0, &[0x86, 0x01, 0x40, 0xC6, 0x80, 0x50]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$01

    // NEGA: 0 - 1 = -1 (0xFF)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // Borrow occurred
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$80 (-128)

    // NEGB: 0 - (-128) = +128 (Overflow!)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x80); // Result is still 0x80
    assert_eq!(cpu.cc & (CcFlag::V as u8), CcFlag::V as u8); // Overflow set
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_complement() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$AA, COMA, LDB #$00, COMB
    bus.load(0, &[0x86, 0xAA, 0x43, 0xC6, 0x00, 0x53]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$AA

    // COMA: ~0xAA = 0x55
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8); // C always set
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0); // V always clear
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$00

    // COMB: ~0x00 = 0xFF
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0xFF);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_clear() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$FF, CLRA, LDB #$42, CLRB
    bus.load(0, &[0x86, 0xFF, 0x4F, 0xC6, 0x42, 0x5F]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$FF

    // CLRA: A = 0
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$42

    // CLRB: B = 0
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x00);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_increment() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$7F, INCA, LDB #$FF, INCB
    bus.load(0, &[0x86, 0x7F, 0x4C, 0xC6, 0xFF, 0x5C]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$7F

    // INCA: 0x7F + 1 = 0x80 (signed overflow: positive -> negative)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x80);
    assert_eq!(
        cpu.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "Overflow should be set (0x7F -> 0x80)"
    );
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$FF

    // INCB: 0xFF + 1 = 0x00 (wraps to zero)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x00);
    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(
        cpu.cc & (CcFlag::V as u8),
        0,
        "Overflow should be clear (0xFF -> 0x00 is not signed overflow)"
    );
}

#[test]
fn test_decrement() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$80, DECA, LDB #$01, DECB
    bus.load(0, &[0x86, 0x80, 0x4A, 0xC6, 0x01, 0x5A]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$80

    // DECA: 0x80 - 1 = 0x7F (signed overflow: negative -> positive)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x7F);
    assert_eq!(
        cpu.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "Overflow should be set (0x80 -> 0x7F)"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$01

    // DECB: 0x01 - 1 = 0x00
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x00);
    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_test_register() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$80, TSTA, LDB #$00, TSTB
    bus.load(0, &[0x86, 0x80, 0x4D, 0xC6, 0x00, 0x5D]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$80

    // TSTA: test A (0x80 is negative, not zero)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.a, 0x80, "A should be unchanged");
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "Negative should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Zero should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow always clear");

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$00

    // TSTB: test B (0x00 is zero, not negative)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    assert_eq!(cpu.b, 0x00, "B should be unchanged");
    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "Negative should be clear");
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0, "Overflow always clear");
}
