use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::z80::Z80;
mod common;
use common::TestBus;

fn run_instruction(cpu: &mut Z80, bus: &mut TestBus) -> u32 {
    let mut cycles = 0;
    loop {
        let done = cpu.tick_with_bus(bus, BusMaster::Cpu(0));
        cycles += 1;
        if done {
            return cycles;
        }
    }
}

// ============================================================
// 16-bit ops with IX/IY prefix (already working via get_rp/set_rp)
// ============================================================

#[test]
fn test_ld_ix_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // DD 21 34 12 → LD IX, 0x1234
    bus.load(0, &[0xDD, 0x21, 0x34, 0x12]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14, "DD LD IX,nn should be 14 T-states (4+10)");
    assert_eq!(cpu.ix, 0x1234);
}

#[test]
fn test_ld_iy_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // FD 21 78 56 → LD IY, 0x5678
    bus.load(0, &[0xFD, 0x21, 0x78, 0x56]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14);
    assert_eq!(cpu.iy, 0x5678);
}

#[test]
fn test_add_ix_bc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1000;
    cpu.b = 0x00; cpu.c = 0x50;
    // DD 09 → ADD IX, BC
    bus.load(0, &[0xDD, 0x09]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "DD ADD IX,BC should be 15 T-states (4+11)");
    assert_eq!(cpu.ix, 0x1050);
}

#[test]
fn test_inc_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1234;
    // DD 23 → INC IX
    bus.load(0, &[0xDD, 0x23]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "DD INC IX should be 10 T-states (4+6)");
    assert_eq!(cpu.ix, 0x1235);
}

#[test]
fn test_dec_iy() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x1000;
    // FD 2B → DEC IY
    bus.load(0, &[0xFD, 0x2B]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.iy, 0x0FFF);
}

#[test]
fn test_push_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0xABCD;
    cpu.sp = 0x1000;
    // DD E5 → PUSH IX
    bus.load(0, &[0xDD, 0xE5]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 15, "DD PUSH IX should be 15 T-states (4+11)");
    assert_eq!(cpu.sp, 0x0FFE);
    assert_eq!(bus.memory[0x0FFF], 0xAB);
    assert_eq!(bus.memory[0x0FFE], 0xCD);
}

#[test]
fn test_pop_iy() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x0FFE;
    bus.memory[0x0FFE] = 0x34;
    bus.memory[0x0FFF] = 0x12;
    // FD E1 → POP IY
    bus.load(0, &[0xFD, 0xE1]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 14, "FD POP IY should be 14 T-states (4+10)");
    assert_eq!(cpu.iy, 0x1234);
}

#[test]
fn test_ld_sp_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x4000;
    // DD F9 → LD SP, IX
    bus.load(0, &[0xDD, 0xF9]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "DD LD SP,IX should be 10 T-states (4+6)");
    assert_eq!(cpu.sp, 0x4000);
}

#[test]
fn test_jp_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1234;
    // DD E9 → JP (IX)
    bus.load(0, &[0xDD, 0xE9]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DD JP (IX) should be 8 T-states (4+4)");
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn test_ex_sp_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0xABCD;
    cpu.sp = 0x1000;
    bus.memory[0x1000] = 0x34;
    bus.memory[0x1001] = 0x12;
    // DD E3 → EX (SP), IX
    bus.load(0, &[0xDD, 0xE3]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23, "DD EX (SP),IX should be 23 T-states (4+19)");
    assert_eq!(cpu.ix, 0x1234);
    assert_eq!(bus.memory[0x1000], 0xCD);
    assert_eq!(bus.memory[0x1001], 0xAB);
}

#[test]
fn test_ld_nn_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0xABCD;
    // DD 22 00 20 → LD (0x2000), IX
    bus.load(0, &[0xDD, 0x22, 0x00, 0x20]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 20, "DD LD (nn),IX should be 20 T-states (4+16)");
    assert_eq!(bus.memory[0x2000], 0xCD);
    assert_eq!(bus.memory[0x2001], 0xAB);
}

#[test]
fn test_ld_ix_nn_ind() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.memory[0x2000] = 0x34;
    bus.memory[0x2001] = 0x12;
    // DD 2A 00 20 → LD IX, (0x2000)
    bus.load(0, &[0xDD, 0x2A, 0x00, 0x20]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 20, "DD LD IX,(nn) should be 20 T-states (4+16)");
    assert_eq!(cpu.ix, 0x1234);
}

// ============================================================
// Undocumented IXH/IXL/IYH/IYL register access
// ============================================================

#[test]
fn test_ld_ixh_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x0000;
    // DD 26 42 → LD IXH, 0x42
    bus.load(0, &[0xDD, 0x26, 0x42]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "DD LD IXH,n should be 11 T-states (4+7)");
    assert_eq!(cpu.ix, 0x4200);
}

#[test]
fn test_ld_ixl_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x0000;
    // DD 2E 55 → LD IXL, 0x55
    bus.load(0, &[0xDD, 0x2E, 0x55]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.ix, 0x0055);
}

#[test]
fn test_ld_a_ixh() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x4200;
    cpu.a = 0x00;
    // DD 7C → LD A, IXH
    bus.load(0, &[0xDD, 0x7C]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DD LD A,IXH should be 8 T-states (4+4)");
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_ld_b_iyl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x0033;
    cpu.b = 0x00;
    // FD 45 → LD B, IYL
    bus.load(0, &[0xFD, 0x45]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.b, 0x33);
}

#[test]
fn test_add_a_ixh() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.ix = 0x2000;
    // DD 84 → ADD A, IXH
    bus.load(0, &[0xDD, 0x84]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DD ADD A,IXH should be 8 T-states (4+4)");
    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_inc_ixh() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x4200;
    cpu.f = 0;
    // DD 24 → INC IXH
    bus.load(0, &[0xDD, 0x24]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DD INC IXH should be 8 T-states (4+4)");
    assert_eq!(cpu.ix, 0x4300);
}

#[test]
fn test_dec_iyl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x0010;
    cpu.f = 0;
    // FD 2D → DEC IYL
    bus.load(0, &[0xFD, 0x2D]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.iy, 0x000F);
}

// ============================================================
// LD r,(IX+d) / LD (IX+d),r — indexed memory access
// ============================================================

#[test]
fn test_ld_a_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x1000;
    bus.memory[0x1005] = 0x42;
    // DD 7E 05 → LD A, (IX+5)
    bus.load(0, &[0xDD, 0x7E, 0x05]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19, "DD LD A,(IX+d) should be 19 T-states");
    assert_eq!(cpu.a, 0x42);
}

#[test]
fn test_ld_b_iy_d_negative() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x1010;
    bus.memory[0x100B] = 0x77; // 0x1010 + (-5) = 0x100B
    // FD 46 FB → LD B, (IY-5)
    bus.load(0, &[0xFD, 0x46, 0xFB]); // 0xFB = -5 signed
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19);
    assert_eq!(cpu.b, 0x77);
}

#[test]
fn test_ld_ix_d_c() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    cpu.c = 0x55;
    // DD 71 03 → LD (IX+3), C
    bus.load(0, &[0xDD, 0x71, 0x03]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19, "DD LD (IX+d),r should be 19 T-states");
    assert_eq!(bus.memory[0x2003], 0x55);
}

#[test]
fn test_ld_iy_d_a() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x3000;
    cpu.a = 0xAA;
    // FD 77 FE → LD (IY-2), A
    bus.load(0, &[0xFD, 0x77, 0xFE]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19);
    assert_eq!(bus.memory[0x2FFE], 0xAA);
}

// ============================================================
// LD (IX+d),n — indexed immediate store
// ============================================================

#[test]
fn test_ld_ix_d_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    // DD 36 05 42 → LD (IX+5), 0x42
    bus.load(0, &[0xDD, 0x36, 0x05, 0x42]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19, "DD LD (IX+d),n should be 19 T-states");
    assert_eq!(bus.memory[0x2005], 0x42);
}

#[test]
fn test_ld_iy_d_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x3000;
    // FD 36 FC 99 → LD (IY-4), 0x99
    bus.load(0, &[0xFD, 0x36, 0xFC, 0x99]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19);
    assert_eq!(bus.memory[0x2FFC], 0x99);
}

// ============================================================
// ALU A,(IX+d) — indexed ALU operations
// ============================================================

#[test]
fn test_add_a_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x10;
    cpu.ix = 0x1000;
    bus.memory[0x1003] = 0x20;
    // DD 86 03 → ADD A, (IX+3)
    bus.load(0, &[0xDD, 0x86, 0x03]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19, "DD ADD A,(IX+d) should be 19 T-states");
    assert_eq!(cpu.a, 0x30);
}

#[test]
fn test_cp_iy_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.iy = 0x2000;
    bus.memory[0x2005] = 0x42;
    // FD BE 05 → CP (IY+5)
    bus.load(0, &[0xFD, 0xBE, 0x05]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 19);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (match)");
    assert_eq!(cpu.a, 0x42, "A should be unchanged after CP");
}

#[test]
fn test_and_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0xFF;
    cpu.ix = 0x1000;
    bus.memory[0x100A] = 0x0F;
    // DD A6 0A → AND (IX+10)
    bus.load(0, &[0xDD, 0xA6, 0x0A]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x0F);
}

// ============================================================
// INC/DEC (IX+d) — indexed increment/decrement
// ============================================================

#[test]
fn test_inc_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    cpu.f = 0;
    bus.memory[0x2005] = 0x41;
    // DD 34 05 → INC (IX+5)
    bus.load(0, &[0xDD, 0x34, 0x05]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23, "DD INC (IX+d) should be 23 T-states");
    assert_eq!(bus.memory[0x2005], 0x42);
}

#[test]
fn test_dec_iy_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x3000;
    cpu.f = 0;
    bus.memory[0x3002] = 0x01;
    // FD 35 02 → DEC (IY+2)
    bus.load(0, &[0xFD, 0x35, 0x02]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23, "FD DEC (IY+d) should be 23 T-states");
    assert_eq!(bus.memory[0x3002], 0x00);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
}

#[test]
fn test_inc_ix_d_negative_offset() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2010;
    bus.memory[0x200B] = 0xFF; // 0x2010 + (-5) = 0x200B
    // DD 34 FB → INC (IX-5)
    bus.load(0, &[0xDD, 0x34, 0xFB]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(bus.memory[0x200B], 0x00);
}

// ============================================================
// DD CB d op — indexed CB (bit) operations
// ============================================================

#[test]
fn test_bit_3_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    bus.memory[0x2005] = 0x08; // Bit 3 is set
    // DD CB 05 5E → BIT 3, (IX+5)
    bus.load(0, &[0xDD, 0xCB, 0x05, 0x5E]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 20, "DD CB BIT should be 20 T-states");
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear (bit is set)");
}

#[test]
fn test_bit_7_ix_d_not_set() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    bus.memory[0x2003] = 0x7F; // Bit 7 is clear
    // DD CB 03 7E → BIT 7, (IX+3)
    bus.load(0, &[0xDD, 0xCB, 0x03, 0x7E]);
    run_instruction(&mut cpu, &mut bus);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (bit is clear)");
}

#[test]
fn test_set_5_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    bus.memory[0x2005] = 0x00;
    // DD CB 05 EE → SET 5, (IX+5)
    bus.load(0, &[0xDD, 0xCB, 0x05, 0xEE]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23, "DD CB SET should be 23 T-states");
    assert_eq!(bus.memory[0x2005], 0x20);
}

#[test]
fn test_res_0_iy_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x3000;
    bus.memory[0x3002] = 0xFF;
    // FD CB 02 86 → RES 0, (IY+2)
    bus.load(0, &[0xFD, 0xCB, 0x02, 0x86]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23);
    assert_eq!(bus.memory[0x3002], 0xFE);
}

#[test]
fn test_rlc_ix_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    bus.memory[0x2005] = 0x81; // 1000_0001
    // DD CB 05 06 → RLC (IX+5)
    bus.load(0, &[0xDD, 0xCB, 0x05, 0x06]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 23, "DD CB RLC should be 23 T-states");
    assert_eq!(bus.memory[0x2005], 0x03); // 0000_0011
    assert_ne!(cpu.f & 0x01, 0, "C should be set (bit 7 was 1)");
}

#[test]
fn test_srl_iy_d() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iy = 0x3000;
    bus.memory[0x3001] = 0x82; // 1000_0010
    // FD CB 01 3E → SRL (IY+1)
    bus.load(0, &[0xFD, 0xCB, 0x01, 0x3E]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(bus.memory[0x3001], 0x41); // 0100_0001
}

#[test]
fn test_indexed_cb_undocumented_reg_copy() {
    // DD CB d op where zzz != 6: result is also copied to register
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0x2000;
    cpu.b = 0x00;
    bus.memory[0x2005] = 0x00;
    // DD CB 05 C0 → SET 0, (IX+5) → B (undocumented: SET 0,(IX+d),B)
    bus.load(0, &[0xDD, 0xCB, 0x05, 0xC0]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(bus.memory[0x2005], 0x01, "Memory should have bit 0 set");
    assert_eq!(cpu.b, 0x01, "B should get copy of result (undocumented)");
}

// ============================================================
// DD/FD prefix chaining and edge cases
// ============================================================

#[test]
fn test_dd_dd_overrides() {
    // DD DD 21 → second DD overrides first, becomes LD IX,nn
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xDD, 0xDD, 0x21, 0x34, 0x12]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    // DD (4T) + DD (4T, overrides) + LD IX,nn (10T) = 18T
    assert_eq!(cycles, 18);
    assert_eq!(cpu.ix, 0x1234);
}

#[test]
fn test_dd_fd_overrides_to_iy() {
    // DD FD 21 → FD overrides DD, becomes LD IY,nn
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xDD, 0xFD, 0x21, 0x78, 0x56]);
    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 18);
    assert_eq!(cpu.iy, 0x5678);
}

#[test]
fn test_dd_ed_resets_index() {
    // DD ED xx → ED prefix resets index_mode to HL
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    // DD ED 47 → LD I,A (ED resets index to HL)
    bus.load(0, &[0xDD, 0xED, 0x47]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.i, 0x42);
}

// ============================================================
// EX DE,HL is NOT affected by DD/FD prefix
// ============================================================

#[test]
fn test_dd_ex_de_hl_not_affected() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.d = 0x12; cpu.e = 0x34;
    cpu.h = 0x56; cpu.l = 0x78;
    cpu.ix = 0xAAAA;
    // DD EB → EX DE,HL (IX not involved)
    bus.load(0, &[0xDD, 0xEB]);
    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_de(), 0x5678);
    assert_eq!(cpu.get_hl(), 0x1234);
    assert_eq!(cpu.ix, 0xAAAA, "IX should be unchanged");
}
