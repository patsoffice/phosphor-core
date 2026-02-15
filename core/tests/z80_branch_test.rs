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

// --- JP nn ---

#[test]
fn test_jp_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0xC3, 0x00, 0x50]); // JP 0x5000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "JP nn should be 10 T-states");
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.memptr, 0x5000);
}

// --- JP cc,nn ---

#[test]
fn test_jp_z_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x40; // Z flag set
    bus.load(0, &[0xCA, 0x00, 0x30]); // JP Z, 0x3000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "JP cc,nn is always 10T");
    assert_eq!(cpu.pc, 0x3000);
}

#[test]
fn test_jp_z_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // Z flag clear
    bus.load(0, &[0xCA, 0x00, 0x30]); // JP Z, 0x3000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "JP cc,nn is always 10T even when not taken");
    assert_eq!(cpu.pc, 3, "PC should be past the JP instruction");
}

#[test]
fn test_jp_nz_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // Z flag clear -> NZ is true
    bus.load(0, &[0xC2, 0x34, 0x12]); // JP NZ, 0x1234

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn test_jp_c_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x01; // C flag set
    bus.load(0, &[0xDA, 0x00, 0x80]); // JP C, 0x8000

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x8000);
}

// --- JR e ---

#[test]
fn test_jr_forward() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x18, 0x10]); // JR +16

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12, "JR should be 12 T-states");
    assert_eq!(cpu.pc, 0x12, "PC = 2 (past JR) + 16 = 0x12");
}

#[test]
fn test_jr_backward() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.pc = 0x100;
    bus.load(0x100, &[0x18, 0xFE]); // JR -2 (infinite loop)

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x100, "JR -2 should loop back to itself");
}

// --- JR cc,e ---

#[test]
fn test_jr_nz_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // Z clear -> NZ true
    bus.load(0, &[0x20, 0x05]); // JR NZ, +5

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12, "JR cc taken should be 12T");
    assert_eq!(cpu.pc, 0x07, "PC = 2 + 5 = 7");
}

#[test]
fn test_jr_nz_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x40; // Z set -> NZ false
    bus.load(0, &[0x20, 0x05]); // JR NZ, +5

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7, "JR cc not taken should be 7T");
    assert_eq!(cpu.pc, 2, "PC should be past the JR instruction");
}

#[test]
fn test_jr_z_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x40; // Z set
    bus.load(0, &[0x28, 0x0A]); // JR Z, +10

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 12);
    assert_eq!(cpu.pc, 0x0C);
}

#[test]
fn test_jr_nc_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // C clear -> NC true
    bus.load(0, &[0x30, 0x03]); // JR NC, +3

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 5);
}

#[test]
fn test_jr_c_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // C clear -> C condition false
    bus.load(0, &[0x38, 0x03]); // JR C, +3

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 7);
    assert_eq!(cpu.pc, 2);
}

// --- JP (HL) ---

#[test]
fn test_jp_hl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x12;
    cpu.l = 0x34;
    bus.load(0, &[0xE9]); // JP (HL)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "JP (HL) should be 4T");
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn test_jp_ix() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.ix = 0xABCD;
    bus.load(0, &[0xDD, 0xE9]); // JP (IX)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DD + JP (IX) = 4+4 = 8T");
    assert_eq!(cpu.pc, 0xABCD);
}

// --- DJNZ ---

#[test]
fn test_djnz_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 5;
    bus.load(0, &[0x10, 0xFE]); // DJNZ -2 (loop back)

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 13, "DJNZ taken should be 13T");
    assert_eq!(cpu.b, 4);
    assert_eq!(cpu.pc, 0x0000, "Should loop back to start");
}

#[test]
fn test_djnz_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 1; // Will become 0 -> not taken
    bus.load(0, &[0x10, 0xFE]); // DJNZ -2

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 8, "DJNZ not taken should be 8T");
    assert_eq!(cpu.b, 0);
    assert_eq!(cpu.pc, 2, "Should fall through");
}

#[test]
fn test_djnz_loop() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 3;
    // Loop: DJNZ -2 (loop back to self)
    bus.load(0, &[0x10, 0xFE]);

    run_instruction(&mut cpu, &mut bus); // B=3->2, taken
    assert_eq!(cpu.b, 2);
    assert_eq!(cpu.pc, 0);

    run_instruction(&mut cpu, &mut bus); // B=2->1, taken
    assert_eq!(cpu.b, 1);
    assert_eq!(cpu.pc, 0);

    run_instruction(&mut cpu, &mut bus); // B=1->0, not taken
    assert_eq!(cpu.b, 0);
    assert_eq!(cpu.pc, 2);
}

// --- CALL nn ---

#[test]
fn test_call_nn() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    bus.load(0, &[0xCD, 0x00, 0x50]); // CALL 0x5000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 17, "CALL nn should be 17T");
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.sp, 0x0FFE);
    // Return address (0x0003) should be on stack
    assert_eq!(bus.memory[0x0FFF], 0x00); // high byte of return addr
    assert_eq!(bus.memory[0x0FFE], 0x03); // low byte of return addr
}

// --- CALL cc,nn ---

#[test]
fn test_call_z_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x2000;
    cpu.f = 0x40; // Z set
    bus.load(0, &[0xCC, 0x00, 0x30]); // CALL Z, 0x3000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 17, "CALL cc taken should be 17T");
    assert_eq!(cpu.pc, 0x3000);
    assert_eq!(cpu.sp, 0x1FFE);
}

#[test]
fn test_call_z_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x2000;
    cpu.f = 0x00; // Z clear
    bus.load(0, &[0xCC, 0x00, 0x30]); // CALL Z, 0x3000

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "CALL cc not taken should be 10T");
    assert_eq!(cpu.pc, 3);
    assert_eq!(cpu.sp, 0x2000, "SP should be unchanged");
}

// --- RET ---

#[test]
fn test_ret() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    bus.memory[0x1000] = 0x34; // low byte
    bus.memory[0x1001] = 0x12; // high byte
    bus.load(0, &[0xC9]); // RET

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 10, "RET should be 10T");
    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cpu.sp, 0x1002);
}

// --- RET cc ---

#[test]
fn test_ret_nz_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.f = 0x00; // Z clear -> NZ true
    bus.memory[0x1000] = 0x00;
    bus.memory[0x1001] = 0x50;
    bus.load(0, &[0xC0]); // RET NZ

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "RET cc taken should be 11T");
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.sp, 0x1002);
}

#[test]
fn test_ret_nz_not_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.f = 0x40; // Z set -> NZ false
    bus.load(0, &[0xC0]); // RET NZ

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 5, "RET cc not taken should be 5T");
    assert_eq!(cpu.pc, 1);
    assert_eq!(cpu.sp, 0x1000, "SP should be unchanged");
}

// --- RST ---

#[test]
fn test_rst_00() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    cpu.pc = 0x0100;
    bus.load(0x100, &[0xC7]); // RST 0x00

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11, "RST should be 11T");
    assert_eq!(cpu.pc, 0x0000);
    assert_eq!(cpu.sp, 0x0FFE);
    assert_eq!(bus.memory[0x0FFF], 0x01); // high byte of return addr (0x0101)
    assert_eq!(bus.memory[0x0FFE], 0x01); // low byte
}

#[test]
fn test_rst_38() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x2000;
    bus.load(0, &[0xFF]); // RST 0x38

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 11);
    assert_eq!(cpu.pc, 0x0038);
    assert_eq!(cpu.sp, 0x1FFE);
}

#[test]
fn test_rst_08() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x3000;
    bus.load(0, &[0xCF]); // RST 0x08

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x0008);
}

// --- CALL/RET round-trip ---

#[test]
fn test_call_ret_roundtrip() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.sp = 0x1000;
    // At 0x0000: CALL 0x5000
    bus.load(0, &[0xCD, 0x00, 0x50]);
    // At 0x5000: RET
    bus.load(0x5000, &[0xC9]);

    run_instruction(&mut cpu, &mut bus); // CALL
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.sp, 0x0FFE);

    run_instruction(&mut cpu, &mut bus); // RET
    assert_eq!(cpu.pc, 0x0003, "Should return to instruction after CALL");
    assert_eq!(cpu.sp, 0x1000, "SP should be restored");
}

// --- DI / EI ---

#[test]
fn test_di() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iff1 = true;
    cpu.iff2 = true;
    bus.load(0, &[0xF3]); // DI

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "DI should be 4T");
    assert!(!cpu.iff1);
    assert!(!cpu.iff2);
}

#[test]
fn test_ei() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.iff1 = false;
    cpu.iff2 = false;
    bus.load(0, &[0xFB]); // EI

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "EI should be 4T");
    assert!(cpu.iff1);
    assert!(cpu.iff2);
}

// --- Condition code coverage ---

#[test]
fn test_jp_po_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // PV clear -> PO (parity odd) true
    bus.load(0, &[0xE2, 0x00, 0x40]); // JP PO, 0x4000

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x4000);
}

#[test]
fn test_jp_pe_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x04; // PV set -> PE (parity even) true
    bus.load(0, &[0xEA, 0x00, 0x40]); // JP PE, 0x4000

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x4000);
}

#[test]
fn test_jp_p_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x00; // S clear -> P (positive) true
    bus.load(0, &[0xF2, 0x00, 0x40]); // JP P, 0x4000

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x4000);
}

#[test]
fn test_jp_m_taken() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.f = 0x80; // S set -> M (minus) true
    bus.load(0, &[0xFA, 0x00, 0x40]); // JP M, 0x4000

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x4000);
}
