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

// --- RLCA ---

#[test]
fn test_rlca() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0x07]); // RLCA

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "RLCA should be 4 T-states");
    assert_eq!(cpu.a, 0x0B); // 00001011
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
}

#[test]
fn test_rlca_no_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x01; // C was set
    bus.load(0, &[0x07]); // RLCA

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x84); // 10000100
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 7 was 0)");
}

#[test]
fn test_rlca_preserves_szpv() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.f = 0xC4; // S, Z, PV all set
    bus.load(0, &[0x07]); // RLCA

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0xC4, 0xC4, "S, Z, PV should be preserved");
}

// --- RRCA ---

#[test]
fn test_rrca() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x85; // 10000101
    cpu.f = 0x00;
    bus.load(0, &[0x0F]); // RRCA

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.a, 0xC2); // 11000010
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
}

#[test]
fn test_rrca_no_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x00;
    bus.load(0, &[0x0F]); // RRCA

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x21); // 00100001
    assert_eq!(cpu.f & 0x01, 0, "C should be clear");
}

// --- RLA ---

#[test]
fn test_rla() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x85; // 10000101
    cpu.f = 0x00; // C clear
    bus.load(0, &[0x17]); // RLA

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.a, 0x0A); // 00001010 (old C=0 to bit 0)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 7)");
}

#[test]
fn test_rla_with_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x01; // C set
    bus.load(0, &[0x17]); // RLA

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x85); // 10000101 (old C=1 to bit 0)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 7 was 0)");
}

// --- RRA ---

#[test]
fn test_rra() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x85; // 10000101
    cpu.f = 0x00; // C clear
    bus.load(0, &[0x1F]); // RRA

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.a, 0x42); // 01000010 (old C=0 to bit 7)
    assert_ne!(cpu.f & 0x01, 0, "C should be set (old bit 0)");
}

#[test]
fn test_rra_with_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42; // 01000010
    cpu.f = 0x01; // C set
    bus.load(0, &[0x1F]); // RRA

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0xA1); // 10100001 (old C=1 to bit 7)
    assert_eq!(cpu.f & 0x01, 0, "C should be clear (old bit 0 was 0)");
}

// --- DAA ---

#[test]
fn test_daa_after_add() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // BCD: 15 + 27 = 42
    cpu.a = 0x15;
    cpu.f = 0x00;
    bus.load(0, &[0xC6, 0x27, 0x27]); // ADD A, 0x27; DAA

    run_instruction(&mut cpu, &mut bus); // ADD A, 0x27
    assert_eq!(cpu.a, 0x3C); // Binary result

    let cycles = run_instruction(&mut cpu, &mut bus); // DAA
    assert_eq!(cycles, 4, "DAA should be 4 T-states");
    assert_eq!(cpu.a, 0x42, "BCD result: 15 + 27 = 42");
}

#[test]
fn test_daa_after_sub() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // BCD: 42 - 15 = 27
    cpu.a = 0x42;
    cpu.f = 0x00;
    bus.load(0, &[0xD6, 0x15, 0x27]); // SUB 0x15; DAA

    run_instruction(&mut cpu, &mut bus); // SUB 0x15
    assert_eq!(cpu.a, 0x2D); // Binary result

    run_instruction(&mut cpu, &mut bus); // DAA
    assert_eq!(cpu.a, 0x27, "BCD result: 42 - 15 = 27");
}

#[test]
fn test_daa_carry() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // BCD: 90 + 15 = 105 -> A=05, C=1
    cpu.a = 0x90;
    cpu.f = 0x00;
    bus.load(0, &[0xC6, 0x15, 0x27]); // ADD A, 0x15; DAA

    run_instruction(&mut cpu, &mut bus); // ADD A, 0x15

    run_instruction(&mut cpu, &mut bus); // DAA
    assert_eq!(cpu.a, 0x05);
    assert_ne!(cpu.f & 0x01, 0, "C should be set (BCD overflow)");
}

// --- CPL ---

#[test]
fn test_cpl() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x55; // 01010101
    cpu.f = 0x00;
    bus.load(0, &[0x2F]); // CPL

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "CPL should be 4 T-states");
    assert_eq!(cpu.a, 0xAA); // 10101010
    assert_ne!(cpu.f & 0x10, 0, "H should be set");
    assert_ne!(cpu.f & 0x02, 0, "N should be set");
}

#[test]
fn test_cpl_preserves_szpvc() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0xC5; // S, Z, PV, C set
    bus.load(0, &[0x2F]); // CPL

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0xFF);
    assert_eq!(cpu.f & 0xC5, 0xC5, "S, Z, PV, C should be preserved");
}

// --- SCF ---

#[test]
fn test_scf() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0x00;
    bus.load(0, &[0x37]); // SCF

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "SCF should be 4 T-states");
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
}

#[test]
fn test_scf_preserves_szpv() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0xC4; // S, Z, PV set
    bus.load(0, &[0x37]); // SCF

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0xC4, 0xC4, "S, Z, PV should be preserved");
}

// --- CCF ---

#[test]
fn test_ccf_from_set() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0x01; // C set
    bus.load(0, &[0x3F]); // CCF

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 4, "CCF should be 4 T-states");
    assert_eq!(cpu.f & 0x01, 0, "C should be cleared");
    assert_ne!(cpu.f & 0x10, 0, "H should be set (old C)");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
}

#[test]
fn test_ccf_from_clear() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x00;
    cpu.f = 0x00; // C clear
    bus.load(0, &[0x3F]); // CCF

    run_instruction(&mut cpu, &mut bus);
    assert_ne!(cpu.f & 0x01, 0, "C should be set");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear (old C was 0)");
}
