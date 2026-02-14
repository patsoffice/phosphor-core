use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::{CcFlag, M6809};
mod common;
use common::TestBus;

fn tick(cpu: &mut M6809, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// ===== SWI (0x3F) =====

#[test]
fn test_swi_pushes_all_registers() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Set up registers with known values
    cpu.a = 0x11;
    cpu.b = 0x22;
    cpu.dp = 0x33;
    cpu.x = 0x4455;
    cpu.y = 0x6677;
    cpu.u = 0x8899;
    cpu.s = 0x0100; // Stack at 0x0100
    cpu.pc = 0x0000;
    cpu.cc = CcFlag::N as u8; // Some flags set

    // SWI vector at 0xFFFA/0xFFFB
    bus.memory[0xFFFA] = 0x20;
    bus.memory[0xFFFB] = 0x00;

    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19); // 1 fetch + 2 internal + 12 push + 1 internal + 2 vector + 1 internal

    // S should be decremented by 12 (all registers)
    assert_eq!(cpu.s, 0x0100 - 12);

    // Verify stack contents (low address = top of stack):
    // CC, A, B, DP, X_hi, X_lo, Y_hi, Y_lo, U_hi, U_lo, PC_hi, PC_lo
    let s = 0x0100 - 12;
    assert_eq!(bus.memory[s], CcFlag::N as u8 | CcFlag::E as u8); // CC with E set
    assert_eq!(bus.memory[s + 1], 0x11); // A
    assert_eq!(bus.memory[s + 2], 0x22); // B
    assert_eq!(bus.memory[s + 3], 0x33); // DP
    assert_eq!(bus.memory[s + 4], 0x44); // X high
    assert_eq!(bus.memory[s + 5], 0x55); // X low
    assert_eq!(bus.memory[s + 6], 0x66); // Y high
    assert_eq!(bus.memory[s + 7], 0x77); // Y low
    assert_eq!(bus.memory[s + 8], 0x88); // U high
    assert_eq!(bus.memory[s + 9], 0x99); // U low
    assert_eq!(bus.memory[s + 10], 0x00); // PC high (was 0x0001 after fetch)
    assert_eq!(bus.memory[s + 11], 0x01); // PC low

    // PC should be loaded from vector
    assert_eq!(cpu.pc, 0x2000);

    // E flag should be set, I and F should be set
    assert_ne!(cpu.cc & (CcFlag::E as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::I as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::F as u8), 0);
}

#[test]
fn test_swi_masks_interrupts() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    cpu.cc = 0; // No flags set initially

    bus.memory[0xFFFA] = 0x10;
    bus.memory[0xFFFB] = 0x00;
    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19);

    // I and F should be set after SWI
    assert_ne!(cpu.cc & (CcFlag::I as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::F as u8), 0);

    // CC on stack should NOT have I/F set (original CC had them clear)
    let s = cpu.s as usize;
    let stacked_cc = bus.memory[s];
    assert_eq!(stacked_cc & (CcFlag::I as u8), 0);
    assert_eq!(stacked_cc & (CcFlag::F as u8), 0);
    // But E should be set on stack
    assert_ne!(stacked_cc & (CcFlag::E as u8), 0);
}

#[test]
fn test_swi_preserves_existing_interrupt_mask() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    // I already set before SWI
    cpu.cc = CcFlag::I as u8;

    bus.memory[0xFFFA] = 0x10;
    bus.memory[0xFFFB] = 0x00;
    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19);

    // CC on stack should have I set (it was already set) plus E
    let s = cpu.s as usize;
    let stacked_cc = bus.memory[s];
    assert_ne!(stacked_cc & (CcFlag::I as u8), 0);
    assert_ne!(stacked_cc & (CcFlag::E as u8), 0);
}

// ===== SWI2 (0x103F) =====

#[test]
fn test_swi2_pushes_all_registers() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.a = 0xAA;
    cpu.b = 0xBB;
    cpu.dp = 0xCC;
    cpu.x = 0x1234;
    cpu.y = 0x5678;
    cpu.u = 0x9ABC;
    cpu.s = 0x0200;
    cpu.cc = 0;

    // SWI2 vector at 0xFFF4/0xFFF5
    bus.memory[0xFFF4] = 0x30;
    bus.memory[0xFFF5] = 0x00;

    bus.load(0, &[0x10, 0x3F]); // SWI2

    tick(&mut cpu, &mut bus, 20); // 2 prefix + 18 execute

    assert_eq!(cpu.s, 0x0200 - 12);
    assert_eq!(cpu.pc, 0x3000);

    // Verify stack contents
    let s = cpu.s as usize;
    assert_ne!(bus.memory[s] & (CcFlag::E as u8), 0); // CC with E
    assert_eq!(bus.memory[s + 1], 0xAA); // A
    assert_eq!(bus.memory[s + 2], 0xBB); // B
    assert_eq!(bus.memory[s + 3], 0xCC); // DP
    assert_eq!(bus.memory[s + 4], 0x12); // X high
    assert_eq!(bus.memory[s + 5], 0x34); // X low
    assert_eq!(bus.memory[s + 6], 0x56); // Y high
    assert_eq!(bus.memory[s + 7], 0x78); // Y low
    assert_eq!(bus.memory[s + 8], 0x9A); // U high
    assert_eq!(bus.memory[s + 9], 0xBC); // U low
}

#[test]
fn test_swi2_does_not_mask_interrupts() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0200;
    cpu.cc = 0; // No flags

    bus.memory[0xFFF4] = 0x30;
    bus.memory[0xFFF5] = 0x00;
    bus.load(0, &[0x10, 0x3F]); // SWI2

    tick(&mut cpu, &mut bus, 20);

    // I and F should NOT be set (SWI2 doesn't mask)
    assert_eq!(cpu.cc & (CcFlag::I as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::F as u8), 0);
    // But E should be set
    assert_ne!(cpu.cc & (CcFlag::E as u8), 0);
}

// ===== SWI3 (0x113F) =====

#[test]
fn test_swi3_pushes_all_registers() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.a = 0x11;
    cpu.b = 0x22;
    cpu.dp = 0x33;
    cpu.x = 0x4455;
    cpu.y = 0x6677;
    cpu.u = 0x8899;
    cpu.s = 0x0300;
    cpu.cc = 0;

    // SWI3 vector at 0xFFF2/0xFFF3
    bus.memory[0xFFF2] = 0x40;
    bus.memory[0xFFF3] = 0x00;

    bus.load(0, &[0x11, 0x3F]); // SWI3

    tick(&mut cpu, &mut bus, 20); // 2 prefix + 18 execute

    assert_eq!(cpu.s, 0x0300 - 12);
    assert_eq!(cpu.pc, 0x4000);

    // Verify stack
    let s = cpu.s as usize;
    assert_ne!(bus.memory[s] & (CcFlag::E as u8), 0); // CC with E
    assert_eq!(bus.memory[s + 1], 0x11); // A
    assert_eq!(bus.memory[s + 2], 0x22); // B
}

#[test]
fn test_swi3_does_not_mask_interrupts() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0300;
    cpu.cc = 0;

    bus.memory[0xFFF2] = 0x40;
    bus.memory[0xFFF3] = 0x00;
    bus.load(0, &[0x11, 0x3F]); // SWI3

    tick(&mut cpu, &mut bus, 20);

    // I and F should NOT be set
    assert_eq!(cpu.cc & (CcFlag::I as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::F as u8), 0);
}

#[test]
fn test_swi3_uses_correct_vector() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0300;
    cpu.cc = 0;

    bus.memory[0xFFF2] = 0xAB;
    bus.memory[0xFFF3] = 0xCD;
    bus.load(0, &[0x11, 0x3F]); // SWI3

    tick(&mut cpu, &mut bus, 20);

    assert_eq!(cpu.pc, 0xABCD);
}

// ===== RTI (0x3B) =====

#[test]
fn test_rti_full_restore_e_set() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Set up stack with all registers (as SWI would have pushed them)
    // Stack layout: CC, A, B, DP, X_hi, X_lo, Y_hi, Y_lo, U_hi, U_lo, PC_hi, PC_lo
    let s: usize = 0x00F4; // S after 12 bytes pushed from 0x0100
    cpu.s = s as u16;

    let cc_on_stack = CcFlag::E as u8 | CcFlag::N as u8; // E=1 → full restore
    bus.memory[s] = cc_on_stack;
    bus.memory[s + 1] = 0x11; // A
    bus.memory[s + 2] = 0x22; // B
    bus.memory[s + 3] = 0x33; // DP
    bus.memory[s + 4] = 0x44; // X high
    bus.memory[s + 5] = 0x55; // X low
    bus.memory[s + 6] = 0x66; // Y high
    bus.memory[s + 7] = 0x77; // Y low
    bus.memory[s + 8] = 0x88; // U high
    bus.memory[s + 9] = 0x99; // U low
    bus.memory[s + 10] = 0x10; // PC high
    bus.memory[s + 11] = 0x00; // PC low

    // RTI instruction at current PC (address 0x2000)
    cpu.pc = 0x2000;
    bus.memory[0x2000] = 0x3B; // RTI

    tick(&mut cpu, &mut bus, 15); // 1 fetch + 1 internal + 12 pulls + 1 internal

    assert_eq!(cpu.cc, cc_on_stack);
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.b, 0x22);
    assert_eq!(cpu.dp, 0x33);
    assert_eq!(cpu.x, 0x4455);
    assert_eq!(cpu.y, 0x6677);
    assert_eq!(cpu.u, 0x8899);
    assert_eq!(cpu.pc, 0x1000);
    assert_eq!(cpu.s, (s + 12) as u16);
}

#[test]
fn test_rti_fast_return_e_clear() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Set up stack with only CC and PC (E=0 means FIRQ return)
    let s: usize = 0x00FD;
    cpu.s = s as u16;

    let cc_on_stack = CcFlag::Z as u8; // E=0 → fast return, Z flag set
    bus.memory[s] = cc_on_stack;
    bus.memory[s + 1] = 0x50; // PC high
    bus.memory[s + 2] = 0x00; // PC low

    // Set regs to something — they should NOT be overwritten
    cpu.a = 0xAA;
    cpu.b = 0xBB;
    cpu.dp = 0xDD;
    cpu.x = 0x1111;
    cpu.y = 0x2222;
    cpu.u = 0x3333;

    cpu.pc = 0x2000;
    bus.memory[0x2000] = 0x3B; // RTI

    tick(&mut cpu, &mut bus, 6); // 1 fetch + 1 internal + 1 pull CC + 1 internal + 2 pull PC

    assert_eq!(cpu.cc, cc_on_stack);
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.s, (s + 3) as u16);

    // Other registers should be unchanged
    assert_eq!(cpu.a, 0xAA);
    assert_eq!(cpu.b, 0xBB);
    assert_eq!(cpu.dp, 0xDD);
    assert_eq!(cpu.x, 0x1111);
    assert_eq!(cpu.y, 0x2222);
    assert_eq!(cpu.u, 0x3333);
}

#[test]
fn test_rti_restores_flags() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    let s: usize = 0x00FD;
    cpu.s = s as u16;

    // Stack has CC with specific flags
    let original_cc = CcFlag::C as u8 | CcFlag::V as u8 | CcFlag::H as u8;
    bus.memory[s] = original_cc; // E=0
    bus.memory[s + 1] = 0x00;
    bus.memory[s + 2] = 0x10;

    // CPU currently has different flags
    cpu.cc = CcFlag::N as u8 | CcFlag::Z as u8 | CcFlag::I as u8;

    cpu.pc = 0x2000;
    bus.memory[0x2000] = 0x3B; // RTI

    tick(&mut cpu, &mut bus, 6);

    // CC should be restored from stack
    assert_eq!(cpu.cc, original_cc);
}

// ===== SWI + RTI round-trip =====

#[test]
fn test_swi_then_rti_roundtrip() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Set up registers with known values
    cpu.a = 0x11;
    cpu.b = 0x22;
    cpu.dp = 0x33;
    cpu.x = 0x4455;
    cpu.y = 0x6677;
    cpu.u = 0x8899;
    cpu.s = 0x0100;
    cpu.pc = 0x0000;
    cpu.cc = CcFlag::C as u8 | CcFlag::V as u8;

    // SWI vector points to 0x2000 where RTI is located
    bus.memory[0xFFFA] = 0x20;
    bus.memory[0xFFFB] = 0x00;

    // SWI at 0x0000
    bus.load(0, &[0x3F]);

    // RTI at handler address
    bus.memory[0x2000] = 0x3B;

    // Execute SWI (19 cycles)
    tick(&mut cpu, &mut bus, 19);
    assert_eq!(cpu.pc, 0x2000);

    // Save the post-SWI state
    let swi_cc = cpu.cc; // Has E, I, F set
    assert_ne!(swi_cc & (CcFlag::E as u8), 0);
    assert_ne!(swi_cc & (CcFlag::I as u8), 0);
    assert_ne!(swi_cc & (CcFlag::F as u8), 0);

    // Execute RTI (15 cycles for E=1)
    tick(&mut cpu, &mut bus, 15);

    // All registers should be restored to pre-SWI values
    assert_eq!(cpu.a, 0x11);
    assert_eq!(cpu.b, 0x22);
    assert_eq!(cpu.dp, 0x33);
    assert_eq!(cpu.x, 0x4455);
    assert_eq!(cpu.y, 0x6677);
    assert_eq!(cpu.u, 0x8899);
    assert_eq!(cpu.pc, 0x0001); // PC after the SWI instruction

    // CC should be restored with E set (and original C, V flags)
    // But I and F should be restored to original values (clear)
    assert_ne!(cpu.cc & (CcFlag::C as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::V as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::E as u8), 0);
    assert_eq!(cpu.cc & (CcFlag::I as u8), 0); // Restored from stack (was clear)
    assert_eq!(cpu.cc & (CcFlag::F as u8), 0); // Restored from stack (was clear)

    // S should be back to original
    assert_eq!(cpu.s, 0x0100);
}

#[test]
fn test_swi2_then_rti_roundtrip() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.a = 0xAA;
    cpu.b = 0xBB;
    cpu.dp = 0x10;
    cpu.x = 0x1234;
    cpu.y = 0x5678;
    cpu.u = 0x9ABC;
    cpu.s = 0x0200;
    cpu.pc = 0x0000;
    cpu.cc = CcFlag::Z as u8;

    // SWI2 vector → 0x3000
    bus.memory[0xFFF4] = 0x30;
    bus.memory[0xFFF5] = 0x00;

    bus.load(0, &[0x10, 0x3F]); // SWI2
    bus.memory[0x3000] = 0x3B; // RTI

    // Execute SWI2 (20 cycles)
    tick(&mut cpu, &mut bus, 20);
    assert_eq!(cpu.pc, 0x3000);

    // Execute RTI (15 cycles for E=1)
    tick(&mut cpu, &mut bus, 15);

    assert_eq!(cpu.a, 0xAA);
    assert_eq!(cpu.b, 0xBB);
    assert_eq!(cpu.dp, 0x10);
    assert_eq!(cpu.x, 0x1234);
    assert_eq!(cpu.y, 0x5678);
    assert_eq!(cpu.u, 0x9ABC);
    assert_eq!(cpu.pc, 0x0002); // After 0x10 0x3F
    assert_eq!(cpu.s, 0x0200);
}

#[test]
fn test_swi_vector_address() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    bus.memory[0xFFFA] = 0xDE;
    bus.memory[0xFFFB] = 0xAD;
    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19);

    assert_eq!(cpu.pc, 0xDEAD);
}

#[test]
fn test_swi2_vector_address() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    bus.memory[0xFFF4] = 0xBE;
    bus.memory[0xFFF5] = 0xEF;
    bus.load(0, &[0x10, 0x3F]); // SWI2

    tick(&mut cpu, &mut bus, 20);

    assert_eq!(cpu.pc, 0xBEEF);
}

#[test]
fn test_swi_stack_pointer_decrement() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    bus.memory[0xFFFA] = 0x20;
    bus.memory[0xFFFB] = 0x00;
    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19);

    // S should be decremented by 12 (all registers)
    assert_eq!(cpu.s, 0x0100 - 12);
}

#[test]
fn test_rti_e_set_restores_interrupt_mask() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Set up stack with CC that has I and F set
    let s: usize = 0x00F4;
    cpu.s = s as u16;

    let cc_on_stack = CcFlag::E as u8 | CcFlag::I as u8 | CcFlag::F as u8;
    bus.memory[s] = cc_on_stack;
    bus.memory[s + 1] = 0x00; // A
    bus.memory[s + 2] = 0x00; // B
    bus.memory[s + 3] = 0x00; // DP
    bus.memory[s + 4] = 0x00; // X hi
    bus.memory[s + 5] = 0x00; // X lo
    bus.memory[s + 6] = 0x00; // Y hi
    bus.memory[s + 7] = 0x00; // Y lo
    bus.memory[s + 8] = 0x00; // U hi
    bus.memory[s + 9] = 0x00; // U lo
    bus.memory[s + 10] = 0x10; // PC hi
    bus.memory[s + 11] = 0x00; // PC lo

    cpu.pc = 0x2000;
    cpu.cc = 0; // Currently no flags
    bus.memory[0x2000] = 0x3B; // RTI

    tick(&mut cpu, &mut bus, 15);

    // I and F should be restored from stack
    assert_ne!(cpu.cc & (CcFlag::I as u8), 0);
    assert_ne!(cpu.cc & (CcFlag::F as u8), 0);
}

#[test]
fn test_swi_with_all_flags_set() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    cpu.s = 0x0100;
    cpu.cc = 0xFF; // All flags set
    bus.memory[0xFFFA] = 0x20;
    bus.memory[0xFFFB] = 0x00;
    bus.load(0, &[0x3F]); // SWI

    tick(&mut cpu, &mut bus, 19);

    // CC on stack should have all flags set (including E which was already set)
    let s = cpu.s as usize;
    assert_eq!(bus.memory[s], 0xFF); // All flags were already set
}
