/// Tests for M6800 indexed addressing mode (X + unsigned 8-bit offset) operations.
///
/// Indexed mode: 5 cycles for 8-bit ALU, 6 cycles for 8-bit stores,
/// 6 cycles for 16-bit loads/CPX, 7 cycles for 16-bit stores.
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};

mod common;
use common::TestBus;

fn tick(cpu: &mut M6800, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ---- 8-bit ALU indexed ----

#[test]
fn test_suba_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x50;
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0x10; // X + 5
    bus.load(0, &[0xA0, 0x05]); // SUBA 5,X
    tick(&mut cpu, &mut bus, 5); // 5 cycles
    assert_eq!(cpu.a, 0x40);
    assert_eq!(cpu.pc, 2);
}

#[test]
fn test_cmpa_idx_less() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.x = 0x0200;
    bus.memory[0x020A] = 0x20; // X + 10
    bus.load(0, &[0xA1, 0x0A]); // CMPA 10,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x10); // A unchanged
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0); // borrow set
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // negative result
}

#[test]
fn test_anda_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0b1010_1010;
    cpu.x = 0x0300;
    bus.memory[0x0300] = 0b1100_1100; // X + 0
    bus.load(0, &[0xA4, 0x00]); // ANDA 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0b1000_1000);
}

#[test]
fn test_adda_idx_with_half_carry() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    cpu.x = 0x0100;
    bus.memory[0x0110] = 0x01; // X + 0x10
    bus.load(0, &[0xAB, 0x10]); // ADDA $10,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x10);
    assert_ne!(cpu.cc & (CcFlag::H as u8), 0); // half carry
}

#[test]
fn test_eora_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.x = 0x0050;
    bus.memory[0x005A] = 0xFF; // X + 10
    bus.load(0, &[0xA8, 0x0A]); // EORA $0A,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_sbca_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.cc |= CcFlag::C as u8;
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0x01;
    bus.load(0, &[0xA2, 0x05]); // SBCA 5,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x7E); // 0x80 - 0x01 - 1
}

#[test]
fn test_bita_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x80;
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x80;
    bus.load(0, &[0xA5, 0x00]); // BITA 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x80); // unchanged
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_adca_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFE;
    cpu.cc |= CcFlag::C as u8;
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x01;
    bus.load(0, &[0xA9, 0x00]); // ADCA 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x00); // 0xFE + 0x01 + 1 = 0x100
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_oraa_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0x0F;
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0xF0;
    bus.load(0, &[0xAA, 0x05]); // ORAA 5,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0xFF);
}

// ---- B register indexed ----

#[test]
fn test_subb_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x80;
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x01;
    bus.load(0, &[0xE0, 0x00]); // SUBB 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.b, 0x7F);
}

#[test]
fn test_addb_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x10;
    cpu.x = 0x0200;
    bus.memory[0x0220] = 0x20; // X + 0x20
    bus.load(0, &[0xEB, 0x20]); // ADDB $20,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.b, 0x30);
}

#[test]
fn test_orab_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x0F;
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0xF0;
    bus.load(0, &[0xEA, 0x05]); // ORAB 5,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.b, 0xFF);
}

#[test]
fn test_cmpb_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x42;
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x42;
    bus.load(0, &[0xE1, 0x00]); // CMPB 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.b, 0x42); // unchanged
}

#[test]
fn test_andb_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0xAA;
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x0F;
    bus.load(0, &[0xE4, 0x00]); // ANDB 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.b, 0x0A);
}

// ---- 8-bit Load/Store indexed ----

#[test]
fn test_ldaa_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x010A] = 0xCD;
    bus.load(0, &[0xA6, 0x0A]); // LDAA $0A,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0xCD);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_ldab_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0200;
    bus.memory[0x0200] = 0x42;
    bus.load(0, &[0xE6, 0x00]); // LDAB 0,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.b, 0x42);
}

#[test]
fn test_staa_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAB;
    cpu.x = 0x0100;
    bus.load(0, &[0xA7, 0x10]); // STAA $10,X
    tick(&mut cpu, &mut bus, 6); // 6 cycles
    assert_eq!(bus.memory[0x0110], 0xAB);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_stab_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.b = 0x00;
    cpu.x = 0x0300;
    bus.load(0, &[0xE7, 0x05]); // STAB 5,X
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(bus.memory[0x0305], 0x00);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
}

// ---- 16-bit Load/Store/Compare indexed ----

#[test]
fn test_ldx_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0110] = 0x02;
    bus.memory[0x0111] = 0x00;
    bus.load(0, &[0xEE, 0x10]); // LDX $10,X
    tick(&mut cpu, &mut bus, 6); // 6 cycles
    assert_eq!(cpu.x, 0x0200);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_lds_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x01;
    bus.memory[0x0101] = 0xFF;
    bus.load(0, &[0xAE, 0x00]); // LDS 0,X
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.sp, 0x01FF);
}

#[test]
fn test_stx_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.load(0, &[0xEF, 0x20]); // STX $20,X
    tick(&mut cpu, &mut bus, 7); // 7 cycles
    assert_eq!(bus.memory[0x0120], 0x01);
    assert_eq!(bus.memory[0x0121], 0x00);
}

#[test]
fn test_sts_idx() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0200;
    cpu.sp = 0xCAFE;
    bus.load(0, &[0xAF, 0x10]); // STS $10,X
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(bus.memory[0x0210], 0xCA);
    assert_eq!(bus.memory[0x0211], 0xFE);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_cpx_idx_equal() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0105] = 0x01;
    bus.memory[0x0106] = 0x00;
    bus.load(0, &[0xAC, 0x05]); // CPX 5,X
    tick(&mut cpu, &mut bus, 6);
    assert_ne!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.x, 0x0100); // unchanged
}

#[test]
fn test_cpx_idx_less() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x02;
    bus.memory[0x0101] = 0x00;
    bus.load(0, &[0xAC, 0x00]); // CPX 0,X
    tick(&mut cpu, &mut bus, 6);
    assert_ne!(cpu.cc & (CcFlag::N as u8), 0); // X < operand
}

// ---- Index wrapping at offset 0xFF ----

#[test]
fn test_ldaa_idx_max_offset() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x01FF] = 0x42; // X + 0xFF
    bus.load(0, &[0xA6, 0xFF]); // LDAA $FF,X
    tick(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.a, 0x42);
}

// ---- Multi-instruction indexed sequences ----

#[test]
fn test_ldaa_staa_indexed_copy() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    cpu.x = 0x0100;
    bus.memory[0x0100] = 0x55; // source
    // LDAA 0,X; STAA 1,X  (copy byte at X+0 to X+1)
    bus.load(0, &[0xA6, 0x00, 0xA7, 0x01]);
    tick(&mut cpu, &mut bus, 5); // LDAA 0,X
    assert_eq!(cpu.a, 0x55);
    tick(&mut cpu, &mut bus, 6); // STAA 1,X
    assert_eq!(bus.memory[0x0101], 0x55);
}
