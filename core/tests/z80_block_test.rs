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
// LDI
// ============================================================

#[test]
fn test_ldi() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00; // HL = source
    cpu.d = 0x20; cpu.e = 0x00; // DE = dest
    cpu.b = 0x00; cpu.c = 0x03; // BC = count
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0xA0]); // LDI
    bus.memory[0x1000] = 0x42;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "LDI should be 16 T-states");
    assert_eq!(bus.memory[0x2000], 0x42, "Byte should be transferred");
    assert_eq!(cpu.get_hl(), 0x1001, "HL should be incremented");
    assert_eq!(cpu.get_de(), 0x2001, "DE should be incremented");
    assert_eq!(cpu.get_bc(), 0x0002, "BC should be decremented");
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (BC != 0)");
    assert_eq!(cpu.f & 0x02, 0, "N should be clear");
    assert_eq!(cpu.f & 0x10, 0, "H should be clear");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
}

#[test]
fn test_ldi_bc_reaches_zero() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.d = 0x20; cpu.e = 0x00;
    cpu.b = 0x00; cpu.c = 0x01; // BC = 1, will become 0
    bus.load(0, &[0xED, 0xA0]);
    bus.memory[0x1000] = 0x55;

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.get_bc(), 0x0000);
    assert_eq!(cpu.f & 0x04, 0, "PV should be clear (BC == 0)");
}

// ============================================================
// LDD
// ============================================================

#[test]
fn test_ldd() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x05;
    cpu.d = 0x20; cpu.e = 0x05;
    cpu.b = 0x00; cpu.c = 0x03;
    bus.load(0, &[0xED, 0xA8]); // LDD
    bus.memory[0x1005] = 0x77;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16);
    assert_eq!(bus.memory[0x2005], 0x77);
    assert_eq!(cpu.get_hl(), 0x1004, "HL should be decremented");
    assert_eq!(cpu.get_de(), 0x2004, "DE should be decremented");
    assert_eq!(cpu.get_bc(), 0x0002);
}

// ============================================================
// LDIR
// ============================================================

#[test]
fn test_ldir() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.d = 0x20; cpu.e = 0x00;
    cpu.b = 0x00; cpu.c = 0x03;
    cpu.f = 0x01;
    bus.load(0, &[0xED, 0xB0]); // LDIR
    bus.memory[0x1000] = 0xAA;
    bus.memory[0x1001] = 0xBB;
    bus.memory[0x1002] = 0xCC;

    // Run 3 iterations
    let cycles1 = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles1, 21, "LDIR repeating should be 21 T-states");
    assert_eq!(bus.memory[0x2000], 0xAA);
    assert_eq!(cpu.get_bc(), 0x0002);

    let cycles2 = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles2, 21);
    assert_eq!(bus.memory[0x2001], 0xBB);

    let cycles3 = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles3, 16, "LDIR final iteration should be 16 T-states");
    assert_eq!(bus.memory[0x2002], 0xCC);
    assert_eq!(cpu.get_bc(), 0x0000);
    assert_eq!(cpu.f & 0x04, 0, "PV should be clear after LDIR completes");
}

// ============================================================
// LDDR
// ============================================================

#[test]
fn test_lddr() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.h = 0x10; cpu.l = 0x02;
    cpu.d = 0x20; cpu.e = 0x02;
    cpu.b = 0x00; cpu.c = 0x03;
    bus.load(0, &[0xED, 0xB8]); // LDDR
    bus.memory[0x1000] = 0x11;
    bus.memory[0x1001] = 0x22;
    bus.memory[0x1002] = 0x33;

    run_instruction(&mut cpu, &mut bus); // Transfer [0x1002] → [0x2002]
    assert_eq!(bus.memory[0x2002], 0x33);
    assert_eq!(cpu.get_hl(), 0x1001);
    assert_eq!(cpu.get_de(), 0x2001);

    run_instruction(&mut cpu, &mut bus); // Transfer [0x1001] → [0x2001]
    assert_eq!(bus.memory[0x2001], 0x22);

    run_instruction(&mut cpu, &mut bus); // Transfer [0x1000] → [0x2000]
    assert_eq!(bus.memory[0x2000], 0x11);
    assert_eq!(cpu.get_bc(), 0x0000);
}

// ============================================================
// CPI
// ============================================================

#[test]
fn test_cpi_match() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x03;
    cpu.f = 0x01; // C set
    bus.load(0, &[0xED, 0xA1]); // CPI
    bus.memory[0x1000] = 0x42; // Match

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "CPI should be 16 T-states");
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (match)");
    assert_ne!(cpu.f & 0x02, 0, "N should be set");
    assert_ne!(cpu.f & 0x01, 0, "C should be preserved");
    assert_ne!(cpu.f & 0x04, 0, "PV should be set (BC != 0)");
    assert_eq!(cpu.get_hl(), 0x1001, "HL should be incremented");
    assert_eq!(cpu.get_bc(), 0x0002);
    assert_eq!(cpu.a, 0x42, "A should be unchanged");
}

#[test]
fn test_cpi_no_match() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x01;
    bus.load(0, &[0xED, 0xA1]);
    bus.memory[0x1000] = 0x43; // No match

    run_instruction(&mut cpu, &mut bus);
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear (no match)");
    assert_eq!(cpu.f & 0x04, 0, "PV should be clear (BC == 0)");
}

// ============================================================
// CPD
// ============================================================

#[test]
fn test_cpd() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.h = 0x10; cpu.l = 0x05;
    cpu.b = 0x00; cpu.c = 0x03;
    bus.load(0, &[0xED, 0xA9]); // CPD
    bus.memory[0x1005] = 0x42; // Match

    run_instruction(&mut cpu, &mut bus);
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (match)");
    assert_eq!(cpu.get_hl(), 0x1004, "HL should be decremented");
    assert_eq!(cpu.get_bc(), 0x0002);
}

// ============================================================
// CPIR
// ============================================================

#[test]
fn test_cpir_find() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.h = 0x10; cpu.l = 0x00;
    cpu.b = 0x00; cpu.c = 0x05;
    bus.load(0, &[0xED, 0xB1]); // CPIR
    bus.memory[0x1000] = 0x00;
    bus.memory[0x1001] = 0x00;
    bus.memory[0x1002] = 0x42; // Match at [0x1002]

    let cycles1 = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles1, 21, "CPIR repeating should be 21 T-states");
    assert_eq!(cpu.f & 0x40, 0, "Z clear (no match yet)");

    run_instruction(&mut cpu, &mut bus); // Skip [0x1001]

    let cycles3 = run_instruction(&mut cpu, &mut bus); // Match at [0x1002]
    assert_eq!(cycles3, 16, "CPIR match should be 16 T-states");
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (match found)");
    assert_eq!(cpu.get_hl(), 0x1003);
}

// ============================================================
// CPDR
// ============================================================

#[test]
fn test_cpdr_find() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.a = 0x42;
    cpu.h = 0x10; cpu.l = 0x02;
    cpu.b = 0x00; cpu.c = 0x05;
    bus.load(0, &[0xED, 0xB9]); // CPDR
    bus.memory[0x1002] = 0x00;
    bus.memory[0x1001] = 0x42; // Match at [0x1001]

    run_instruction(&mut cpu, &mut bus); // Skip [0x1002]
    assert_eq!(cpu.f & 0x40, 0, "Z clear (no match)");

    run_instruction(&mut cpu, &mut bus); // Match at [0x1001]
    assert_ne!(cpu.f & 0x40, 0, "Z should be set");
    assert_eq!(cpu.get_hl(), 0x1000);
}

// ============================================================
// INI / OUTI (timing)
// ============================================================

#[test]
fn test_ini() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x03;
    cpu.c = 0x10;
    cpu.h = 0x20; cpu.l = 0x00;
    bus.load(0, &[0xED, 0xA2]); // INI

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "INI should be 16 T-states");
    assert_eq!(cpu.b, 0x02, "B should be decremented");
    assert_eq!(bus.memory[0x2000], 0x00, "I/O read mapped to memory (port 0x0210)");
    assert_eq!(cpu.get_hl(), 0x2001, "HL should be incremented");
    assert_eq!(cpu.f & 0x40, 0, "Z should be clear (B != 0)");
}

#[test]
fn test_outi() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    cpu.b = 0x01;
    cpu.c = 0x10;
    cpu.h = 0x20; cpu.l = 0x00;
    bus.load(0, &[0xED, 0xA3]); // OUTI
    bus.memory[0x2000] = 0x42;

    let cycles = run_instruction(&mut cpu, &mut bus);
    assert_eq!(cycles, 16, "OUTI should be 16 T-states");
    assert_eq!(cpu.b, 0x00, "B should be decremented");
    assert_eq!(cpu.get_hl(), 0x2001, "HL should be incremented");
    assert_ne!(cpu.f & 0x40, 0, "Z should be set (B == 0)");
}
