use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::z80::Z80;
mod common;
use common::TestBus;

#[test]
fn test_ld_a_n() {
    let mut cpu = Z80::new();
    let mut bus = TestBus::new();
    // LD A, 0x42 (0x3E 0x42)
    bus.load(0, &[0x3E, 0x42]);

    // Cycle 0: Fetch opcode 0x3E
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // Cycle 1: Fetch operand 0x42, execute
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
}
