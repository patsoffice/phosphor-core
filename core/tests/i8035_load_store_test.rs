use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8035::I8035;
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` machine cycles.
fn tick(cpu: &mut I8035, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// MOV A,Rn (0xF8-0xFF) — 1 cycle
// =============================================================================

#[test]
fn test_mov_a_r0() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x42; // R0 = 0x42
    bus.load(0, &[0xF8]); // MOV A,R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_mov_a_r7() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[7] = 0xBE; // R7 = 0xBE
    bus.load(0, &[0xFF]); // MOV A,R7
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xBE);
}

// =============================================================================
// MOV Rn,A (0xA8-0xAF) — 1 cycle
// =============================================================================

#[test]
fn test_mov_rn_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55;
    bus.load(0, &[0xAB]); // MOV R3,A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ram[3], 0x55);
}

// =============================================================================
// MOV A,@Ri (0xF0-0xF1) — 1 cycle
// =============================================================================

#[test]
fn test_mov_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x30; // R0 = 0x30 (pointer)
    cpu.ram[0x30] = 0xAB;
    bus.load(0, &[0xF0]); // MOV A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xAB);
}

// =============================================================================
// MOV @Ri,A (0xA0-0xA1) — 1 cycle
// =============================================================================

#[test]
fn test_mov_indirect_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xCD;
    cpu.ram[1] = 0x20; // R1 = 0x20 (pointer)
    bus.load(0, &[0xA1]); // MOV @R1,A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ram[0x20], 0xCD);
}

// =============================================================================
// MOV A,#data (0x23) — 2 cycles
// =============================================================================

#[test]
fn test_mov_a_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x23, 0x99]); // MOV A,#0x99
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x99);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// MOV Rn,#data (0xB8-0xBF) — 2 cycles
// =============================================================================

#[test]
fn test_mov_rn_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xBC, 0x77]); // MOV R4,#0x77
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ram[4], 0x77);
    assert_eq!(cpu.pc, 2);
}

// =============================================================================
// MOV @Ri,#data (0xB0-0xB1) — 2 cycles
// =============================================================================

#[test]
fn test_mov_indirect_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x20; // R0 = 0x20 (pointer)
    bus.load(0, &[0xB0, 0xAA]); // MOV @R0,#0xAA
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ram[0x20], 0xAA);
}

// =============================================================================
// XCH A,Rn (0x28-0x2F) — 1 cycle
// =============================================================================

#[test]
fn test_xch_a_rn() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x11;
    cpu.ram[2] = 0x22; // R2 = 0x22
    bus.load(0, &[0x2A]); // XCH A,R2
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x22);
    assert_eq!(cpu.ram[2], 0x11);
}

// =============================================================================
// XCH A,@Ri (0x20-0x21) — 1 cycle
// =============================================================================

#[test]
fn test_xch_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xAA;
    cpu.ram[0] = 0x30; // R0 = 0x30 (pointer)
    cpu.ram[0x30] = 0x55;
    bus.load(0, &[0x20]); // XCH A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x55);
    assert_eq!(cpu.ram[0x30], 0xAA);
}

// =============================================================================
// XCHD A,@Ri (0x30-0x31) — 1 cycle
// =============================================================================

#[test]
fn test_xchd_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xA5; // A = 0xA5
    cpu.ram[0] = 0x20; // R0 = 0x20 (pointer)
    cpu.ram[0x20] = 0x3C; // RAM[0x20] = 0x3C
    bus.load(0, &[0x30]); // XCHD A,@R0
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xAC); // high nibble A preserved, low nibble from RAM
    assert_eq!(cpu.ram[0x20], 0x35); // high nibble RAM preserved, low nibble from A
}

// =============================================================================
// MOV A,T (0x42) — 1 cycle
// =============================================================================

#[test]
fn test_mov_a_t() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.t = 0x42;
    bus.load(0, &[0x42]); // MOV A,T
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0x42);
}

// =============================================================================
// MOV T,A (0x62) — 1 cycle
// =============================================================================

#[test]
fn test_mov_t_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xBB;
    bus.load(0, &[0x62]); // MOV T,A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.t, 0xBB);
}

// =============================================================================
// MOV A,PSW (0xC7) — 1 cycle
// =============================================================================

#[test]
fn test_mov_a_psw() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.psw = 0xD0; // CY + BS + F0
    bus.load(0, &[0xC7]); // MOV A,PSW
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.a, 0xD0);
}

// =============================================================================
// MOV PSW,A (0xD7) — 1 cycle
// =============================================================================

#[test]
fn test_mov_psw_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xB2; // sets CY, AC, BS, SP=2
    bus.load(0, &[0xD7]); // MOV PSW,A
    tick(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.psw, 0xB2);
}

// =============================================================================
// MOVX A,@Ri (0x80-0x81) — 2 cycles, external RAM via io_read
// =============================================================================

#[test]
fn test_movx_a_indirect() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0] = 0x50; // R0 = 0x50 (external address)
    // TestBus io_read defaults to read, so set memory[0x50]
    bus.memory[0x50] = 0xEE;
    bus.load(0, &[0x80]); // MOVX A,@R0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xEE);
}

// =============================================================================
// MOVX @Ri,A (0x90-0x91) — 2 cycles, external RAM via io_write
// =============================================================================

#[test]
fn test_movx_indirect_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xDD;
    cpu.ram[1] = 0x60; // R1 = 0x60 (external address)
    bus.load(0, &[0x91]); // MOVX @R1,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(bus.memory[0x60], 0xDD);
}

// =============================================================================
// MOVP A,@A (0xA3) — 2 cycles, read from current page
// =============================================================================

#[test]
fn test_movp_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x05;
    // PC will be 1 after opcode fetch (page 0)
    // Target: (PC & 0xF00) | A = 0x000 | 0x05 = 0x005
    bus.memory[0x05] = 0x42;
    bus.load(0, &[0xA3]); // MOVP A,@A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_movp_a_page_1() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x100; // page 1
    cpu.a = 0x0A;
    // Target: (0x100 & 0xF00) | 0x0A = 0x10A
    bus.memory[0x10A] = 0x77;
    bus.load(0x100, &[0xA3]); // MOVP A,@A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x77);
}

// =============================================================================
// MOVP3 A,@A (0xE3) — 2 cycles, read from page 3
// =============================================================================

#[test]
fn test_movp3_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x20;
    // Target: 0x300 | 0x20 = 0x320
    bus.memory[0x320] = 0xBB;
    bus.load(0, &[0xE3]); // MOVP3 A,@A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xBB);
}

// =============================================================================
// INS A,BUS (0x08) — 2 cycles
// =============================================================================

#[test]
fn test_ins_a_bus() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // TestBus io_read defaults to read: port 0x100 maps to memory[0x100]
    bus.memory[0x100] = 0xAB;
    bus.load(0, &[0x08]); // INS A,BUS
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xAB);
}

// =============================================================================
// IN A,P1 (0x09) — 2 cycles
// =============================================================================

#[test]
fn test_in_a_p1() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x101] = 0xCC; // PORT_P1 = 0x101
    bus.load(0, &[0x09]); // IN A,P1
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xCC);
}

// =============================================================================
// IN A,P2 (0x0A) — 2 cycles
// =============================================================================

#[test]
fn test_in_a_p2() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x102] = 0xDD; // PORT_P2 = 0x102
    bus.load(0, &[0x0A]); // IN A,P2
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0xDD);
}

// =============================================================================
// OUTL BUS,A (0x02) — 2 cycles
// =============================================================================

#[test]
fn test_outl_bus_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x77;
    bus.load(0, &[0x02]); // OUTL BUS,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.dbbb, 0x77);
    assert_eq!(bus.memory[0x100], 0x77); // io_write to PORT_BUS
}

// =============================================================================
// OUTL P1,A (0x39) — 2 cycles
// =============================================================================

#[test]
fn test_outl_p1_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x88;
    bus.load(0, &[0x39]); // OUTL P1,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p1, 0x88);
    assert_eq!(bus.memory[0x101], 0x88);
}

// =============================================================================
// OUTL P2,A (0x3A) — 2 cycles
// =============================================================================

#[test]
fn test_outl_p2_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0x99;
    bus.load(0, &[0x3A]); // OUTL P2,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p2, 0x99);
    assert_eq!(bus.memory[0x102], 0x99);
}

// =============================================================================
// Port RMW: ANL BUS,#data (0x98) — 2 cycles
// =============================================================================

#[test]
fn test_anl_bus_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.dbbb = 0xFF;
    bus.load(0, &[0x98, 0x0F]); // ANL BUS,#0x0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.dbbb, 0x0F);
    assert_eq!(bus.memory[0x100], 0x0F);
}

// =============================================================================
// Port RMW: ORL BUS,#data (0x88) — 2 cycles
// =============================================================================

#[test]
fn test_orl_bus_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.dbbb = 0xA0;
    bus.load(0, &[0x88, 0x05]); // ORL BUS,#0x05
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.dbbb, 0xA5);
    assert_eq!(bus.memory[0x100], 0xA5);
}

// =============================================================================
// Port RMW: ANL P1,#data (0x99) — 2 cycles
// =============================================================================

#[test]
fn test_anl_p1_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.p1 = 0xFF;
    bus.load(0, &[0x99, 0xF0]); // ANL P1,#0xF0
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p1, 0xF0);
}

// =============================================================================
// Port RMW: ORL P2,#data (0x8A) — 2 cycles
// =============================================================================

#[test]
fn test_orl_p2_imm() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.p2 = 0x00;
    bus.load(0, &[0x8A, 0x0F]); // ORL P2,#0x0F
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p2, 0x0F);
}

// =============================================================================
// MOVD A,Pp (0x0C-0x0F) — 2 cycles
// =============================================================================

#[test]
fn test_movd_a_p4() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    // PORT_P4 = 0x104
    bus.memory[0x104] = 0x3F; // only low nibble used
    bus.load(0, &[0x0C]); // MOVD A,P4
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.a, 0x0F); // masked to low nibble
}

// =============================================================================
// MOVD Pp,A (0x3C-0x3F) — 2 cycles
// =============================================================================

#[test]
fn test_movd_p5_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.a = 0xF9; // only low nibble used
    bus.load(0, &[0x3D]); // MOVD P5,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(bus.memory[0x105], 0x09); // low nibble of A
}

// =============================================================================
// ORLD Pp,A (0x8C-0x8F) — 2 cycles
// =============================================================================

#[test]
fn test_orld_p4_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x104] = 0x05; // current expander value
    cpu.a = 0x0A;
    bus.load(0, &[0x8C]); // ORLD P4,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(bus.memory[0x104], 0x0F); // 0x05 | 0x0A = 0x0F
}

// =============================================================================
// ANLD Pp,A (0x9C-0x9F) — 2 cycles
// =============================================================================

#[test]
fn test_anld_p6_a() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    bus.memory[0x106] = 0x0F; // current expander value
    cpu.a = 0x0A;
    bus.load(0, &[0x9E]); // ANLD P6,A
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(bus.memory[0x106], 0x0A); // 0x0F & (0x0A | 0xF0) = 0x0A
}

// =============================================================================
// Bank 1 register operations
// =============================================================================

#[test]
fn test_bank1_mov() {
    let mut cpu = I8035::new();
    let mut bus = TestBus::new();
    cpu.ram[0x18] = 0xDD; // Bank 1 R0
    // SEL RB1 (0xD5), MOV A,R0 (0xF8)
    bus.load(0, &[0xD5, 0xF8]);
    tick(&mut cpu, &mut bus, 1); // SEL RB1
    tick(&mut cpu, &mut bus, 1); // MOV A,R0
    assert_eq!(cpu.a, 0xDD);
}
