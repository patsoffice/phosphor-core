use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_bra_forward() {
    let mut sys = Simple6809System::new();
    // 0x00: BRA $02 (skip next 2 bytes)
    // 0x02: NOP (0x12) - skipped
    // 0x03: NOP (0x12) - skipped
    // 0x04: LDA #$42
    sys.load_rom(0, &[0x20, 0x02, 0x12, 0x12, 0x86, 0x42]);

    // BRA (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    assert_eq!(sys.get_cpu_state().pc, 0x04);

    // LDA (2 cycles)
    sys.tick();
    sys.tick();
    assert_eq!(sys.get_cpu_state().a, 0x42);
}

#[test]
fn test_bra_backward() {
    let mut sys = Simple6809System::new();
    // 0x00: BRA $00 (infinite loop to self)
    sys.load_rom(0, &[0x20, 0xFE]); // 0xFE is -2

    // Execute BRA
    sys.tick();
    sys.tick();
    sys.tick();
    // PC should be back at 0x00 (0x02 + (-2) = 0x00)
    assert_eq!(sys.get_cpu_state().pc, 0x00);
}

#[test]
fn test_beq_taken() {
    let mut sys = Simple6809System::new();
    // LDA #$00 (sets Z), BEQ $02
    sys.load_rom(0, &[0x86, 0x00, 0x27, 0x02, 0x12, 0x12, 0x86, 0x42]);

    sys.tick();
    sys.tick(); // LDA
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    sys.tick();
    sys.tick();
    sys.tick(); // BEQ
    assert_eq!(sys.get_cpu_state().pc, 0x06); // 0x04 + 2 = 0x06
}

#[test]
fn test_beq_not_taken() {
    let mut sys = Simple6809System::new();
    // LDA #$01 (clears Z), BEQ $02
    sys.load_rom(0, &[0x86, 0x01, 0x27, 0x02, 0x86, 0x42]);

    sys.tick();
    sys.tick(); // LDA
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), 0);

    sys.tick();
    sys.tick();
    sys.tick(); // BEQ (not taken)
    assert_eq!(sys.get_cpu_state().pc, 0x04); // 0x04 + 0 (not taken) -> 0x04

    sys.tick();
    sys.tick(); // Next instruction (LDA #$42)
    assert_eq!(sys.get_cpu_state().a, 0x42);
}

#[test]
fn test_bne_taken() {
    let mut sys = Simple6809System::new();
    // LDA #$01 (clears Z), BNE $02
    sys.load_rom(0, &[0x86, 0x01, 0x26, 0x02, 0x12, 0x12, 0x86, 0x42]);

    sys.tick();
    sys.tick(); // LDA
    sys.tick();
    sys.tick();
    sys.tick(); // BNE
    assert_eq!(sys.get_cpu_state().pc, 0x06);
}

#[test]
fn test_bmi_taken() {
    let mut sys = Simple6809System::new();
    // LDA #$80 (sets N), BMI $02
    sys.load_rom(0, &[0x86, 0x80, 0x2B, 0x02, 0x12, 0x12, 0x86, 0x42]);

    sys.tick();
    sys.tick(); // LDA
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::N as u8), CcFlag::N as u8);

    sys.tick();
    sys.tick();
    sys.tick(); // BMI
    assert_eq!(sys.get_cpu_state().pc, 0x06);
}

#[test]
fn test_brn_never() {
    let mut sys = Simple6809System::new();
    // BRN $02 (should not branch)
    sys.load_rom(0, &[0x21, 0x02, 0x86, 0x42]);

    sys.tick();
    sys.tick();
    sys.tick(); // BRN
    assert_eq!(sys.get_cpu_state().pc, 0x02); // 0x02 (next instruction)

    sys.tick();
    sys.tick(); // LDA #$42
    assert_eq!(sys.get_cpu_state().a, 0x42);
}
