use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

fn tick(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

#[test]
fn test_addd_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1000, ADDD #$0123
    bus.load(0, &[0xCC, 0x10, 0x00, 0xC3, 0x01, 0x23]);

    // LDD (3 cycles)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.a, 0x10);
    assert_eq!(cpu.b, 0x00);

    // ADDD (4 cycles: 1 fetch + 3 exec)
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.a, 0x11, "A should be high byte of 0x1123");
    assert_eq!(cpu.b, 0x23, "B should be low byte of 0x1123");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_subd_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1000, SUBD #$0001
    bus.load(0, &[0xCC, 0x10, 0x00, 0x83, 0x00, 0x01]);

    // LDD (3 cycles)
    tick(&mut cpu, &mut bus, 3);

    // SUBD (4 cycles: 1 fetch + 3 exec)
    tick(&mut cpu, &mut bus, 4);
    // 0x1000 - 0x0001 = 0x0FFF
    assert_eq!(cpu.a, 0x0F, "A should be high byte of 0x0FFF");
    assert_eq!(cpu.b, 0xFF, "B should be low byte of 0x0FFF");
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDX #$1000, CMPX #$1000, CMPX #$2000
    bus.load(0, &[0x8E, 0x10, 0x00, 0x8C, 0x10, 0x00, 0x8C, 0x20, 0x00]);

    // LDX (3 cycles)
    tick(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.x, 0x1000);

    // CMPX #$1000 (4 cycles) -> Z=1
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    // CMPX #$2000 (4 cycles) -> N=1, C=1
    tick(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_addd_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1000, ADDD $2000
    bus.load(0, &[0xCC, 0x10, 0x00, 0xF3, 0x20, 0x00]);
    bus.memory[0x2000] = 0x01;
    bus.memory[0x2001] = 0x23;

    // LDD (3 cycles) + ADDD (7 cycles) = 10 cycles
    tick(&mut cpu, &mut bus, 10);

    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.b, 0x23);
}

#[test]
fn test_subd_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1000, SUBD $3000
    bus.load(0, &[0xCC, 0x10, 0x00, 0xB3, 0x30, 0x00]);
    bus.memory[0x3000] = 0x00;
    bus.memory[0x3001] = 0x01;

    // LDD (3 cycles) + SUBD (7 cycles) = 10 cycles
    tick(&mut cpu, &mut bus, 10);

    assert_eq!(cpu.a, 0x0F);
    assert_eq!(cpu.b, 0xFF);
}

#[test]
fn test_cmpx_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDX #$5000, CMPX $4000
    bus.load(0, &[0x8E, 0x50, 0x00, 0xBC, 0x40, 0x00]);
    bus.memory[0x4000] = 0x50;
    bus.memory[0x4001] = 0x00;

    // LDX (3 cycles) + CMPX (7 cycles) = 10 cycles
    tick(&mut cpu, &mut bus, 10);

    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_cmpy_immediate_equal() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.y = 0x1234;
    // CMPY #$1234 (0x10 0x8C 0x12 0x34)
    bus.load(0, &[0x10, 0x8C, 0x12, 0x34]);

    // CMPY immediate: 5 cycles (2 prefix + 3 execute)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(cpu.y, 0x1234, "Y should be unchanged");
    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0, "N should be clear");
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0, "C should be clear");
}

#[test]
fn test_cmpy_immediate_less() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.y = 0x1000;
    // CMPY #$2000 -> 0x1000 - 0x2000 = -0x1000 (N=1, C=1)
    bus.load(0, &[0x10, 0x8C, 0x20, 0x00]);

    tick(&mut cpu, &mut bus, 5);

    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        CcFlag::N as u8,
        "N should be set"
    );
    assert_eq!(
        cpu.cc & (CcFlag::C as u8),
        CcFlag::C as u8,
        "C should be set (borrow)"
    );
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0, "Z should be clear");
}

#[test]
fn test_cmpy_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.y = 0x5000;
    // CMPY $20 (0x10 0x9C 0x20)
    bus.load(0, &[0x10, 0x9C, 0x20]);
    bus.memory[0x0020] = 0x50;
    bus.memory[0x0021] = 0x00;

    // CMPY direct: 7 cycles (2 prefix + 5 execute)
    tick(&mut cpu, &mut bus, 7);

    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
}

#[test]
fn test_cmpy_extended() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.y = 0x5000;
    // CMPY $4000 (0x10 0xBC 0x40 0x00)
    bus.load(0, &[0x10, 0xBC, 0x40, 0x00]);
    bus.memory[0x4000] = 0x50;
    bus.memory[0x4001] = 0x00;

    // CMPY extended: 8 cycles (2 prefix + 6 execute)
    tick(&mut cpu, &mut bus, 8);

    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Z should be set"
    );
}

#[test]
fn test_cmpd_immediate() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$1234, CMPD #$1234 (0x10 0x83 0x12 0x34)
    bus.load(0, &[0xCC, 0x12, 0x34, 0x10, 0x83, 0x12, 0x34]);

    // LDD (3 cycles)
    tick(&mut cpu, &mut bus, 3);

    // CMPD (5 cycles: 2 prefix + 3 execute)
    tick(&mut cpu, &mut bus, 5);

    assert_eq!(
        cpu.cc & (CcFlag::Z as u8),
        CcFlag::Z as u8,
        "Zero flag should be set"
    );
    assert_eq!(
        cpu.cc & (CcFlag::N as u8),
        0,
        "Negative flag should be clear"
    );
}
