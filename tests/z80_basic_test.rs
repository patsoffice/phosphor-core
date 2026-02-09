use phosphor_core::machine::simplez80::SimpleZ80System;

#[test]
fn test_ld_a_n() {
    let mut sys = SimpleZ80System::new();
    // LD A, 0x42 (0x3E 0x42)
    sys.load_program(0, &[0x3E, 0x42]);

    // Cycle 0: Fetch opcode 0x3E
    sys.tick();
    // Cycle 1: Fetch operand 0x42, execute
    sys.tick();

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x42);
    assert_eq!(state.pc, 2);
}
