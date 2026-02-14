use phosphor_core::device::Pokey;

#[test]
fn test_register_routing() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x00, 100); // AUDF1
    pokey.write(0x01, 0xAF); // AUDC1

    // Verify side effects via pot scan (indirect)
    pokey.set_pot_input(0, 50);
    pokey.write(0x0B, 0); // POTGO

    // Tick enough times
    for _ in 0..6000 {
        pokey.tick();
    }

    let pot0 = pokey.read(0x00);
    assert_eq!(pot0, 50);
}

#[test]
fn test_poly_counters() {
    let mut pokey = Pokey::new(44100);

    let r1 = pokey.read(0x0A);
    pokey.tick();
    let r2 = pokey.read(0x0A);
    assert_ne!(r1, r2);

    // Test 9-bit mode
    pokey.write(0x08, 0x80); // AUDCTL bit 7 set
    let r3 = pokey.read(0x0A);
    pokey.tick();
    let r4 = pokey.read(0x0A);
    assert_ne!(r3, r4);
}

#[test]
fn test_divider_frequency() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x08, 0x40); // Ch1 1.79MHz
    pokey.write(0x00, 4); // Period 5
    pokey.write(0x01, 0xAF); // Pure tone, vol 15
    pokey.write(0x09, 0); // Reset

    pokey.drain_audio();
    for _ in 0..10 {
        pokey.tick();
    }

    let samples = pokey.drain_audio();
    assert!(!samples.is_empty());
}

#[test]
fn test_linked_mode() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x08, 0x50); // Ch1+2 linked, Ch1 1.79MHz
    pokey.write(0x00, 0xFF);
    pokey.write(0x02, 0x00); // Total 255
    pokey.write(0x03, 0xAF); // Ch2 output
    pokey.write(0x09, 0);

    for _ in 0..300 {
        pokey.tick();
    }

    let samples = pokey.drain_audio();
    assert!(!samples.is_empty());
}

#[test]
fn test_volume_only() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x01, 0x08); // Vol 8, force output

    for _ in 0..50 {
        pokey.tick();
    }

    let samples = pokey.drain_audio();
    assert!(!samples.is_empty());
    let val = samples[0];
    assert!((val - 0.133).abs() < 0.01);
}

#[test]
fn test_irq_lifecycle() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x0E, 0x01); // Enable IRQ1
    pokey.write(0x08, 0x40); // 1.79MHz
    pokey.write(0x00, 2);
    pokey.write(0x09, 0);

    assert_eq!(pokey.read(0x0E) & 0x01, 0x01); // Not pending
    assert_eq!(pokey.irq(), false);

    for _ in 0..10 {
        pokey.tick();
    }

    assert_eq!(pokey.read(0x0E) & 0x01, 0x00); // Pending
    assert_eq!(pokey.irq(), true);

    pokey.write(0x0E, 0x00); // Disable

    assert_eq!(pokey.read(0x0E) & 0x01, 0x01); // Cleared
    assert_eq!(pokey.irq(), false);
}

#[test]
fn test_pot_scan() {
    let mut pokey = Pokey::new(44100);
    pokey.set_pot_input(2, 10);
    pokey.set_pot_input(5, 20);

    pokey.write(0x0B, 0); // POTGO
    assert_eq!(pokey.read(0x08), 0xFF);

    for _ in 0..1200 {
        pokey.tick();
    }

    let allpot = pokey.read(0x08);
    assert_eq!(allpot & 0x04, 0); // Pot 2 done
    assert_eq!(allpot & 0x20, 0x20); // Pot 5 not done
    assert_eq!(pokey.read(0x02), 10);

    for _ in 0..1200 {
        pokey.tick();
    }

    let allpot2 = pokey.read(0x08);
    assert_eq!(allpot2 & 0x20, 0); // Pot 5 done
    assert_eq!(pokey.read(0x05), 20);
}

#[test]
fn test_stimer_reset() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x08, 0x40); // 1.79MHz
    pokey.write(0x00, 10); // Period 11
    pokey.write(0x0E, 0x01); // Enable IRQ1
    pokey.write(0x09, 0); // Reset

    for _ in 0..5 {
        pokey.tick();
    }
    pokey.write(0x09, 0); // Reset again

    for _ in 0..10 {
        pokey.tick();
    }
    assert!(!pokey.irq()); // Should not have fired

    pokey.tick();
    pokey.tick(); // 12 ticks from reset
    assert!(pokey.irq());
}

#[test]
fn test_high_pass_filter() {
    let mut pokey = Pokey::new(44100);
    // Ch1 filtered by Ch3 (AUDCTL bit 2)
    pokey.write(0x08, 0x44); // 1.79MHz Ch1 + HPF Ch1

    // Ch1: Pure tone, vol 15
    pokey.write(0x00, 10);
    pokey.write(0x01, 0xAF);

    // Ch3: Pure tone, vol 0 (modulator only)
    pokey.write(0x04, 20);
    pokey.write(0x05, 0xA0);

    pokey.write(0x09, 0);

    // We expect Ch1 output to be modulated by Ch3.
    // Since we can't easily check the waveform shape without FFT or complex analysis,
    // we just verify that we get *some* output and it's not constant.

    for _ in 0..100 {
        pokey.tick();
    }
    let samples = pokey.drain_audio();
    assert!(!samples.is_empty());

    // Check for variation
    let min = samples.iter().fold(1.0, |a, &b| a.min(b));
    let max = samples.iter().fold(0.0, |a, &b| a.max(b));
    assert!(max > min);
}

#[test]
fn test_peripheral_stubs() {
    let mut pokey = Pokey::new(44100);

    // KBCODE
    pokey.set_kbcode(0x42);
    assert_eq!(pokey.read(0x09), 0x42);

    // SERIN
    pokey.set_serin(0x55);
    assert_eq!(pokey.read(0x0D), 0x55);

    // SEROUT
    pokey.write(0x0D, 0xAA);
    assert_eq!(pokey.read_serout(), 0xAA);

    // SKSTAT
    // Initial 0xFF
    assert_eq!(pokey.read(0x0F), 0xFF);
}

#[test]
fn test_audio_drain() {
    let mut pokey = Pokey::new(44100);
    pokey.write(0x01, 0x08); // Vol 8
    for _ in 0..100 {
        pokey.tick();
    }

    let s1 = pokey.drain_audio();
    assert!(!s1.is_empty());

    let s2 = pokey.drain_audio();
    assert!(s2.is_empty());
}
