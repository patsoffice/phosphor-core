use phosphor_core::device::williams_blitter::WilliamsBlitter;

fn make_vram() -> Vec<u8> {
    vec![0u8; 0xC000] // 48KB, same as Joust video RAM
}

/// Run the blitter to completion, returning the number of cycles consumed.
fn run_to_completion(blitter: &mut WilliamsBlitter, vram: &mut [u8]) -> usize {
    let mut cycles = 0;
    while blitter.is_active() {
        blitter.do_dma_cycle(vram);
        cycles += 1;
        assert!(cycles < 100_000, "blit did not complete");
    }
    cycles
}

// ===== Construction and Defaults =====

#[test]
fn test_not_active_initially() {
    let blitter = WilliamsBlitter::new();
    assert!(!blitter.is_active());
}

#[test]
fn test_default_is_same_as_new() {
    let a = WilliamsBlitter::new();
    let b = WilliamsBlitter::default();
    assert_eq!(a.is_active(), b.is_active());
    for offset in 0..8 {
        assert_eq!(a.read_register(offset), b.read_register(offset));
    }
}

// ===== Register Write/Readback =====

#[test]
fn test_register_write_readback() {
    let mut blitter = WilliamsBlitter::new();

    blitter.write_register(0, 0xFF); // mask
    blitter.write_register(1, 0x42); // solid color
    blitter.write_register(2, 0x10); // src hi
    blitter.write_register(3, 0x20); // src lo
    blitter.write_register(4, 0x30); // dst hi
    blitter.write_register(5, 0x40); // dst lo
    blitter.write_register(6, 0x03); // width

    assert_eq!(blitter.read_register(0), 0xFF);
    assert_eq!(blitter.read_register(1), 0x42);
    assert_eq!(blitter.read_register(2), 0x10);
    assert_eq!(blitter.read_register(3), 0x20);
    assert_eq!(blitter.read_register(4), 0x30);
    assert_eq!(blitter.read_register(5), 0x40);
    assert_eq!(blitter.read_register(6), 0x03);
}

#[test]
fn test_write_height_triggers_blit() {
    let mut blitter = WilliamsBlitter::new();
    blitter.write_register(6, 0); // width = 1
    blitter.write_register(7, 0); // height = 1, triggers blit
    assert!(blitter.is_active());
}

// ===== Simple Copy Operations =====

#[test]
fn test_copy_1x1() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();
    vram[0x0100] = 0xAB; // Source byte

    blitter.set_control(0x00); // Pure copy mode
    blitter.write_register(0, 0xFF); // mask = all bits
    blitter.write_register(2, 0x01); // src hi
    blitter.write_register(3, 0x00); // src lo -> src = 0x0100
    blitter.write_register(4, 0x02); // dst hi
    blitter.write_register(5, 0x00); // dst lo -> dst = 0x0200
    blitter.write_register(6, 0); // width = 1 byte
    blitter.write_register(7, 0); // height = 1 row, trigger

    let cycles = run_to_completion(&mut blitter, &mut vram);
    assert_eq!(cycles, 1, "1x1 blit should take 1 cycle");
    assert!(!blitter.is_active());
    assert_eq!(vram[0x0200], 0xAB);
}

#[test]
fn test_copy_4x1() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    // Source data: 4 bytes starting at 0x0100
    vram[0x0100] = 0x11;
    vram[0x0101] = 0x22;
    vram[0x0102] = 0x33;
    vram[0x0103] = 0x44;

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF); // mask = all
    blitter.write_register(2, 0x01); // src = 0x0100
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02); // dst = 0x0200
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 3); // width = 4 bytes (0-based)
    blitter.write_register(7, 0); // height = 1 row, trigger

    let cycles = run_to_completion(&mut blitter, &mut vram);
    assert_eq!(cycles, 4, "4x1 blit should take 4 cycles");
    assert_eq!(vram[0x0200], 0x11);
    assert_eq!(vram[0x0201], 0x22);
    assert_eq!(vram[0x0202], 0x33);
    assert_eq!(vram[0x0203], 0x44);
}

#[test]
fn test_copy_2x3() {
    // 2 columns x 3 rows
    // Destination stride between rows = 256
    // Source is packed linearly
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    // Source: 6 bytes packed at 0x0100
    vram[0x0100] = 0xA1; // row 0, col 0
    vram[0x0101] = 0xA2; // row 0, col 1
    vram[0x0102] = 0xB1; // row 1, col 0
    vram[0x0103] = 0xB2; // row 1, col 1
    vram[0x0104] = 0xC1; // row 2, col 0
    vram[0x0105] = 0xC2; // row 2, col 1

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01); // src = 0x0100
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x20); // dst = 0x2000
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 1); // width = 2 (0-based)
    blitter.write_register(7, 2); // height = 3 (0-based), trigger

    let cycles = run_to_completion(&mut blitter, &mut vram);
    assert_eq!(cycles, 6, "2x3 blit should take 6 cycles");

    // Row 0: dst = 0x2000, 0x2001
    assert_eq!(vram[0x2000], 0xA1);
    assert_eq!(vram[0x2001], 0xA2);
    // Row 1: dst = 0x2100, 0x2101 (stride 256)
    assert_eq!(vram[0x2100], 0xB1);
    assert_eq!(vram[0x2101], 0xB2);
    // Row 2: dst = 0x2200, 0x2201
    assert_eq!(vram[0x2200], 0xC1);
    assert_eq!(vram[0x2201], 0xC2);
}

// ===== Solid Fill =====

#[test]
fn test_solid_fill() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    blitter.set_control(0x04); // Solid flag (bit 2)
    blitter.write_register(0, 0xFF); // mask = all
    blitter.write_register(1, 0x55); // solid_color = 0x55
    blitter.write_register(2, 0x00); // src doesn't matter in solid mode
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x10); // dst = 0x1000
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 2); // width = 3
    blitter.write_register(7, 1); // height = 2, trigger

    let cycles = run_to_completion(&mut blitter, &mut vram);
    assert_eq!(cycles, 6, "3x2 solid fill should take 6 cycles");

    // Row 0
    assert_eq!(vram[0x1000], 0x55);
    assert_eq!(vram[0x1001], 0x55);
    assert_eq!(vram[0x1002], 0x55);
    // Row 1 (stride 256)
    assert_eq!(vram[0x1100], 0x55);
    assert_eq!(vram[0x1101], 0x55);
    assert_eq!(vram[0x1102], 0x55);
}

// ===== Mask =====

#[test]
fn test_mask_partial() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    vram[0x0100] = 0xAB; // Source
    vram[0x0200] = 0xCD; // Existing destination

    blitter.set_control(0x00);
    blitter.write_register(0, 0xF0); // mask = upper nibble only
    blitter.write_register(2, 0x01); // src = 0x0100
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02); // dst = 0x0200
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0); // width = 1
    blitter.write_register(7, 0); // height = 1, trigger

    run_to_completion(&mut blitter, &mut vram);

    // Upper nibble from source (0xA_), lower nibble preserved from dest (_D)
    assert_eq!(vram[0x0200], 0xAD);
}

#[test]
fn test_mask_ff_full_write() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    vram[0x0100] = 0xAB;
    vram[0x0200] = 0xCD;

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF); // mask = all bits
    blitter.write_register(2, 0x01);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    run_to_completion(&mut blitter, &mut vram);

    assert_eq!(
        vram[0x0200], 0xAB,
        "Full mask should write entire source byte"
    );
}

// ===== Foreground Only (Transparency) =====

#[test]
fn test_foreground_only_skips_zero() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    vram[0x0100] = 0x00; // Source is zero
    vram[0x0200] = 0xCC; // Existing destination

    blitter.set_control(0x02); // Foreground-only flag (bit 1)
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    run_to_completion(&mut blitter, &mut vram);

    assert_eq!(
        vram[0x0200], 0xCC,
        "Zero source should not overwrite destination in foreground-only mode"
    );
}

#[test]
fn test_foreground_only_writes_nonzero() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    vram[0x0100] = 0x42; // Non-zero source
    vram[0x0200] = 0xCC; // Existing destination

    blitter.set_control(0x02); // Foreground-only
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    run_to_completion(&mut blitter, &mut vram);

    assert_eq!(
        vram[0x0200], 0x42,
        "Non-zero source should be written in foreground-only mode"
    );
}

// ===== Cycle Count =====

#[test]
fn test_cycle_count() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x20);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 3); // width = 4
    blitter.write_register(7, 2); // height = 3, trigger

    let cycles = run_to_completion(&mut blitter, &mut vram);
    assert_eq!(cycles, 12, "4x3 blit should take exactly 12 cycles");
}

// ===== Completion =====

#[test]
fn test_completion_clears_active() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x00);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x10);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0); // 1x1
    blitter.write_register(7, 0);

    assert!(blitter.is_active());
    blitter.do_dma_cycle(&mut vram);
    assert!(!blitter.is_active(), "Active should clear after completion");
}

// ===== Out of Bounds Safety =====

#[test]
fn test_out_of_bounds_safe() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = vec![0u8; 256]; // Tiny VRAM

    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0xFF); // src = 0xFF00 (beyond vram)
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0xFE); // dst = 0xFE00 (beyond vram)
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    // Should not panic
    run_to_completion(&mut blitter, &mut vram);
}

// ===== Re-trigger =====

#[test]
fn test_retrigger_after_completion() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    // First blit: copy 0xAA
    vram[0x0100] = 0xAA;
    blitter.set_control(0x00);
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01);
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02);
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    run_to_completion(&mut blitter, &mut vram);
    assert_eq!(vram[0x0200], 0xAA);
    assert!(!blitter.is_active());

    // Second blit: copy 0xBB to a different location
    vram[0x0300] = 0xBB;
    blitter.write_register(2, 0x03); // src = 0x0300
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x04); // dst = 0x0400
    blitter.write_register(5, 0x00);
    blitter.write_register(7, 0); // re-trigger

    run_to_completion(&mut blitter, &mut vram);
    assert_eq!(vram[0x0400], 0xBB);
}

// ===== Shift Mode =====

#[test]
fn test_shift_mode() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    // Source bytes: 0xAB, 0xCD
    // With shift mode, shift register starts at 0:
    //   byte 0: output = (0x00 << 8 | 0xAB) >> 4 = 0x0A, shift_reg = 0xAB
    //   byte 1: output = (0xAB << 8 | 0xCD) >> 4 = 0xABC (& 0xFF) = 0xBC, shift_reg = 0xCD
    vram[0x0100] = 0xAB;
    vram[0x0101] = 0xCD;

    blitter.set_control(0x08); // Shift flag (bit 3)
    blitter.write_register(0, 0xFF);
    blitter.write_register(2, 0x01); // src = 0x0100
    blitter.write_register(3, 0x00);
    blitter.write_register(4, 0x02); // dst = 0x0200
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 1); // width = 2
    blitter.write_register(7, 0); // height = 1, trigger

    run_to_completion(&mut blitter, &mut vram);

    assert_eq!(vram[0x0200], 0x0A, "First shifted byte");
    assert_eq!(vram[0x0201], 0xBC, "Second shifted byte");
}

// ===== Solid + Foreground-Only =====

#[test]
fn test_solid_foreground_zero_skips() {
    let mut blitter = WilliamsBlitter::new();
    let mut vram = make_vram();

    vram[0x0200] = 0xEE; // Pre-existing destination data

    // Solid fill with color=0x00 + foreground-only: should skip all writes
    blitter.set_control(0x06); // Solid (bit 2) + Foreground-only (bit 1)
    blitter.write_register(0, 0xFF);
    blitter.write_register(1, 0x00); // solid_color = 0x00
    blitter.write_register(4, 0x02); // dst = 0x0200
    blitter.write_register(5, 0x00);
    blitter.write_register(6, 0);
    blitter.write_register(7, 0);

    run_to_completion(&mut blitter, &mut vram);

    assert_eq!(
        vram[0x0200], 0xEE,
        "Solid color 0x00 with foreground-only should skip write"
    );
}
