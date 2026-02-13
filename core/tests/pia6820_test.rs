use phosphor_core::device::pia6820::Pia6820;

// ==========================================================================
// Core register tests
// ==========================================================================

#[test]
fn test_new_pia_defaults() {
    let mut pia = Pia6820::new();

    // All data/DDR registers zero
    assert_eq!(pia.read(0), 0x00); // DDRA (ctrl_a.2 = 0 by default)
    assert_eq!(pia.read(1), 0x00); // CRA
    assert_eq!(pia.read(2), 0x00); // DDRB
    assert_eq!(pia.read(3), 0x00); // CRB

    // No interrupts
    assert!(!pia.irq_a());
    assert!(!pia.irq_b());

    // No output
    assert_eq!(pia.read_output_a(), 0x00);
    assert_eq!(pia.read_output_b(), 0x00);
}

#[test]
fn test_ddr_select_via_control_bit2() {
    let mut pia = Pia6820::new();

    // CRA bit 2 = 0 (default): writing offset 0 sets DDRA
    pia.write(0, 0xFF);
    assert_eq!(pia.read(0), 0xFF); // Read DDRA

    // Set CRA bit 2 = 1: now offset 0 accesses data register
    pia.write(1, 0x04);
    pia.write(0, 0x42); // Write ORA
    // Read data port: all outputs (DDR=0xFF), ORA=0x42
    assert_eq!(pia.read(0), 0x42);

    // Switch back to DDR: CRA bit 2 = 0
    pia.write(1, 0x00);
    assert_eq!(pia.read(0), 0xFF); // DDRA still 0xFF
}

#[test]
fn test_port_a_all_output() {
    let mut pia = Pia6820::new();

    // Set DDRA = 0xFF (all output)
    pia.write(0, 0xFF);
    // Switch to data register
    pia.write(1, 0x04);
    // Write ORA
    pia.write(0, 0x42);
    // Read port A: all bits from output register
    assert_eq!(pia.read(0), 0x42);
}

#[test]
fn test_port_a_all_input() {
    let mut pia = Pia6820::new();

    // DDRA = 0x00 (all input, default)
    // Switch to data register
    pia.write(1, 0x04);
    // Set external input pins
    pia.set_port_a_input(0xAB);
    // Read port A: all bits from input pins
    assert_eq!(pia.read(0), 0xAB);
}

#[test]
fn test_port_a_mixed_ddr() {
    let mut pia = Pia6820::new();

    // DDRA = 0xF0 (upper nibble output, lower nibble input)
    pia.write(0, 0xF0);
    // Switch to data register
    pia.write(1, 0x04);
    // Write ORA = 0xA0
    pia.write(0, 0xA0);
    // Set input = 0x0B
    pia.set_port_a_input(0x0B);
    // Read: upper from ORA (0xA0), lower from input (0x0B) = 0xAB
    assert_eq!(pia.read(0), 0xAB);
}

#[test]
fn test_port_b_mirrors_port_a() {
    let mut pia = Pia6820::new();

    // --- All output ---
    pia.write(2, 0xFF); // DDRB = 0xFF
    pia.write(3, 0x04); // CRB bit 2 = 1
    pia.write(2, 0x42); // ORB = 0x42
    assert_eq!(pia.read(2), 0x42);

    // --- All input ---
    let mut pia2 = Pia6820::new();
    pia2.write(3, 0x04); // CRB bit 2 = 1
    pia2.set_port_b_input(0xAB);
    assert_eq!(pia2.read(2), 0xAB);

    // --- Mixed DDR ---
    let mut pia3 = Pia6820::new();
    pia3.write(2, 0xF0); // DDRB = 0xF0
    pia3.write(3, 0x04); // CRB bit 2 = 1
    pia3.write(2, 0xA0); // ORB = 0xA0
    pia3.set_port_b_input(0x0B);
    assert_eq!(pia3.read(2), 0xAB);
}

// ==========================================================================
// Output reading tests
// ==========================================================================

#[test]
fn test_read_output_a() {
    let mut pia = Pia6820::new();

    // Set DDRA = 0xF0 (upper nibble output)
    pia.write(0, 0xF0);
    // Switch to data register and write ORA = 0xFF
    pia.write(1, 0x04);
    pia.write(0, 0xFF);
    // read_output_a = ORA & DDRA = 0xFF & 0xF0 = 0xF0
    assert_eq!(pia.read_output_a(), 0xF0);
}

#[test]
fn test_read_output_b() {
    let mut pia = Pia6820::new();

    // Set DDRB = 0x0F (lower nibble output)
    pia.write(2, 0x0F);
    // Switch to data register and write ORB = 0xFF
    pia.write(3, 0x04);
    pia.write(2, 0xFF);
    // read_output_b = ORB & DDRB = 0xFF & 0x0F = 0x0F
    assert_eq!(pia.read_output_b(), 0x0F);
}

// ==========================================================================
// Interrupt tests — CA1/CB1
// ==========================================================================

#[test]
fn test_ca1_rising_edge_interrupt() {
    let mut pia = Pia6820::new();

    // CRA: bit 1 = 1 (rising edge), bit 0 = 1 (enable IRQ)
    pia.write(1, 0x03);

    // CA1 starts low (default false), transition to high
    pia.set_ca1(true);
    assert!(pia.irq_a());

    // Verify flag visible in CRA bit 7
    assert_eq!(pia.read(1) & 0x80, 0x80);
}

#[test]
fn test_ca1_falling_edge_interrupt() {
    let mut pia = Pia6820::new();

    // CRA: bit 1 = 0 (falling edge), bit 0 = 1 (enable IRQ)
    pia.write(1, 0x01);

    // First bring CA1 high
    pia.set_ca1(true);
    assert!(!pia.irq_a()); // No interrupt on rising when configured for falling

    // Now transition high -> low
    pia.set_ca1(false);
    assert!(pia.irq_a());
}

#[test]
fn test_cb1_rising_edge_interrupt() {
    let mut pia = Pia6820::new();

    // CRB: bit 1 = 1 (rising edge), bit 0 = 1 (enable IRQ)
    pia.write(3, 0x03);

    pia.set_cb1(true);
    assert!(pia.irq_b());

    // Verify flag visible in CRB bit 7
    assert_eq!(pia.read(3) & 0x80, 0x80);
}

#[test]
fn test_cb1_falling_edge_interrupt() {
    let mut pia = Pia6820::new();

    // CRB: bit 1 = 0 (falling edge), bit 0 = 1 (enable IRQ)
    pia.write(3, 0x01);

    pia.set_cb1(true); // Rising — no interrupt
    assert!(!pia.irq_b());

    pia.set_cb1(false); // Falling — interrupt
    assert!(pia.irq_b());
}

// ==========================================================================
// Interrupt tests — CA2/CB2
// ==========================================================================

#[test]
fn test_ca2_input_mode_falling_edge() {
    let mut pia = Pia6820::new();

    // CRA: bit 5 = 0 (CA2 input), bit 4 = 0 (falling edge), bit 3 = 1 (enable)
    pia.write(1, 0x08);

    // Bring CA2 high first
    pia.set_ca2(true);
    assert!(!pia.irq_a()); // Rising shouldn't trigger

    // Falling edge
    pia.set_ca2(false);
    assert!(pia.irq_a());

    // Verify irq_a2 flag in CRA bit 6
    assert_eq!(pia.read(1) & 0x40, 0x40);
}

#[test]
fn test_ca2_input_mode_rising_edge() {
    let mut pia = Pia6820::new();

    // CRA: bit 5 = 0 (CA2 input), bit 4 = 1 (rising edge), bit 3 = 1 (enable)
    pia.write(1, 0x18);

    pia.set_ca2(true); // Rising edge
    assert!(pia.irq_a());
}

#[test]
fn test_ca2_output_mode_no_interrupt() {
    let mut pia = Pia6820::new();

    // CRA: bit 5 = 1 (CA2 output), bit 3 = 1 (enable would be set if input)
    pia.write(1, 0x28);

    pia.set_ca2(true);
    pia.set_ca2(false);
    // No interrupt should fire — CA2 is in output mode
    assert!(!pia.irq_a());

    // Verify no flag set
    assert_eq!(pia.read(1) & 0x40, 0x00);
}

#[test]
fn test_cb2_input_mode_interrupt() {
    let mut pia = Pia6820::new();

    // CRB: bit 5 = 0 (CB2 input), bit 4 = 0 (falling edge), bit 3 = 1 (enable)
    pia.write(3, 0x08);

    // Bring CB2 high, then low (falling edge)
    pia.set_cb2(true);
    assert!(!pia.irq_b());

    pia.set_cb2(false);
    assert!(pia.irq_b());

    // Also test rising edge config
    let mut pia2 = Pia6820::new();
    // CRB: bit 5 = 0, bit 4 = 1 (rising edge), bit 3 = 1 (enable)
    pia2.write(3, 0x18);

    pia2.set_cb2(true);
    assert!(pia2.irq_b());
}

// ==========================================================================
// IRQ flag behavior
// ==========================================================================

#[test]
fn test_irq_flag_cleared_on_data_read() {
    let mut pia = Pia6820::new();

    // Enable CA1 rising edge interrupt
    pia.write(1, 0x07); // bit 2=1 (data select), bit 1=1 (rising), bit 0=1 (enable)

    // Trigger interrupt
    pia.set_ca1(true);
    assert!(pia.irq_a());
    assert_eq!(pia.read(1) & 0x80, 0x80); // Flag visible in CRA

    // Read port A data register — clears flags
    let _ = pia.read(0);
    assert!(!pia.irq_a());
    assert_eq!(pia.read(1) & 0xC0, 0x00); // Both flags cleared
}

#[test]
fn test_irq_disabled_when_enable_bit_clear() {
    let mut pia = Pia6820::new();

    // CRA: bit 1 = 1 (rising edge), bit 0 = 0 (IRQ DISABLED)
    pia.write(1, 0x02);

    pia.set_ca1(true);

    // Flag is set (visible in CRA)...
    assert_eq!(pia.read(1) & 0x80, 0x80);
    // ...but irq_a() output is NOT asserted
    assert!(!pia.irq_a());
}

#[test]
fn test_control_register_read_shows_flags() {
    let mut pia = Pia6820::new();

    // Set up CRA with some control bits
    pia.write(1, 0x03); // bits 1:0 set

    // Trigger CA1 interrupt
    pia.set_ca1(true);

    // Read CRA: should show flag in bit 7, plus control bits
    let cra = pia.read(1);
    assert_eq!(cra & 0x80, 0x80); // irq_a1 flag
    assert_eq!(cra & 0x03, 0x03); // control bits preserved
}

#[test]
fn test_control_register_write_preserves_flags() {
    let mut pia = Pia6820::new();

    // Enable rising edge interrupt
    pia.write(1, 0x03);
    pia.set_ca1(true);

    // Verify flag is set
    assert_eq!(pia.read(1) & 0x80, 0x80);

    // Write new control value — should NOT clear interrupt flags
    pia.write(1, 0x07); // Change bit 2

    // Flag should still be set
    assert_eq!(pia.read(1) & 0x80, 0x80);
}

#[test]
fn test_multiple_irq_sources() {
    let mut pia = Pia6820::new();

    // Enable CA1 (rising, enabled) and CA2 (input, falling, enabled)
    // CRA: bit 3=1 (CA2 enable), bit 1=1 (CA1 rising), bit 0=1 (CA1 enable)
    pia.write(1, 0x0B);

    // Trigger CA1 rising edge
    pia.set_ca1(true);
    assert!(pia.irq_a());

    // Also trigger CA2 falling edge (bring high first, then low)
    pia.set_ca2(true);
    pia.set_ca2(false);

    // Both flags set
    let cra = pia.read(1);
    assert_eq!(cra & 0xC0, 0xC0); // Both bits 7 and 6

    // IRQ still asserted
    assert!(pia.irq_a());

    // Now read data port to clear flags — need bit 2 set first
    pia.write(1, 0x0F); // Same config + bit 2 for data select
    let _ = pia.read(0);

    // Both flags should be cleared
    assert!(!pia.irq_a());
    assert_eq!(pia.read(1) & 0xC0, 0x00);
}

// ==========================================================================
// Edge detection robustness
// ==========================================================================

#[test]
fn test_no_false_trigger_on_repeated_state() {
    let mut pia = Pia6820::new();

    // Enable CA1 rising edge interrupt
    pia.write(1, 0x03);

    // First call: low -> high = rising edge
    pia.set_ca1(true);
    assert!(pia.irq_a());

    // Clear the flag by reading data port
    pia.write(1, 0x07); // bit 2=1 for data select
    let _ = pia.read(0);
    assert!(!pia.irq_a());

    // Reset control to original
    pia.write(1, 0x03);

    // Second call with same state: high -> high = NO edge
    pia.set_ca1(true);
    assert!(!pia.irq_a()); // Must NOT trigger again
}

#[test]
fn test_no_edge_on_initial_low() {
    let mut pia = Pia6820::new();

    // Enable CA1 falling edge interrupt
    pia.write(1, 0x01); // bit 1=0 (falling), bit 0=1 (enable)

    // Initial state is false. Calling set_ca1(false) = no transition.
    pia.set_ca1(false);
    assert!(!pia.irq_a());
}
