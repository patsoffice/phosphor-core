use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_lbeq_taken() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_cc(CcFlag::Z as u8);
    // LBEQ $0010 (0x10 0x27 0x00 0x10)
    sys.load_rom(0, &[0x10, 0x27, 0x00, 0x10]);

    // 6 cycles (taken: 2 prefix + 4 execute)
    for _ in 0..6 {
        sys.tick();
    }

    // PC = 0x04 (past instruction) + 0x0010 (offset) = 0x0014
    assert_eq!(sys.get_cpu_state().pc, 0x0014, "PC should branch forward");
}

#[test]
fn test_lbeq_not_taken() {
    let mut sys = Simple6809System::new();
    // Z=0 (default), so LBEQ should not be taken
    sys.load_rom(0, &[0x10, 0x27, 0x00, 0x10]);

    // 5 cycles (not taken: 2 prefix + 3 execute)
    for _ in 0..5 {
        sys.tick();
    }

    // PC should be just past the instruction (4 bytes)
    assert_eq!(sys.get_cpu_state().pc, 0x0004, "PC should not branch");
}

#[test]
fn test_lbne_taken() {
    let mut sys = Simple6809System::new();
    // Z=0 (default), so LBNE should be taken
    // LBNE $0020 (0x10 0x26 0x00 0x20)
    sys.load_rom(0, &[0x10, 0x26, 0x00, 0x20]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x0024, "PC should branch forward");
}

#[test]
fn test_lbrn_never() {
    let mut sys = Simple6809System::new();
    // LBRN $1000 — never branches regardless of flags
    sys.load_rom(0, &[0x10, 0x21, 0x10, 0x00]);

    // 5 cycles (never taken)
    for _ in 0..5 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x0004, "LBRN should never branch");
}

#[test]
fn test_lbmi_taken() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_cc(CcFlag::N as u8);
    // LBMI $0008 (0x10 0x2B 0x00 0x08)
    sys.load_rom(0, &[0x10, 0x2B, 0x00, 0x08]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x000C, "PC should branch to 0x04 + 0x08");
}

#[test]
fn test_lbcs_taken() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_cc(CcFlag::C as u8);
    // LBCS $0004 (0x10 0x25 0x00 0x04)
    sys.load_rom(0, &[0x10, 0x25, 0x00, 0x04]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x0008);
}

#[test]
fn test_lbhi_taken() {
    let mut sys = Simple6809System::new();
    // C=0 and Z=0 (default), so LBHI should be taken
    // LBHI $0010 (0x10 0x22 0x00 0x10)
    sys.load_rom(0, &[0x10, 0x22, 0x00, 0x10]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x0014);
}

#[test]
fn test_lbge_taken() {
    let mut sys = Simple6809System::new();
    // N=0, V=0 → N==V → taken
    // LBGE $0010 (0x10 0x2C 0x00 0x10)
    sys.load_rom(0, &[0x10, 0x2C, 0x00, 0x10]);

    for _ in 0..6 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().pc, 0x0014);
}

#[test]
fn test_lbeq_large_forward_offset() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_cc(CcFlag::Z as u8);
    // LBEQ $0200 — offset larger than 8-bit max
    sys.load_rom(0, &[0x10, 0x27, 0x02, 0x00]);

    for _ in 0..6 {
        sys.tick();
    }

    // PC = 0x04 + 0x0200 = 0x0204
    assert_eq!(sys.get_cpu_state().pc, 0x0204, "16-bit offset should work");
}

#[test]
fn test_lbeq_backward_offset() {
    let mut sys = Simple6809System::new();
    sys.set_cpu_cc(CcFlag::Z as u8);
    // Place LBEQ at offset 0x10, branch backward by 0x08
    // 0xFFF8 = -8 as i16
    sys.load_rom(0x10, &[0x10, 0x27, 0xFF, 0xF8]);

    // Start execution at 0x10
    // We need to set PC to 0x10 first — use a JMP or set directly
    // Easiest: fill ROM from 0 with NOPs (we don't have NOP, use BRN as 3-byte NOP)
    // Actually, just load at 0 and pad with known instructions
    // Simpler: load the instruction at address 0 and use a negative offset
    let mut sys2 = Simple6809System::new();
    sys2.set_cpu_cc(CcFlag::Z as u8);
    // LBEQ $FFF0 at address 0 — offset = -16 = 0xFFF0
    // PC after instruction = 0x04, so target = 0x04 + 0xFFF0 = 0xFFF4 (wraps)
    sys2.load_rom(0, &[0x10, 0x27, 0xFF, 0xF0]);

    for _ in 0..6 {
        sys2.tick();
    }

    assert_eq!(sys2.get_cpu_state().pc, 0xFFF4, "Negative offset should wrap PC");
}
