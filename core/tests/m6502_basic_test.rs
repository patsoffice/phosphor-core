use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6502::{M6502, StatusFlag};
mod common;
use common::TestBus;

#[test]
fn test_lda_immediate() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // LDA #$42
    bus.load(0, &[0xA9, 0x42]);

    // Cycle 0: Fetch opcode 0xA9
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    // Cycle 1: Fetch operand 0x42, execute
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.pc, 2);
    assert_eq!(cpu.p & (StatusFlag::Z as u8), 0);
    assert_eq!(cpu.p & (StatusFlag::N as u8), 0);
}
