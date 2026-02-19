use phosphor_core::core::{Bus, BusMaster, InterruptState};
use phosphor_core::device::i8257::{DmaDirection, I8257};

/// Simple flat-memory Bus for DMA controller unit tests.
struct TestBus {
    mem: Vec<u8>,
}

impl TestBus {
    fn new() -> Self {
        Self {
            mem: vec![0u8; 0x10000],
        }
    }
}

impl Bus for TestBus {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        *self.mem.get(addr as usize).unwrap_or(&0)
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        if let Some(byte) = self.mem.get_mut(addr as usize) {
            *byte = data;
        }
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState::default()
    }
}

/// Helper: write a 16-bit value to an i8257 register pair via flip-flop.
fn write_reg16(dma: &mut I8257, offset: u8, value: u16) {
    dma.write(offset, value as u8); // LSB first
    dma.write(offset, (value >> 8) as u8); // MSB second
}

/// Helper: read a 16-bit value from an i8257 register pair via flip-flop.
fn read_reg16(dma: &mut I8257, offset: u8) -> u16 {
    let lo = dma.read(offset) as u16;
    let hi = dma.read(offset) as u16;
    (hi << 8) | lo
}

// ---- Construction ----

#[test]
fn default_state_all_channels_disabled() {
    let dma = I8257::new();
    // No channels enabled, no DREQ → HRQ should be false
    assert!(!dma.hrq());
}

#[test]
fn default_status_register_is_zero() {
    let mut dma = I8257::new();
    assert_eq!(dma.read(8), 0x00);
}

// ---- Register access via flip-flop ----

#[test]
fn write_and_read_channel_address() {
    let mut dma = I8257::new();
    // Write channel 0 address = 0x1234
    write_reg16(&mut dma, 0, 0x1234);
    // Read it back
    assert_eq!(read_reg16(&mut dma, 0), 0x1234);
}

#[test]
fn write_and_read_channel_count() {
    let mut dma = I8257::new();
    // Write channel 1 count = 0x8100 (mode=10=Read, count=0x100)
    write_reg16(&mut dma, 3, 0x8100);
    assert_eq!(read_reg16(&mut dma, 3), 0x8100);
}

#[test]
fn all_four_channels_independent() {
    let mut dma = I8257::new();
    write_reg16(&mut dma, 0, 0x1000); // Ch0 addr
    write_reg16(&mut dma, 2, 0x2000); // Ch1 addr
    write_reg16(&mut dma, 4, 0x3000); // Ch2 addr
    write_reg16(&mut dma, 6, 0x4000); // Ch3 addr

    assert_eq!(read_reg16(&mut dma, 0), 0x1000);
    assert_eq!(read_reg16(&mut dma, 2), 0x2000);
    assert_eq!(read_reg16(&mut dma, 4), 0x3000);
    assert_eq!(read_reg16(&mut dma, 6), 0x4000);
}

// ---- Flip-flop behavior ----

#[test]
fn mode_write_resets_flip_flop() {
    let mut dma = I8257::new();
    // Write one byte to offset 0 (flip-flop now = MSB)
    dma.write(0, 0xAA);
    // Write mode register resets flip-flop
    dma.write(8, 0x00);
    // Now writing to offset 0 should write LSB again
    dma.write(0, 0xBB);
    dma.write(0, 0xCC);
    assert_eq!(read_reg16(&mut dma, 0), 0xCCBB);
}

#[test]
fn flip_flop_shared_across_channels() {
    let mut dma = I8257::new();
    // Write LSB to ch0 address (flip-flop → MSB)
    dma.write(0, 0x11);
    // Write to ch1 address — should write MSB due to shared flip-flop
    dma.write(2, 0x22);
    // Ch0 got LSB=0x11, MSB unchanged (0x00)
    // Ch1 got LSB unchanged (0x00), MSB=0x22
    // Reset flip-flop to read cleanly
    dma.write(8, 0x00);
    assert_eq!(read_reg16(&mut dma, 0), 0x0011);
    assert_eq!(read_reg16(&mut dma, 2), 0x2200);
}

#[test]
fn read_status_does_not_reset_flip_flop() {
    let mut dma = I8257::new();
    // Write LSB to ch0 (flip-flop → MSB)
    dma.write(0, 0x55);
    // Read status register — should NOT reset flip-flop
    let _ = dma.read(8);
    // Next write to ch0 should still be MSB
    dma.write(0, 0xAA);
    // Reset flip-flop and read back
    dma.write(8, 0x00);
    assert_eq!(read_reg16(&mut dma, 0), 0xAA55);
}

// ---- Mode register ----

#[test]
fn mode_write_clears_tc_and_update_flags() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Set up ch0 for a 1-byte read transfer to trigger TC
    write_reg16(&mut dma, 0, 0x0100); // address
    write_reg16(&mut dma, 1, 0x8000); // mode=Read, count=0 (TC on first cycle)
    dma.write(8, 0x01); // enable ch0
    dma.set_dreq(0, true);
    bus.mem[0x0100] = 0x42;

    let result = dma.do_dma_cycle(&mut bus, 0);
    assert!(result.unwrap().tc);
    assert_ne!(dma.read(8) & 0x01, 0); // TC flag set

    // Writing mode register clears TC flags
    dma.write(8, 0x01);
    assert_eq!(dma.read(8), 0x00);
}

// ---- HRQ ----

#[test]
fn hrq_requires_enabled_channel_with_dreq() {
    let mut dma = I8257::new();

    // DREQ without channel enabled → no HRQ
    dma.set_dreq(0, true);
    assert!(!dma.hrq());

    // Enable channel 0
    dma.write(8, 0x01);
    assert!(dma.hrq());

    // Deassert DREQ → no HRQ
    dma.set_dreq(0, false);
    assert!(!dma.hrq());
}

#[test]
fn hrq_checks_all_channels() {
    let mut dma = I8257::new();
    dma.write(8, 0x0F); // enable all channels

    for ch in 0..4 {
        dma.set_dreq(ch, true);
        assert!(dma.hrq());
        dma.set_dreq(ch, false);
    }
    assert!(!dma.hrq());
}

// ---- DMA Read transfer (memory → peripheral) ----

#[test]
fn dma_read_single_byte() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    bus.mem[0x2000] = 0xAB;

    write_reg16(&mut dma, 0, 0x2000); // ch0 address
    write_reg16(&mut dma, 1, 0x8000); // mode=Read, count=0 → 1 byte, TC on first
    dma.write(8, 0x01); // enable ch0
    dma.set_dreq(0, true);

    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(result.channel, 0);
    assert_eq!(result.data, 0xAB);
    assert_eq!(result.direction, DmaDirection::Read);
    assert!(result.tc);
}

#[test]
fn dma_read_multi_byte_sequence() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Fill source memory
    for i in 0..4u8 {
        bus.mem[0x1000 + i as usize] = 0x10 + i;
    }

    write_reg16(&mut dma, 0, 0x1000); // ch0 address
    write_reg16(&mut dma, 1, 0x8003); // mode=Read, count=3 (4 bytes total)
    dma.write(8, 0x01); // enable ch0
    dma.set_dreq(0, true);

    let mut results = Vec::new();
    for _ in 0..4 {
        results.push(dma.do_dma_cycle(&mut bus, 0).unwrap());
    }

    assert_eq!(results[0].data, 0x10);
    assert!(!results[0].tc);
    assert_eq!(results[1].data, 0x11);
    assert!(!results[1].tc);
    assert_eq!(results[2].data, 0x12);
    assert!(!results[2].tc);
    assert_eq!(results[3].data, 0x13);
    assert!(results[3].tc);
}

#[test]
fn dma_read_address_increments() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0xFFFE); // ch0 address near wrap
    write_reg16(&mut dma, 1, 0x8001); // mode=Read, count=1 (2 bytes)
    dma.write(8, 0x01);
    dma.set_dreq(0, true);

    dma.do_dma_cycle(&mut bus, 0); // reads 0xFFFE
    dma.do_dma_cycle(&mut bus, 0); // reads 0xFFFF (wraps address to 0x0000)

    // After 2 reads, address should have wrapped to 0x0000
    // Read back the current address register
    dma.write(8, 0x01); // reset flip-flop
    assert_eq!(read_reg16(&mut dma, 0), 0x0000);
}

// ---- DMA Write transfer (peripheral → memory) ----

#[test]
fn dma_write_single_byte() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x3000); // ch0 address
    write_reg16(&mut dma, 1, 0x4000); // mode=Write, count=0 → 1 byte
    dma.write(8, 0x01);
    dma.set_dreq(0, true);

    let result = dma.do_dma_cycle(&mut bus, 0x77).unwrap();
    assert_eq!(result.direction, DmaDirection::Write);
    assert_eq!(result.data, 0x77);
    assert!(result.tc);
    assert_eq!(bus.mem[0x3000], 0x77);
}

#[test]
fn dma_write_multi_byte_sequence() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x5000); // ch0 address
    write_reg16(&mut dma, 1, 0x4002); // mode=Write, count=2 (3 bytes)
    dma.write(8, 0x01);
    dma.set_dreq(0, true);

    for i in 0..3u8 {
        dma.do_dma_cycle(&mut bus, 0xA0 + i);
    }

    assert_eq!(bus.mem[0x5000], 0xA0);
    assert_eq!(bus.mem[0x5001], 0xA1);
    assert_eq!(bus.mem[0x5002], 0xA2);
}

// ---- DMA Verify transfer ----

#[test]
fn dma_verify_no_bus_access() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    bus.mem[0x4000] = 0xFF; // should not be read

    write_reg16(&mut dma, 0, 0x4000); // ch0 address
    write_reg16(&mut dma, 1, 0x0000); // mode=Verify, count=0
    dma.write(8, 0x01);
    dma.set_dreq(0, true);

    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(result.direction, DmaDirection::Verify);
    assert_eq!(result.data, 0);
    assert!(result.tc);
    // Memory should be untouched
    assert_eq!(bus.mem[0x4000], 0xFF);
}

// ---- Terminal count ----

#[test]
fn tc_stop_disables_channel() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8000); // Read, count=0
    dma.write(8, 0x01 | 0x20); // enable ch0 + TC Stop
    dma.set_dreq(0, true);

    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert!(result.tc);

    // Channel should be disabled — no more transfers
    assert!(!dma.hrq());
    assert!(dma.do_dma_cycle(&mut bus, 0).is_none());
}

#[test]
fn without_tc_stop_channel_stays_enabled() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8000); // Read, count=0
    dma.write(8, 0x01); // enable ch0, NO TC Stop
    dma.set_dreq(0, true);

    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert!(result.tc);

    // Channel still enabled — HRQ still active with DREQ
    assert!(dma.hrq());
}

#[test]
fn tc_flags_in_status_register() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Set up channels 0 and 2 with count=0
    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8000); // ch0: Read, count=0
    write_reg16(&mut dma, 4, 0x2000);
    write_reg16(&mut dma, 5, 0x8000); // ch2: Read, count=0
    dma.write(8, 0x05); // enable ch0 + ch2
    dma.set_dreq(0, true);
    dma.do_dma_cycle(&mut bus, 0); // ch0 TC
    dma.set_dreq(0, false);

    dma.set_dreq(2, true);
    dma.do_dma_cycle(&mut bus, 0); // ch2 TC

    let status = dma.read(8);
    assert_eq!(status & 0x05, 0x05); // TC flags for ch0 and ch2
}

// ---- Auto-load ----

#[test]
fn auto_load_ch2_from_ch3() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Set up channel 3 base registers (will be source for auto-load)
    write_reg16(&mut dma, 6, 0x5000); // ch3 address
    write_reg16(&mut dma, 7, 0x80FF); // ch3 count: Read mode, count=0xFF

    // Set up channel 2 for a 1-byte transfer
    write_reg16(&mut dma, 4, 0x2000); // ch2 address
    write_reg16(&mut dma, 5, 0x8000); // ch2 count: Read, count=0

    dma.write(8, 0x04 | 0x40); // enable ch2 + auto-load
    dma.set_dreq(2, true);

    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert!(result.tc);

    // Status should show update flag
    let status = dma.read(8);
    assert_ne!(status & 0x10, 0); // update flag set

    // Channel 2 should now have channel 3's base values
    dma.write(8, 0x04 | 0x40); // reset flip-flop (re-enable + auto-load)
    assert_eq!(read_reg16(&mut dma, 4), 0x5000);
    assert_eq!(read_reg16(&mut dma, 5), 0x80FF);
}

#[test]
fn auto_load_re_enables_channel_after_tc_stop() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 6, 0x5000); // ch3 base
    write_reg16(&mut dma, 7, 0x8001); // ch3 count

    write_reg16(&mut dma, 4, 0x2000); // ch2 address
    write_reg16(&mut dma, 5, 0x8000); // ch2 count=0

    dma.write(8, 0x04 | 0x20 | 0x40); // enable ch2 + TC Stop + auto-load
    dma.set_dreq(2, true);

    dma.do_dma_cycle(&mut bus, 0); // TC → auto-load
    // Despite TC Stop, auto-load should re-enable ch2
    assert!(dma.hrq());
}

// ---- Priority ----

#[test]
fn fixed_priority_ch0_wins() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Set up ch0 and ch2
    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8001); // ch0: Read, count=1
    write_reg16(&mut dma, 4, 0x2000);
    write_reg16(&mut dma, 5, 0x8001); // ch2: Read, count=1

    dma.write(8, 0x05); // enable ch0 + ch2, fixed priority
    dma.set_dreq(0, true);
    dma.set_dreq(2, true);

    // Ch0 should always be serviced first
    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(result.channel, 0);
    let result = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(result.channel, 0);
}

#[test]
fn rotating_priority_round_robin() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    // Set up ch0 and ch1 with large counts
    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x80FF); // ch0: Read
    write_reg16(&mut dma, 2, 0x2000);
    write_reg16(&mut dma, 3, 0x80FF); // ch1: Read

    dma.write(8, 0x03 | 0x10); // enable ch0+ch1, rotating priority
    dma.set_dreq(0, true);
    dma.set_dreq(1, true);

    // First cycle: starts after last_serviced=0, so ch1 first (rotating starts at last+1)
    // Actually, initial last_serviced=0, so rotating checks ch1 first
    let r1 = dma.do_dma_cycle(&mut bus, 0).unwrap();
    let r2 = dma.do_dma_cycle(&mut bus, 0).unwrap();

    // Should alternate between channels
    assert_ne!(r1.channel, r2.channel);
}

// ---- No service when no DREQ ----

#[test]
fn no_transfer_without_dreq() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8001);
    dma.write(8, 0x01); // enable ch0
    // DREQ not set

    assert!(dma.do_dma_cycle(&mut bus, 0).is_none());
}

#[test]
fn no_transfer_on_disabled_channel() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    write_reg16(&mut dma, 0, 0x1000);
    write_reg16(&mut dma, 1, 0x8001);
    dma.write(8, 0x00); // all channels disabled
    dma.set_dreq(0, true);

    assert!(dma.do_dma_cycle(&mut bus, 0).is_none());
}

// ---- Address wrap ----

#[test]
fn address_wraps_at_16_bit_boundary() {
    let mut dma = I8257::new();
    let mut bus = TestBus::new();

    bus.mem[0xFFFF] = 0xAA;
    bus.mem[0x0000] = 0xBB;

    write_reg16(&mut dma, 0, 0xFFFF);
    write_reg16(&mut dma, 1, 0x8001); // Read, count=1 (2 bytes)
    dma.write(8, 0x01);
    dma.set_dreq(0, true);

    let r1 = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(r1.data, 0xAA);

    let r2 = dma.do_dma_cycle(&mut bus, 0).unwrap();
    assert_eq!(r2.data, 0xBB); // wrapped to 0x0000
}
