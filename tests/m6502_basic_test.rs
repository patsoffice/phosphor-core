use phosphor_core::cpu::m6502::StatusFlag;
use phosphor_core::machine::simple6502::Simple6502System;

#[test]
fn test_lda_immediate() {
    let mut sys = Simple6502System::new();
    // LDA #$42
    sys.load_program(0, &[0xA9, 0x42]);

    // Cycle 0: Fetch opcode 0xA9
    sys.tick();
    // Cycle 1: Fetch operand 0x42, execute
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x42);
    assert_eq!(state.pc, 2);
    assert_eq!(state.p & (StatusFlag::Z as u8), 0);
    assert_eq!(state.p & (StatusFlag::N as u8), 0);
}
