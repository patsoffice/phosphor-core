use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6800::{CcFlag, M6800};
mod common;
use common::TestBus;

#[test]
fn test_nop() {
    let mut cpu = M6800::new();
    let mut bus = TestBus::new();
    bus.load(0, &[0x01]); // NOP
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // fetch
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // execute (internal)
    assert_eq!(cpu.pc, 1);
}
