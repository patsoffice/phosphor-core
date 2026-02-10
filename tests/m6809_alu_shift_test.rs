use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

#[test]
fn test_asl() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$55, ASLA, LDB #$80, ASLB
    bus.load(0, &[0x86, 0x55, 0x48, 0xC6, 0x80, 0x58]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$55

    // ASLA: 0x55 (0101_0101) << 1 = 0xAA (1010_1010), C=0
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0xAA);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        0,
        "C should be clear (old bit 7 was 0)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
    // V = N XOR C = 1 XOR 0 = 1
    assert_eq!(
        state.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "V should be set (N XOR C)"
    );

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$80

    // ASLB: 0x80 (1000_0000) << 1 = 0x00, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.b, 0x00);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 7 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
    // V = N XOR C = 0 XOR 1 = 1
    assert_eq!(
        state.cc & (CcFlag::V as u8),
        CcFlag::V as u8,
        "V should be set (N XOR C)"
    );
}

#[test]
fn test_asr() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$81, ASRA, LDB #$40, ASRB
    bus.load(0, &[0x86, 0x81, 0x47, 0xC6, 0x40, 0x57]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$81

    // ASRA: 0x81 (1000_0001) >> 1 = 0xC0 (1100_0000), sign preserved, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0xC0);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 0 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set (sign preserved)"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$40

    // ASRB: 0x40 (0100_0000) >> 1 = 0x20 (0010_0000), C=0
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.b, 0x20);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        0,
        "C should be clear (old bit 0 was 0)"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_lsr() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01, LSRA, LDB #$80, LSRB
    bus.load(0, &[0x86, 0x01, 0x44, 0xC6, 0x80, 0x54]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$01

    // LSRA: 0x01 >> 1 = 0x00, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0x00);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 0 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N always clear for LSR");

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$80

    // LSRB: 0x80 >> 1 = 0x40, C=0
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.b, 0x40);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        0,
        "C should be clear (old bit 0 was 0)"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N always clear for LSR");
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_rol() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$80, ROLA (C starts clear from reset)
    bus.load(0, &[0x86, 0x80, 0x49]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$80

    // ROLA: 0x80 rotated left, old C=0 enters bit 0
    // Result: 0x00, C=1 (old bit 7)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0x00);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 7 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(state.cc & (CcFlag::N as u8), 0, "N should be clear");
}

#[test]
fn test_rol_with_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01, ASLA (to set carry since 0x01<<1=0x02, C=0... need different approach)
    // LDA #$80, ASLA (sets C=1), LDA #$55, ROLA
    bus.load(0, &[0x86, 0x80, 0x48, 0x86, 0x55, 0x49]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$80
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ASLA: 0x80 << 1 = 0x00, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$55
    // Now C=1 (still set from ASLA, LDA doesn't affect C)

    // ROLA: 0x55 (0101_0101) rotated left with C=1
    // Result: 0xAB (1010_1011), C=0 (old bit 7 of 0x55 was 0)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0xAB);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        0,
        "C should be clear (old bit 7 was 0)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
}

#[test]
fn test_ror() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01, RORA (C starts clear from reset)
    bus.load(0, &[0x86, 0x01, 0x46]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$01

    // RORA: 0x01 rotated right, old C=0 enters bit 7
    // Result: 0x00, C=1 (old bit 0)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.a, 0x00);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 0 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        0,
        "N should be clear (old C was 0)"
    );
}

#[test]
fn test_ror_with_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$01, LSRA (sets C=1, A=0x00), LDB #$40, RORB
    bus.load(0, &[0x86, 0x01, 0x44, 0xC6, 0x40, 0x56]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$01
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LSRA: 0x01 >> 1 = 0x00, C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDB #$40

    // RORB: 0x40 (0100_0000) rotated right with C=1
    // Result: 0xA0 (1010_0000), C=0 (old bit 0 of 0x40 was 0)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(state.b, 0xA0);
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        0,
        "C should be clear (old bit 0 was 0)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set (old C entered bit 7)"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_asr_sign_extension() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$FF, ASRA — shifting -1 right should stay -1
    bus.load(0, &[0x86, 0xFF, 0x47]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA #$FF

    // ASRA: 0xFF (1111_1111) >> 1 = 0xFF (sign extended), C=1
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    let state = &cpu;
    assert_eq!(
        state.a, 0xFF,
        "ASR of 0xFF should remain 0xFF (sign extension)"
    );
    assert_eq!(
        state.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (old bit 0 was 1)"
    );
    assert_eq!(
        state.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(state.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}
