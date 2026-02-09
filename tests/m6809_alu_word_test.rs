use phosphor_core::cpu::m6809::CcFlag;
use phosphor_core::machine::simple6809::Simple6809System;

#[test]
fn test_addd_immediate() {
    let mut sys = Simple6809System::new();
    // LDD #$1000, ADDD #$0123
    sys.load_rom(0, &[0xCC, 0x10, 0x00, 0xC3, 0x01, 0x23]);

    // LDD (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    assert_eq!(sys.get_cpu_state().a, 0x10);
    assert_eq!(sys.get_cpu_state().b, 0x00);

    // ADDD (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x11, "A should be high byte of 0x1123");
    assert_eq!(state.b, 0x23, "B should be low byte of 0x1123");
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_subd_immediate() {
    let mut sys = Simple6809System::new();
    // LDD #$1000, SUBD #$0001
    sys.load_rom(0, &[0xCC, 0x10, 0x00, 0x83, 0x00, 0x01]);

    // LDD (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();

    // SUBD (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    // 0x1000 - 0x0001 = 0x0FFF
    assert_eq!(state.a, 0x0F, "A should be high byte of 0x0FFF");
    assert_eq!(state.b, 0xFF, "B should be low byte of 0x0FFF");
    assert_eq!(state.cc & (CcFlag::N as u8), 0);
    assert_eq!(state.cc & (CcFlag::Z as u8), 0);
    assert_eq!(state.cc & (CcFlag::V as u8), 0);
    assert_eq!(state.cc & (CcFlag::C as u8), 0);
}

#[test]
fn test_cmpx_immediate() {
    let mut sys = Simple6809System::new();
    // LDX #$1000, CMPX #$1000, CMPX #$2000
    sys.load_rom(0, &[0x8E, 0x10, 0x00, 0x8C, 0x10, 0x00, 0x8C, 0x20, 0x00]);

    // LDX (3 cycles)
    sys.tick();
    sys.tick();
    sys.tick();
    assert_eq!(sys.get_cpu_state().x, 0x1000);

    // CMPX #$1000 (4 cycles) -> Z=1
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), CcFlag::Z as u8);

    // CMPX #$2000 (4 cycles) -> N=1, C=1
    sys.tick();
    sys.tick();
    sys.tick();
    sys.tick();
    let state = sys.get_cpu_state();
    assert_eq!(state.cc & (CcFlag::N as u8), CcFlag::N as u8);
    assert_eq!(state.cc & (CcFlag::C as u8), CcFlag::C as u8);
}

#[test]
fn test_addd_extended() {
    let mut sys = Simple6809System::new();
    // LDD #$1000, ADDD $2000
    sys.load_rom(0, &[0xCC, 0x10, 0x00, 0xF3, 0x20, 0x00]);
    sys.write_ram(0x2000, 0x01);
    sys.write_ram(0x2001, 0x23);

    // LDD (3 cycles) + ADDD (5 cycles) = 8 cycles
    for _ in 0..8 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x11);
    assert_eq!(state.b, 0x23);
}

#[test]
fn test_subd_extended() {
    let mut sys = Simple6809System::new();
    // LDD #$1000, SUBD $3000
    sys.load_rom(0, &[0xCC, 0x10, 0x00, 0xB3, 0x30, 0x00]);
    sys.write_ram(0x3000, 0x00);
    sys.write_ram(0x3001, 0x01);

    // LDD (3 cycles) + SUBD (5 cycles) = 8 cycles
    for _ in 0..8 {
        sys.tick();
    }

    let state = sys.get_cpu_state();
    assert_eq!(state.a, 0x0F);
    assert_eq!(state.b, 0xFF);
}

#[test]
fn test_cmpx_extended() {
    let mut sys = Simple6809System::new();
    // LDX #$5000, CMPX $4000
    sys.load_rom(0, &[0x8E, 0x50, 0x00, 0xBC, 0x40, 0x00]);
    sys.write_ram(0x4000, 0x50);
    sys.write_ram(0x4001, 0x00);

    // LDX (3 cycles) + CMPX (5 cycles) = 8 cycles
    for _ in 0..8 {
        sys.tick();
    }

    assert_eq!(sys.get_cpu_state().cc & (CcFlag::Z as u8), CcFlag::Z as u8);
}
