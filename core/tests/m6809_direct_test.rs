use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

// --- 8-bit load/store direct ---

#[test]
fn test_lda_direct_dp_zero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // Store a value at RAM[0x20], then load it via LDA direct
    bus.memory[0x20] = 0x42;
    bus.load(0, &[0x96, 0x20]); // LDA $20

    // 4 cycles: fetch opcode, fetch addr, form DP:addr, read operand
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_lda_direct_dp_nonzero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // DP=$10, addr=$20 -> effective address = $1020
    cpu.dp = 0x10;
    bus.memory[0x1020] = 0x7F;
    bus.load(0, &[0x96, 0x20]); // LDA $20 (effective: $1020)

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x7F);
    assert_eq!(cpu.dp, 0x10);
}

#[test]
fn test_lda_direct_negative() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x80;
    bus.load(0, &[0x96, 0x10]); // LDA $10

    // 4 cycles: fetch opcode, fetch addr, form DP:addr, read operand
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_lda_direct_zero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // RAM defaults to 0
    bus.load(0, &[0x96, 0x10]); // LDA $10

    // 4 cycles: fetch opcode, fetch addr, form DP:addr, read operand
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_ldb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x30] = 0xAB;
    bus.load(0, &[0xD6, 0x30]); // LDB $30

    // 4 cycles: fetch opcode, fetch addr, form DP:addr, read operand
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.b, 0xAB);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_sta_direct_dp_combining() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // DP=$10, STA $20 should store to $1020
    cpu.dp = 0x10;
    bus.load(0, &[0x86, 0x55, 0x97, 0x20]); // LDA #$55, STA $20

    // LDA #$55: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STA $20: 4 cycles (fetch opcode, fetch addr, form DP:addr, write)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(
        bus.memory[0x1020], 0x55,
        "STA should write to DP:addr = $1020"
    );
    assert_eq!(bus.memory[0x20], 0x00, "RAM[0x20] should be untouched");
}

#[test]
fn test_stb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC6, 0x77, 0xD7, 0x40]); // LDB #$77, STB $40

    // LDB #$77: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STB $40: 4 cycles (fetch opcode, fetch addr, form DP:addr, write)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(bus.memory[0x40], 0x77);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

// --- 8-bit ALU direct ---

#[test]
fn test_adda_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x20;
    // LDA #$10, ADDA $10 (adds value at RAM[$10] = $20)
    bus.load(0, &[0x86, 0x10, 0x9B, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ADDA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x30);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_suba_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x05;
    // LDA #$10, SUBA $10
    bus.load(0, &[0x86, 0x10, 0x90, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // SUBA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x0B);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpa_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x10;
    // LDA #$10, CMPA $10 -> equal
    bus.load(0, &[0x86, 0x10, 0x91, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // CMPA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x10, "CMPA should not modify A");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_anda_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0xF0;
    // LDA #$CC, ANDA $10 -> CC & F0 = C0
    bus.load(0, &[0x86, 0xCC, 0x94, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ANDA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0xC0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_ora_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x03;
    // LDA #$C0, ORA $10 -> C0 | 03 = C3
    bus.load(0, &[0x86, 0xC0, 0x9A, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ORA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0xC3);
}

#[test]
fn test_eora_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0xFF;
    // LDA #$CC, EORA $10 -> CC ^ FF = 33
    bus.load(0, &[0x86, 0xCC, 0x98, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // EORA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x33);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_bita_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x00;
    // LDA #$FF, BITA $10 -> FF & 00 = 00, Z=1
    bus.load(0, &[0x86, 0xFF, 0x95, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // BITA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0xFF, "BITA should not modify A");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_adca_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x00;
    // LDA #$FF, ADDA #$01 (sets C), ADCA $10 -> 00 + 00 + 1 = 01
    bus.load(0, &[0x86, 0xFF, 0x8B, 0x01, 0x99, 0x10]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // ADDA -> A=00, C=1
    // ADCA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x01);
}

#[test]
fn test_sbca_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x01;
    // LDA #$00, SUBA #$01 (sets C), SBCA $10 -> FF - 01 - 1 = FD
    bus.load(0, &[0x86, 0x00, 0x80, 0x01, 0x92, 0x10]);

    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // LDA
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // SUBA -> A=FF, C=1
    // SBCA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0xFD);
}

// --- B register ALU direct ---

#[test]
fn test_addb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x20;
    // LDB #$10, ADDB $10
    bus.load(0, &[0xC6, 0x10, 0xDB, 0x10]);

    // LDB: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ADDB direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.b, 0x30);
}

#[test]
fn test_subb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x05;
    // LDB #$10, SUBB $10
    bus.load(0, &[0xC6, 0x10, 0xD0, 0x10]);

    // LDB: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // SUBB direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.b, 0x0B);
}

#[test]
fn test_cmpb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x20;
    // LDB #$10, CMPB $10 -> 10 - 20 = F0, N=1, C=1
    bus.load(0, &[0xC6, 0x10, 0xD1, 0x10]);

    // LDB: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // CMPB direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.b, 0x10, "CMPB should not modify B");
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_andb_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x0F;
    // LDB #$FF, ANDB $10 -> FF & 0F = 0F
    bus.load(0, &[0xC6, 0xFF, 0xD4, 0x10]);

    // LDB: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ANDB direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.b, 0x0F);
}

// --- 16-bit load/store direct ---

#[test]
fn test_ldd_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x12;
    bus.memory[0x21] = 0x34;
    bus.load(0, &[0xDC, 0x20]); // LDD $20

    // 5 cycles: fetch opcode, fetch addr, form DP:addr, read high byte, read low byte
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x12);
    assert_eq!(cpu.b, 0x34);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_ldd_direct_dp_nonzero() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    cpu.dp = 0x05;
    bus.memory[0x0510] = 0xAB;
    bus.memory[0x0511] = 0xCD;
    bus.load(0, &[0xDC, 0x10]); // LDD $10 (effective: $0510)

    // 5 cycles: fetch opcode, fetch addr, form DP:addr, read high byte, read low byte
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0xAB);
    assert_eq!(cpu.b, 0xCD);
}

#[test]
fn test_std_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDD #$ABCD, STD $30
    bus.load(0, &[0xCC, 0xAB, 0xCD, 0xDD, 0x30]);

    // LDD: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STD: 5 cycles (fetch opcode, fetch addr, form DP:addr, write high, write low)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(bus.memory[0x30], 0xAB);
    assert_eq!(bus.memory[0x31], 0xCD);
}

#[test]
fn test_ldx_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x56;
    bus.memory[0x21] = 0x78;
    bus.load(0, &[0x9E, 0x20]); // LDX $20

    // 5 cycles: fetch opcode, fetch addr, form DP:addr, read high byte, read low byte
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.x, 0x5678);
}

#[test]
fn test_stx_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDX #$1234, STX $30
    bus.load(0, &[0x8E, 0x12, 0x34, 0x9F, 0x30]);

    // LDX: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STX: 5 cycles (fetch opcode, fetch addr, form DP:addr, write high, write low)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(bus.memory[0x30], 0x12);
    assert_eq!(bus.memory[0x31], 0x34);
}

#[test]
fn test_ldu_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x9A;
    bus.memory[0x21] = 0xBC;
    bus.load(0, &[0xDE, 0x20]); // LDU $20

    // 5 cycles: fetch opcode, fetch addr, form DP:addr, read high byte, read low byte
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.u, 0x9ABC);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
}

#[test]
fn test_stu_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDU #$BEEF, STU $40
    bus.load(0, &[0xCE, 0xBE, 0xEF, 0xDF, 0x40]);

    // LDU: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STU: 5 cycles (fetch opcode, fetch addr, form DP:addr, write high, write low)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(bus.memory[0x40], 0xBE);
    assert_eq!(bus.memory[0x41], 0xEF);
}

// --- 16-bit ALU direct ---

#[test]
fn test_subd_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x00;
    bus.memory[0x21] = 0x10;
    // LDD #$0100, SUBD $20 -> 0100 - 0010 = 00F0
    bus.load(0, &[0xCC, 0x01, 0x00, 0x93, 0x20]);

    // LDD: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // SUBD direct: 6 cycles (fetch opcode, fetch addr, form DP:addr, read high, read low, execute)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.b, 0xF0);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_addd_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x00;
    bus.memory[0x21] = 0x10;
    // LDD #$0100, ADDD $20 -> 0100 + 0010 = 0110
    bus.load(0, &[0xCC, 0x01, 0x00, 0xD3, 0x20]);

    // LDD: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ADDD direct: 6 cycles (fetch opcode, fetch addr, form DP:addr, read high, read low, execute)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.b, 0x10);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0x12;
    bus.memory[0x21] = 0x34;
    // LDX #$1234, CMPX $20 -> equal, Z=1
    bus.load(0, &[0x8E, 0x12, 0x34, 0x9C, 0x20]);

    // LDX: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // CMPX direct: 6 cycles (fetch opcode, fetch addr, form DP:addr, read high, read low, execute)
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.x, 0x1234, "CMPX should not modify X");
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_direct_less() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x20] = 0xFF;
    bus.memory[0x21] = 0xFF;
    // LDX #$0001, CMPX $20 -> 0001 - FFFF, C=1
    bus.load(0, &[0x8E, 0x00, 0x01, 0x9C, 0x20]);

    // LDX: 3 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // CMPX direct: 6 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
}

// --- Overflow/edge cases ---

#[test]
fn test_adda_direct_overflow() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x01;
    // LDA #$7F, ADDA $10 -> 7F + 01 = 80, V=1 (signed overflow)
    bus.load(0, &[0x86, 0x7F, 0x9B, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ADDA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.cc & (CcFlag::V as u8), CcFlag::V as u8);
    assert_eq!(cpu.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_adda_direct_carry() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    bus.memory[0x10] = 0x01;
    // LDA #$FF, ADDA $10 -> FF + 01 = 00, C=1, Z=1
    bus.load(0, &[0x86, 0xFF, 0x9B, 0x10]);

    // LDA: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // ADDA direct: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_load_store_roundtrip_direct() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    // LDA #$42, STA $30, LDA #$00, LDA $30 -> A should be $42 again
    bus.load(
        0,
        &[
            0x86, 0x42, // LDA #$42
            0x97, 0x30, // STA $30
            0x86, 0x00, // LDA #$00
            0x96, 0x30, // LDA $30
        ],
    );

    // LDA #$42: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // STA $30: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // LDA #$00: 2 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // LDA $30: 4 cycles
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x42);
}
