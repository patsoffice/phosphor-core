use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6502::{M6502, StatusFlag};
mod common;
use common::TestBus;

/// Helper: tick the CPU for `n` cycles
fn tick(cpu: &mut M6502, bus: &mut TestBus, n: usize) {
    for _ in 0..n {
        cpu.tick_with_bus(bus, BusMaster::Cpu(0));
    }
}

// =============================================================================
// IRQ
// =============================================================================

#[test]
fn test_irq_triggers_when_i_clear() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Clear I flag to enable IRQ
    // Set up IRQ vector
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80; // Vector = $8000
    // Put a NOP at $0000 so the CPU has an instruction to complete
    bus.load(0, &[0xEA, 0xEA]); // NOP; NOP
    // Execute NOP (2 cycles), then IRQ triggers at next Fetch
    tick(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc, 0x01); // Past the first NOP, about to fetch second
    // Now signal IRQ
    bus.irq = true;
    // The interrupt detection takes 1 cycle (Fetch), then 6 cycles for handler
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x8000);
}

#[test]
fn test_irq_masked_when_i_set() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::I as u8; // I flag set (default)
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA, 0xEA, 0xEA]); // NOP; NOP; NOP
    bus.irq = true;
    // Execute 3 NOPs (6 cycles) — IRQ should NOT fire
    tick(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.pc, 0x03); // Executed all 3 NOPs normally
}

#[test]
fn test_irq_sets_i_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Enable IRQ
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA]); // NOP
    tick(&mut cpu, &mut bus, 2); // Execute NOP
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7); // Interrupt sequence
    assert_eq!(cpu.p & (StatusFlag::I as u8), StatusFlag::I as u8);
}

#[test]
fn test_irq_pushes_p_with_b_clear() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = StatusFlag::U as u8; // Only U set, I clear to enable IRQ
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA]); // NOP
    tick(&mut cpu, &mut bus, 2); // Execute NOP
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7);
    // P was pushed to stack: B should be 0 (hardware interrupt, not BRK)
    let pushed_p = bus.memory[0x01FB]; // 3rd push (after PCH, PCL)
    assert_eq!(
        pushed_p & (StatusFlag::B as u8),
        0,
        "Hardware IRQ should push P with B=0"
    );
    assert_eq!(
        pushed_p & (StatusFlag::U as u8),
        StatusFlag::U as u8,
        "U should always be set in pushed P"
    );
}

#[test]
fn test_irq_pushes_correct_return_address() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Enable IRQ
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA, 0xEA]); // NOP; NOP
    tick(&mut cpu, &mut bus, 2); // Execute first NOP, PC = $0001
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7);
    // Return address should be $0001 (the PC at the instruction boundary)
    let ret_hi = bus.memory[0x01FD];
    let ret_lo = bus.memory[0x01FC];
    assert_eq!(
        u16::from_le_bytes([ret_lo, ret_hi]),
        0x0001,
        "IRQ should push PC pointing to next instruction"
    );
}

#[test]
fn test_irq_sp_decremented_by_3() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8);
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA]);
    let sp_before = cpu.sp;
    tick(&mut cpu, &mut bus, 2);
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.sp, sp_before.wrapping_sub(3));
}

// =============================================================================
// NMI
// =============================================================================

#[test]
fn test_nmi_triggers_on_edge() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    // NMI vector
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90; // Vector = $9000
    bus.load(0, &[0xEA]); // NOP
    tick(&mut cpu, &mut bus, 2); // Execute NOP
    // Signal NMI (rising edge)
    bus.nmi = true;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x9000);
}

#[test]
fn test_nmi_not_masked_by_i_flag() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p |= StatusFlag::I as u8; // I flag set — should NOT block NMI
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90;
    bus.load(0, &[0xEA]);
    tick(&mut cpu, &mut bus, 2);
    bus.nmi = true;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x9000, "NMI should not be masked by I flag");
}

#[test]
fn test_nmi_edge_not_retriggered_while_held() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90;
    bus.load(0, &[0xEA, 0xEA, 0xEA]); // NOP; NOP; NOP
    // Signal NMI and execute first one
    tick(&mut cpu, &mut bus, 2); // NOP
    bus.nmi = true;
    tick(&mut cpu, &mut bus, 7); // NMI fires
    assert_eq!(cpu.pc, 0x9000);
    // NMI line still held high — put NOPs at handler and keep executing
    bus.memory[0x9000] = 0xEA; // NOP at handler
    bus.memory[0x9001] = 0xEA; // NOP
    tick(&mut cpu, &mut bus, 2); // Execute NOP at $9000
    // NMI should NOT fire again (line hasn't gone low then high)
    tick(&mut cpu, &mut bus, 2); // Execute NOP at $9001
    assert_eq!(cpu.pc, 0x9002, "NMI should not retrigger while held high");
}

#[test]
fn test_nmi_retriggers_after_release_and_reassert() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90;
    bus.load(0, &[0xEA, 0xEA]);
    // First NMI
    tick(&mut cpu, &mut bus, 2);
    bus.nmi = true;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x9000);
    // Release NMI line
    bus.nmi = false;
    bus.memory[0x9000] = 0xEA;
    tick(&mut cpu, &mut bus, 2); // Execute NOP at handler
    // Reassert NMI — new rising edge
    bus.nmi = true;
    // NMI fires again at next instruction boundary
    bus.memory[0x9001] = 0xEA;
    tick(&mut cpu, &mut bus, 7); // Second NMI
    assert_eq!(
        cpu.pc, 0x9000,
        "NMI should retrigger after release+reassert"
    );
}

#[test]
fn test_nmi_pushes_p_with_b_clear() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p = StatusFlag::U as u8 | StatusFlag::I as u8;
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90;
    bus.load(0, &[0xEA]);
    tick(&mut cpu, &mut bus, 2);
    bus.nmi = true;
    tick(&mut cpu, &mut bus, 7);
    let pushed_p = bus.memory[0x01FB];
    assert_eq!(
        pushed_p & (StatusFlag::B as u8),
        0,
        "NMI should push P with B=0"
    );
}

// =============================================================================
// NMI priority over IRQ
// =============================================================================

#[test]
fn test_nmi_has_priority_over_irq() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Enable IRQ too
    bus.memory[0xFFFA] = 0x00;
    bus.memory[0xFFFB] = 0x90; // NMI vector = $9000
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80; // IRQ vector = $8000
    bus.load(0, &[0xEA]);
    tick(&mut cpu, &mut bus, 2);
    // Both signals active simultaneously
    bus.nmi = true;
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x9000, "NMI should take priority over IRQ");
}

// =============================================================================
// BRK vs IRQ (B flag distinction)
// =============================================================================

#[test]
fn test_brk_pushes_b_set_irq_pushes_b_clear() {
    // Verify the B flag distinguishes BRK from hardware IRQ
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();

    // First: BRK
    cpu.p = StatusFlag::U as u8; // I clear
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0x00, 0xEA]); // BRK; padding
    tick(&mut cpu, &mut bus, 7);
    let brk_p = bus.memory[0x01FB];
    assert_eq!(
        brk_p & (StatusFlag::B as u8),
        StatusFlag::B as u8,
        "BRK should push B=1"
    );

    // Second: IRQ (fresh CPU)
    let mut cpu2 = M6502::new();
    let mut bus2 = TestBus::new();
    cpu2.p = StatusFlag::U as u8; // I clear
    bus2.memory[0xFFFE] = 0x00;
    bus2.memory[0xFFFF] = 0x80;
    bus2.load(0, &[0xEA]); // NOP
    tick(&mut cpu2, &mut bus2, 2);
    bus2.irq = true;
    tick(&mut cpu2, &mut bus2, 7);
    let irq_p = bus2.memory[0x01FB];
    assert_eq!(irq_p & (StatusFlag::B as u8), 0, "IRQ should push B=0");
}

// =============================================================================
// IRQ/RTI round trip
// =============================================================================

#[test]
fn test_irq_rti_round_trip() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8); // Enable IRQ
    let p_before = cpu.p;
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80; // IRQ vector = $8000
    bus.memory[0x8000] = 0x40; // RTI at handler
    bus.load(0, &[0xEA, 0xEA]); // NOP; NOP
    tick(&mut cpu, &mut bus, 2); // Execute first NOP, PC=$0001
    bus.irq = true;
    tick(&mut cpu, &mut bus, 7); // IRQ fires
    assert_eq!(cpu.pc, 0x8000);
    bus.irq = false; // Clear IRQ before RTI restores I=0
    tick(&mut cpu, &mut bus, 6); // RTI
    assert_eq!(cpu.pc, 0x0001); // Returns to instruction after the NOP
    // P restored (I should be back to original)
    assert_eq!(
        cpu.p & 0xCF,
        p_before & 0xCF,
        "RTI should restore original flags"
    );
}

// =============================================================================
// Interrupt timing
// =============================================================================

#[test]
fn test_irq_takes_7_cycles() {
    let mut cpu = M6502::new();
    let mut bus = TestBus::new();
    cpu.p &= !(StatusFlag::I as u8);
    bus.memory[0xFFFE] = 0x00;
    bus.memory[0xFFFF] = 0x80;
    bus.load(0, &[0xEA]); // NOP
    tick(&mut cpu, &mut bus, 2); // Execute NOP
    bus.irq = true;
    // After exactly 7 cycles, should be at handler
    tick(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.pc, 0x8000);
    // After only 6, should NOT yet be at handler
    let mut cpu2 = M6502::new();
    let mut bus2 = TestBus::new();
    cpu2.p &= !(StatusFlag::I as u8);
    bus2.memory[0xFFFE] = 0x00;
    bus2.memory[0xFFFF] = 0x80;
    bus2.load(0, &[0xEA]);
    tick(&mut cpu2, &mut bus2, 2);
    bus2.irq = true;
    tick(&mut cpu2, &mut bus2, 6);
    assert_ne!(cpu2.pc, 0x8000, "IRQ should not complete in only 6 cycles");
}
