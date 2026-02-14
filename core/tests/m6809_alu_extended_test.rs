use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

#[test]
fn test_adda_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$20
    // ADDA $1000
    bus.load(
        0,
        &[
            0x86, 0x20, // LDA #$20
            0xBB, 0x10, 0x00, // ADDA $1000
        ],
    );
    bus.memory[0x1000] = 0x30;

    // Run 7 cycles (2 for LDA, 5 for ADDA)
    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.a, 0x50);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_subb_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$50
    // SUBB $0500
    bus.load(
        0,
        &[
            0xC6, 0x50, // LDB #$50
            0xF0, 0x05, 0x00, // SUBB $0500
        ],
    );
    bus.memory[0x0500] = 0x10;

    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.b, 0x40);
}

#[test]
fn test_cmpa_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$40
    // CMPA $2000 (Value $40) -> Z=1
    bus.load(
        0,
        &[
            0x86, 0x40, // LDA #$40
            0xB1, 0x20, 0x00, // CMPA $2000
        ],
    );
    bus.memory[0x2000] = 0x40;

    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.a, 0x40); // A should not change
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_anda_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$FF
    // ANDA $3000 (Value $0F) -> A=$0F
    bus.load(
        0,
        &[
            0x86, 0xFF, // LDA #$FF
            0xB4, 0x30, 0x00, // ANDA $3000
        ],
    );
    bus.memory[0x3000] = 0x0F;

    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.a, 0x0F);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_adcb_extended_with_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$00
    // COMA (Sets Carry)
    // ADCB $4000 (Value $10) -> B = $00 + $10 + 1 = $11
    bus.load(
        0,
        &[
            0xC6, 0x00, // LDB #$00 (2 cycles)
            0x43, // COMA (2 cycles) - Sets C=1
            0xF9, 0x40, 0x00, // ADCB $4000 (5 cycles)
        ],
    );
    bus.memory[0x4000] = 0x10;

    // Total 9 cycles
    for _ in 0..9 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.b, 0x11);
}

#[test]
fn test_orb_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDB #$F0
    // ORB $5000 (Value $0F) -> B=$FF
    bus.load(
        0,
        &[
            0xC6, 0xF0, // LDB #$F0
            0xFA, 0x50, 0x00, // ORB $5000
        ],
    );
    bus.memory[0x5000] = 0x0F;

    for _ in 0..7 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    assert_eq!(cpu.b, 0xFF);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}
