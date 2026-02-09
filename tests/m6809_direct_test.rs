use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

// --- 8-bit load/store direct ---

#[test]
fn test_lda_direct_dp_zero() {
    let mut sys = Simple6809System::new();
    // Store a value at RAM[0x20], then load it via LDA direct
    sys.write_ram(0x20, 0x42);
    sys.load_rom(0, &[0x96, 0x20]); // LDA $20

    // 3 cycles: fetch opcode, fetch addr + form DP:addr, read operand
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x42);
    assert_eq!(state.pc, 2);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_lda_direct_dp_nonzero() {
    let mut sys = Simple6809System::new();
    // DP=$10, addr=$20 -> effective address = $1020
    sys.set_cpu_dp(0x10);
    sys.write_ram(0x1020, 0x7F);
    sys.load_rom(0, &[0x96, 0x20]); // LDA $20 (effective: $1020)

    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x7F);
    assert_eq!(state.dp, 0x10);
}

#[test]
fn test_lda_direct_negative() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x80);
    sys.load_rom(0, &[0x96, 0x10]); // LDA $10

    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x80);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_lda_direct_zero() {
    let mut sys = Simple6809System::new();
    // RAM defaults to 0
    sys.load_rom(0, &[0x96, 0x10]); // LDA $10

    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x00);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_ldb_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x30, 0xAB);
    sys.load_rom(0, &[0xD6, 0x30]); // LDB $30

    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0xAB);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}

#[test]
fn test_sta_direct_dp_combining() {
    let mut sys = Simple6809System::new();
    // DP=$10, STA $20 should store to $1020
    sys.set_cpu_dp(0x10);
    sys.load_rom(0, &[0x86, 0x55, 0x97, 0x20]); // LDA #$55, STA $20

    // LDA #$55: 2 cycles
    sys.tick();
    sys.tick();
    // STA $20: 3 cycles (fetch opcode, fetch addr, write)
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(
        sys.read_ram(0x1020),
        0x55,
        "STA should write to DP:addr = $1020"
    );
    assert_eq!(sys.read_ram(0x20), 0x00, "RAM[0x20] should be untouched");
}

#[test]
fn test_stb_direct() {
    let mut sys = Simple6809System::new();
    sys.load_rom(0, &[0xC6, 0x77, 0xD7, 0x40]); // LDB #$77, STB $40

    // LDB #$77: 2 cycles
    sys.tick();
    sys.tick();
    // STB $40: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.read_ram(0x40), 0x77);
    let state = sys.get_cpu_state();
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}

// --- 8-bit ALU direct ---

#[test]
fn test_adda_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x20);
    // LDA #$10, ADDA $10 (adds value at RAM[$10] = $20)
    sys.load_rom(0, &[0x86, 0x10, 0x9B, 0x10]);

    // LDA: 2 cycles
    sys.tick();
    sys.tick();
    // ADDA direct: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x30);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_suba_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x05);
    // LDA #$10, SUBA $10
    sys.load_rom(0, &[0x86, 0x10, 0x90, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x0B);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpa_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x10);
    // LDA #$10, CMPA $10 -> equal
    sys.load_rom(0, &[0x86, 0x10, 0x91, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x10, "CMPA should not modify A");
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_anda_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0xF0);
    // LDA #$CC, ANDA $10 -> CC & F0 = C0
    sys.load_rom(0, &[0x86, 0xCC, 0x94, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0xC0);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_ora_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x03);
    // LDA #$C0, ORA $10 -> C0 | 03 = C3
    sys.load_rom(0, &[0x86, 0xC0, 0x9A, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().a, 0xC3);
}

#[test]
fn test_eora_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0xFF);
    // LDA #$CC, EORA $10 -> CC ^ FF = 33
    sys.load_rom(0, &[0x86, 0xCC, 0x98, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().a, 0x33);
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), 0);
}

#[test]
fn test_bita_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x00);
    // LDA #$FF, BITA $10 -> FF & 00 = 00, Z=1
    sys.load_rom(0, &[0x86, 0xFF, 0x95, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0xFF, "BITA should not modify A");
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_adca_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x00);
    // LDA #$FF, ADDA #$01 (sets C), ADCA $10 -> 00 + 00 + 1 = 01
    sys.load_rom(0, &[0x86, 0xFF, 0x8B, 0x01, 0x99, 0x10]);

    sys.tick();
    sys.tick(); // LDA
    sys.tick();
    sys.tick(); // ADDA -> A=00, C=1
    sys.tick();
    sys.tick();
    sys.tick(); // ADCA direct

    assert_eq!(sys.get_cpu_state().a, 0x01);
}

#[test]
fn test_sbca_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x01);
    // LDA #$00, SUBA #$01 (sets C), SBCA $10 -> FF - 01 - 1 = FD
    sys.load_rom(0, &[0x86, 0x00, 0x80, 0x01, 0x92, 0x10]);

    sys.tick();
    sys.tick(); // LDA
    sys.tick();
    sys.tick(); // SUBA -> A=FF, C=1
    sys.tick();
    sys.tick();
    sys.tick(); // SBCA direct

    assert_eq!(sys.get_cpu_state().a, 0xFD);
}

// --- B register ALU direct ---

#[test]
fn test_addb_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x20);
    // LDB #$10, ADDB $10
    sys.load_rom(0, &[0xC6, 0x10, 0xDB, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().b, 0x30);
}

#[test]
fn test_subb_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x05);
    // LDB #$10, SUBB $10
    sys.load_rom(0, &[0xC6, 0x10, 0xD0, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().b, 0x0B);
}

#[test]
fn test_cmpb_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x20);
    // LDB #$10, CMPB $10 -> 10 - 20 = F0, N=1, C=1
    sys.load_rom(0, &[0xC6, 0x10, 0xD1, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.b, 0x10, "CMPB should not modify B");
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_andb_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x0F);
    // LDB #$FF, ANDB $10 -> FF & 0F = 0F
    sys.load_rom(0, &[0xC6, 0xFF, 0xD4, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().b, 0x0F);
}

// --- 16-bit load/store direct ---

#[test]
fn test_ldd_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x12);
    sys.write_ram(0x21, 0x34);
    sys.load_rom(0, &[0xDC, 0x20]); // LDD $20

    // 4 cycles: fetch opcode, fetch addr, read high byte, read low byte
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x12);
    assert_eq!(state.b, 0x34);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
}

#[test]
fn test_ldd_direct_dp_nonzero() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_dp(0x05);
    sys.write_ram(0x0510, 0xAB);
    sys.write_ram(0x0511, 0xCD);
    sys.load_rom(0, &[0xDC, 0x10]); // LDD $10 (effective: $0510)

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0xAB);
    assert_eq!(state.b, 0xCD);
}

#[test]
fn test_std_direct() {
    let mut sys = Simple6809System::new();
    // LDD #$ABCD, STD $30
    sys.load_rom(0, &[0xCC, 0xAB, 0xCD, 0xDD, 0x30]);

    // LDD: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // STD: 4 cycles (fetch opcode, fetch addr, write high, write low)
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.read_ram(0x30), 0xAB);
    assert_eq!(sys.read_ram(0x31), 0xCD);
}

#[test]
fn test_ldx_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x56);
    sys.write_ram(0x21, 0x78);
    sys.load_rom(0, &[0x9E, 0x20]); // LDX $20

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().x, 0x5678);
}

#[test]
fn test_stx_direct() {
    let mut sys = Simple6809System::new();
    // LDX #$1234, STX $30
    sys.load_rom(0, &[0x8E, 0x12, 0x34, 0x9F, 0x30]);

    // LDX: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // STX: 4 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.read_ram(0x30), 0x12);
    assert_eq!(sys.read_ram(0x31), 0x34);
}

#[test]
fn test_ldu_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x9A);
    sys.write_ram(0x21, 0xBC);
    sys.load_rom(0, &[0xDE, 0x20]); // LDU $20

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.u, 0x9ABC);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
}

#[test]
fn test_stu_direct() {
    let mut sys = Simple6809System::new();
    // LDU #$BEEF, STU $40
    sys.load_rom(0, &[0xCE, 0xBE, 0xEF, 0xDF, 0x40]);

    // LDU: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // STU: 4 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.read_ram(0x40), 0xBE);
    assert_eq!(sys.read_ram(0x41), 0xEF);
}

// --- 16-bit ALU direct ---

#[test]
fn test_subd_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x00);
    sys.write_ram(0x21, 0x10);
    // LDD #$0100, SUBD $20 -> 0100 - 0010 = 00F0
    sys.load_rom(0, &[0xCC, 0x01, 0x00, 0x93, 0x20]);

    // LDD: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // SUBD direct: 4 cycles (fetch opcode, fetch addr, read high, read low + execute)
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x00);
    assert_eq!(state.b, 0xF0);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_addd_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x00);
    sys.write_ram(0x21, 0x10);
    // LDD #$0100, ADDD $20 -> 0100 + 0010 = 0110
    sys.load_rom(0, &[0xCC, 0x01, 0x00, 0xD3, 0x20]);

    // LDD: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // ADDD direct: 4 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x01);
    assert_eq!(state.b, 0x10);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_direct() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0x12);
    sys.write_ram(0x21, 0x34);
    // LDX #$1234, CMPX $20 -> equal, Z=1
    sys.load_rom(0, &[0x8E, 0x12, 0x34, 0x9C, 0x20]);

    // LDX: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // CMPX direct: 4 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.x, 0x1234, "CMPX should not modify X");
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_direct_less() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x20, 0xFF);
    sys.write_ram(0x21, 0xFF);
    // LDX #$0001, CMPX $20 -> 0001 - FFFF, C=1
    sys.load_rom(0, &[0x8E, 0x00, 0x01, 0x9C, 0x20]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
}

// --- Overflow/edge cases ---

#[test]
fn test_adda_direct_overflow() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x01);
    // LDA #$7F, ADDA $10 -> 7F + 01 = 80, V=1 (signed overflow)
    sys.load_rom(0, &[0x86, 0x7F, 0x9B, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x80);
    assert_eq!(state.cc & (CcFlag::V as u8), CcFlag::V as u8);
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_adda_direct_carry() {
    let mut sys = Simple6809System::new();
    sys.write_ram(0x10, 0x01);
    // LDA #$FF, ADDA $10 -> FF + 01 = 00, C=1, Z=1
    sys.load_rom(0, &[0x86, 0xFF, 0x9B, 0x10]);

    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x00);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
    assert_eq!(state.cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}

#[test]
fn test_load_store_roundtrip_direct() {
    let mut sys = Simple6809System::new();
    // LDA #$42, STA $30, LDA #$00, LDA $30 -> A should be $42 again
    sys.load_rom(
        0,
        &[
            0x86, 0x42, // LDA #$42
            0x97, 0x30, // STA $30
            0x86, 0x00, // LDA #$00
            0x96, 0x30, // LDA $30
        ],
    );

    // LDA #$42: 2 cycles
    sys.tick();
    sys.tick();
    // STA $30: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();
    // LDA #$00: 2 cycles
    sys.tick();
    sys.tick();
    // LDA $30: 3 cycles
    sys.tick();
    sys.tick();
    sys.tick();

    assert_eq!(sys.get_cpu_state().a, 0x42);
}
