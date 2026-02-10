use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

#[test]
fn test_tfr_8bit() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$42, TFR A,B
    // TFR op: 1F, operand: 89 (A=8, B=9)
    bus.load(0, &[0x86, 0x42, 0x1F, 0x89]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.b, 0x00);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // TFR
    assert_eq!(cpu.b, 0x42);
    assert_eq!(cpu.a, 0x42); // Source unchanged
}

#[test]
fn test_tfr_16bit() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDX #$1234, TFR X,Y
    // TFR op: 1F, operand: 12 (X=1, Y=2)
    bus.load(0, &[0x8E, 0x12, 0x34, 0x1F, 0x12]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDX
    assert_eq!(cpu.x, 0x1234);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // TFR
    assert_eq!(cpu.y, 0x1234);
}

#[test]
fn test_exg_8bit() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$AA, LDB #$55, EXG A,B
    // EXG op: 1E, operand: 89
    bus.load(0, &[0x86, 0xAA, 0xC6, 0x55, 0x1E, 0x89]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB
    assert_eq!(cpu.a, 0xAA);
    assert_eq!(cpu.b, 0x55);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // EXG
    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.b, 0xAA);
}
