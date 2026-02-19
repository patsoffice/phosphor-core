use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::Cpu;
use phosphor_core::cpu::i8035::I8035;
use phosphor_core::cpu::z80::Z80;
use phosphor_core::device::dac::Mc1408Dac;
use phosphor_core::device::dkong_discrete::DkongDiscrete;
use phosphor_core::device::i8257::I8257;

use crate::rom_loader::{RomEntry, RomRegion};

// ---------------------------------------------------------------------------
// Donkey Kong ROM definitions (TKG-04 / "dkong" MAME set)
// ---------------------------------------------------------------------------

/// Main CPU program ROMs: 16KB at 0x0000-0x3FFF (four 4KB chips).
pub static DKONG_PROGRAM_ROM: RomRegion = RomRegion {
    size: 0x4000,
    entries: &[
        RomEntry {
            name: "c_5et_g.bin",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0xba70b88b],
        },
        RomEntry {
            name: "c_5ct_g.bin",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0x5ec461ec],
        },
        RomEntry {
            name: "c_5bt_g.bin",
            size: 0x1000,
            offset: 0x2000,
            crc32: &[0x1c97d324],
        },
        RomEntry {
            name: "c_5at_g.bin",
            size: 0x1000,
            offset: 0x3000,
            crc32: &[0xb9005ac0],
        },
    ],
};

/// Sound CPU ROM: 2KB at 0x0000-0x07FF, mirrored to 0x0800-0x0FFF.
pub static DKONG_SOUND_ROM: RomRegion = RomRegion {
    size: 0x0800,
    entries: &[RomEntry {
        name: "s_3i_b.bin",
        size: 0x0800,
        offset: 0x0000,
        crc32: &[0x45a4ed06],
    }],
};

/// Tune ROM: 2KB, accessed via MOVX with P2 bank select.
pub static DKONG_TUNE_ROM: RomRegion = RomRegion {
    size: 0x0800,
    entries: &[RomEntry {
        name: "s_3j_b.bin",
        size: 0x0800,
        offset: 0x0000,
        crc32: &[0x4743fe92],
    }],
};

/// Tile GFX: 4KB (two 2KB ROMs, one per bitplane).
pub static DKONG_TILE_ROM: RomRegion = RomRegion {
    size: 0x1000,
    entries: &[
        RomEntry {
            name: "v_5h_b.bin",
            size: 0x0800,
            offset: 0x0000,
            crc32: &[0x12c8c95d],
        },
        RomEntry {
            name: "v_3pt.bin",
            size: 0x0800,
            offset: 0x0800,
            crc32: &[0x15e9c5e9],
        },
    ],
};

/// Sprite GFX: 8KB (four 2KB ROMs, interleaved).
pub static DKONG_SPRITE_ROM: RomRegion = RomRegion {
    size: 0x2000,
    entries: &[
        RomEntry {
            name: "l_4m_b.bin",
            size: 0x0800,
            offset: 0x0000,
            crc32: &[0x59f8054d],
        },
        RomEntry {
            name: "l_4n_b.bin",
            size: 0x0800,
            offset: 0x0800,
            crc32: &[0x672e4714],
        },
        RomEntry {
            name: "l_4r_b.bin",
            size: 0x0800,
            offset: 0x1000,
            crc32: &[0xfeaa59ee],
        },
        RomEntry {
            name: "l_4s_b.bin",
            size: 0x0800,
            offset: 0x1800,
            crc32: &[0x20f2ef7e],
        },
    ],
};

/// Palette PROMs: c-2k (256B), c-2j (256B), v-5e (256B color codes).
pub static DKONG_PALETTE_PROMS: RomRegion = RomRegion {
    size: 0x0300,
    entries: &[
        RomEntry {
            name: "c-2k.bpr",
            size: 0x0100,
            offset: 0x0000,
            crc32: &[0xe273ede5],
        },
        RomEntry {
            name: "c-2j.bpr",
            size: 0x0100,
            offset: 0x0100,
            crc32: &[0xd6412358],
        },
        RomEntry {
            name: "v-5e.bpr",
            size: 0x0100,
            offset: 0x0200,
            crc32: &[0xb869b8f5],
        },
    ],
};

// ---------------------------------------------------------------------------
// Input button IDs (active-high: 0x00 = all released)
// ---------------------------------------------------------------------------
pub const INPUT_P1_RIGHT: u8 = 0;
pub const INPUT_P1_LEFT: u8 = 1;
pub const INPUT_P1_UP: u8 = 2;
pub const INPUT_P1_DOWN: u8 = 3;
pub const INPUT_P1_JUMP: u8 = 4;
pub const INPUT_P1_START: u8 = 5;
pub const INPUT_P2_START: u8 = 6;
pub const INPUT_COIN: u8 = 7;
pub const INPUT_P2_RIGHT: u8 = 8;
pub const INPUT_P2_LEFT: u8 = 9;
pub const INPUT_P2_UP: u8 = 10;
pub const INPUT_P2_DOWN: u8 = 11;
pub const INPUT_P2_JUMP: u8 = 12;

const DKONG_INPUT_MAP: &[InputButton] = &[
    InputButton {
        id: INPUT_P1_RIGHT,
        name: "P1 Right",
    },
    InputButton {
        id: INPUT_P1_LEFT,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_P1_UP,
        name: "P1 Up",
    },
    InputButton {
        id: INPUT_P1_DOWN,
        name: "P1 Down",
    },
    InputButton {
        id: INPUT_P1_JUMP,
        name: "P1 Jump",
    },
    InputButton {
        id: INPUT_P1_START,
        name: "P1 Start",
    },
    InputButton {
        id: INPUT_P2_START,
        name: "P2 Start",
    },
    InputButton {
        id: INPUT_COIN,
        name: "Coin",
    },
    InputButton {
        id: INPUT_P2_RIGHT,
        name: "P2 Right",
    },
    InputButton {
        id: INPUT_P2_LEFT,
        name: "P2 Left",
    },
    InputButton {
        id: INPUT_P2_UP,
        name: "P2 Up",
    },
    InputButton {
        id: INPUT_P2_DOWN,
        name: "P2 Down",
    },
    InputButton {
        id: INPUT_P2_JUMP,
        name: "P2 Jump",
    },
];

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------
// Master clock:  61.44 MHz
// CPU clock:     61.44 / 5 / 4 = 3.072 MHz
// Pixel clock:   61.44 / 10 = 6.144 MHz
// HTOTAL:        384 pixels = 192 CPU cycles per scanline
// VTOTAL:        264 lines
// VBSTART:       240 (visible height)
// Frame:         192 × 264 = 50688 CPU cycles per frame
// Frame rate:    3072000 / 50688 ≈ 60.61 Hz

const CYCLES_PER_SCANLINE: u64 = 192;
const VISIBLE_LINES: u64 = 240;
const TOTAL_LINES: u64 = 264;
const CYCLES_PER_FRAME: u64 = TOTAL_LINES * CYCLES_PER_SCANLINE;

const CPU_CLOCK_HZ: u64 = 3_072_000;
const OUTPUT_SAMPLE_RATE: u64 = 44_100;

// Sound CPU: I8035 @ 6 MHz / 15 = 400 kHz machine cycles
// Bresenham ratio: 400000 / 3072000 = 25 / 192
const SOUND_TICK_NUM: u32 = 25;
const SOUND_TICK_DEN: u32 = 192;

// Screen: 256×240 native, visible region Y: 16-239 (224 lines, VBEND=16).
// Rotated 90° CCW → 224×256 output.
const NATIVE_WIDTH: usize = 256;
const NATIVE_HEIGHT: usize = 240;
const VBLANK_END: usize = 16; // first visible scanline
const SCREEN_WIDTH: u32 = (NATIVE_HEIGHT - VBLANK_END) as u32; // 224
const SCREEN_HEIGHT: u32 = NATIVE_WIDTH as u32; // 256

/// Donkey Kong Arcade System (Nintendo, 1981)
///
/// Hardware: Z80 @ 3.072 MHz (main), I8035 @ 6 MHz (sound).
/// Video: 32×32 tile playfield + 16×16 sprites, 2bpp, PROM palette.
/// Audio: I8035 DAC + discrete circuits (walk, jump, stomp effects).
/// Screen: 256×240 displayed rotated 90° CCW on vertical monitor.
pub struct DkongSystem {
    // Main CPU (Z80 @ 3.072 MHz)
    cpu: Z80,

    // Sound CPU (I8035 @ 6 MHz / 15 = 400 kHz machine cycles)
    sound_cpu: I8035,

    // Main CPU memory
    rom: [u8; 0x4000],
    ram: [u8; 0x0C00],
    sprite_ram: [u8; 0x0400],
    video_ram: [u8; 0x0400],

    // Sound CPU memory
    sound_rom: [u8; 0x1000],
    tune_rom: [u8; 0x0800],

    // GFX ROMs
    tile_rom: [u8; 0x1000],
    sprite_rom: [u8; 0x2000],

    // PROMs
    palette_prom: [u8; 0x0200], // c-2k + c-2j
    color_prom: [u8; 0x0100],   // v-5e

    // Pre-computed palette (256 RGB entries)
    palette_rgb: [(u8, u8, u8); 256],

    // Scanline-rendered framebuffer (256 × 240 × RGB24)
    scanline_buffer: Vec<u8>,

    // I/O state (active-high: 0x00 = all released)
    in0: u8,
    in1: u8,
    in2: u8,
    dsw0: u8,

    // Control registers
    sound_latch: u8,
    sound_control_latch: u8,
    flip_screen: bool,
    sprite_bank: bool,
    nmi_mask: bool,
    palette_bank: u8,

    // DMA controller (i8257)
    dma: I8257,

    // Sound CPU interface
    sound_irq_pending: bool,

    // Audio output
    dac: Mc1408Dac,
    audio_buffer: Vec<i16>,
    sample_accum: i64,
    sample_count: u32,
    sample_phase: u64,

    // Timing
    clock: u64,
    sound_phase_accum: u32,
    vblank_nmi_pending: bool,

    // Discrete sound effects (walk, jump, stomp)
    discrete: DkongDiscrete,
}

impl Default for DkongSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DkongSystem {
    pub fn new() -> Self {
        Self {
            cpu: Z80::new(),
            sound_cpu: I8035::new(),
            rom: [0; 0x4000],
            ram: [0; 0x0C00],
            sprite_ram: [0; 0x0400],
            video_ram: [0; 0x0400],
            sound_rom: [0; 0x1000],
            tune_rom: [0; 0x0800],
            tile_rom: [0; 0x1000],
            sprite_rom: [0; 0x2000],
            palette_prom: [0; 0x0200],
            color_prom: [0; 0x0100],
            palette_rgb: [(0, 0, 0); 256],
            scanline_buffer: vec![0u8; NATIVE_WIDTH * NATIVE_HEIGHT * 3],
            in0: 0x00,
            in1: 0x00,
            in2: 0x00,
            // Default DIP: upright cabinet (bit 7), 3 lives, 7000 bonus, 1 coin/1 play
            dsw0: 0x80,
            sound_latch: 0,
            sound_control_latch: 0,
            flip_screen: false,
            sprite_bank: false,
            nmi_mask: false,
            palette_bank: 0,
            dma: I8257::new(),
            sound_irq_pending: false,
            dac: Mc1408Dac::new(),
            audio_buffer: Vec::with_capacity(1024),
            sample_accum: 0,
            sample_count: 0,
            sample_phase: 0,
            clock: 0,
            sound_phase_accum: 0,
            vblank_nmi_pending: false,
            discrete: DkongDiscrete::new(),
        }
    }

    /// Load all ROM sets.
    pub fn load_rom_set(
        &mut self,
        rom_set: &crate::rom_loader::RomSet,
    ) -> Result<(), crate::rom_loader::RomLoadError> {
        let rom_data = DKONG_PROGRAM_ROM.load(rom_set)?;
        self.rom.copy_from_slice(&rom_data);

        let sound_data = DKONG_SOUND_ROM.load(rom_set)?;
        self.sound_rom[..0x0800].copy_from_slice(&sound_data);
        self.sound_rom[0x0800..].copy_from_slice(&sound_data); // mirror

        let tune_data = DKONG_TUNE_ROM.load(rom_set)?;
        self.tune_rom.copy_from_slice(&tune_data);

        let tile_data = DKONG_TILE_ROM.load(rom_set)?;
        self.tile_rom.copy_from_slice(&tile_data);

        let sprite_data = DKONG_SPRITE_ROM.load(rom_set)?;
        self.sprite_rom.copy_from_slice(&sprite_data);

        let prom_data = DKONG_PALETTE_PROMS.load(rom_set)?;
        self.palette_prom.copy_from_slice(&prom_data[..0x200]);
        self.color_prom.copy_from_slice(&prom_data[0x200..0x300]);

        self.build_palette();
        Ok(())
    }

    /// Pre-compute the 256-entry RGB palette from PROMs using resistor network
    /// decoding (Darlington amp for R/G, emitter follower for B).
    fn build_palette(&mut self) {
        for i in 0..256 {
            // PROMs are inverted (open-collector MB7052/MB7114)
            let c2k = !self.palette_prom[i]; // c-2k at offset 0x000
            let c2j = !self.palette_prom[0x100 + i]; // c-2j at offset 0x100

            // Special case: when (i & 0x03) == 0, output is forced black
            // (tri-state NOR on the color decoder)
            if (i & 0x03) == 0x00 {
                self.palette_rgb[i] = (0, 0, 0);
                continue;
            }

            // Red: 3 bits from c-2j (bits 1-3), Darlington amp
            // Resistors: 1kΩ, 470Ω, 220Ω with 470Ω pulldown
            let r_bit0 = ((c2j >> 1) & 1) as f64;
            let r_bit1 = ((c2j >> 2) & 1) as f64;
            let r_bit2 = ((c2j >> 3) & 1) as f64;
            let r = darlington_3bit(r_bit0, r_bit1, r_bit2);

            // Green: c-2j bit 0 (MSB/220Ω) + c-2k bits 2-3 (LSB weights)
            let g_bit0 = ((c2k >> 2) & 1) as f64;
            let g_bit1 = ((c2k >> 3) & 1) as f64;
            let g_bit2 = (c2j & 1) as f64;
            let g = darlington_3bit(g_bit0, g_bit1, g_bit2);

            // Blue: 2 bits from c-2k (bits 0-1), emitter follower
            // Resistors: 470Ω, 220Ω with 680Ω pulldown
            let b_bit0 = (c2k & 1) as f64;
            let b_bit1 = ((c2k >> 1) & 1) as f64;
            let b = emitter_2bit(b_bit0, b_bit1);

            self.palette_rgb[i] = (r, g, b);
        }
    }

    /// Resolve a 2-bit pixel value to an RGB color using the palette system.
    ///
    /// `color` is the combined color code (base_color + 16 * palette_bank, 0-63).
    /// Direct index: palette_rgb[color * 4 + pixel_value].
    fn resolve_color(&self, color: u8, pixel_value: u8) -> (u8, u8, u8) {
        let palette_index = (color as usize & 0x3F) * 4 + (pixel_value as usize & 0x03);
        self.palette_rgb[palette_index & 0xFF]
    }

    /// Decode a single tile pixel from the GFX ROM.
    /// 8×8 tiles, 2bpp planar: plane 0 at 0x000-0x7FF, plane 1 at 0x800-0xFFF.
    fn decode_tile_pixel(&self, tile_code: u16, px: u8, py: u8) -> u8 {
        let tile_offset = (tile_code as usize) * 8 + py as usize;
        let plane0 = self.tile_rom[tile_offset];
        let plane1 = self.tile_rom[0x800 + tile_offset];
        let bit_mask = 0x80 >> px; // MSB = leftmost pixel (hardware shift register)
        let p0 = u8::from(plane0 & bit_mask != 0);
        let p1 = u8::from(plane1 & bit_mask != 0);
        p0 | (p1 << 1)
    }

    /// Decode a single sprite pixel from the GFX ROM.
    /// 16×16 sprites, 2bpp, 4-ROM interleaved layout:
    ///   Plane 0: 0x0000-0x0FFF (l_4m_b + l_4n_b)
    ///   Plane 1: 0x1000-0x1FFF (l_4r_b + l_4s_b)
    ///   Within each plane: left 8px from first 0x800, right 8px from second 0x800.
    fn decode_sprite_pixel(&self, spr_code: u16, px: u8, py: u8) -> u8 {
        let base = (spr_code as usize) * 16 + py as usize;
        let (plane0_addr, plane1_addr) = if px < 8 {
            (base, 0x1000 + base)
        } else {
            (0x0800 + base, 0x1800 + base)
        };
        let bit_mask = 0x80 >> (px & 7); // MSB = leftmost pixel (hardware shift register)
        let p0 = u8::from(self.sprite_rom[plane0_addr] & bit_mask != 0);
        let p1 = u8::from(self.sprite_rom[plane1_addr] & bit_mask != 0);
        p0 | (p1 << 1)
    }

    /// Render a single scanline from current VRAM/sprite state.
    fn render_scanline(&mut self, scanline: usize) {
        let row_offset = scanline * NATIVE_WIDTH * 3;

        // --- Background tiles: 32×32 tilemap, 8×8 tiles ---
        let tile_row = scanline / 8;
        let py = (scanline % 8) as u8;
        for tile_col in 0..32 {
            let vram_offset = tile_row * 32 + tile_col;
            let tile_code = self.video_ram[vram_offset] as u16;
            // Color attribute from v-5e PROM: per-column lookup, combined with palette_bank
            let attribute =
                (self.color_prom[tile_col + 32 * (tile_row / 4)] & 0x0F) + 0x10 * self.palette_bank;

            for px in 0..8u8 {
                let screen_x = tile_col * 8 + px as usize;
                let pixel_value = self.decode_tile_pixel(tile_code, px, py);
                let (r, g, b) = self.resolve_color(attribute, pixel_value);
                let off = row_offset + screen_x * 3;
                self.scanline_buffer[off] = r;
                self.scanline_buffer[off + 1] = g;
                self.scanline_buffer[off + 2] = b;
            }
        }

        // --- Sprites ---
        // Iterate forward: later sprites overwrite earlier ones.
        let sprite_base = if self.sprite_bank { 0x200 } else { 0x000 };
        let mut offs = sprite_base;
        while offs < sprite_base + 0x200 {
            let y_byte = self.sprite_ram[offs];
            let code_byte = self.sprite_ram[offs + 1];
            let attr_byte = self.sprite_ram[offs + 2];
            let x_byte = self.sprite_ram[offs + 3];

            // Visibility: (y + add_y + 1 + scanline_vf) where add_y=0xF9, scanline_vf=scanline-1
            // = (y + 0xF9 + scanline). Test: & 0xF0 == 0xF0, row: & 0x0F.
            let test = y_byte.wrapping_add(0xF9).wrapping_add(scanline as u8);
            if (test & 0xF0) == 0xF0 {
                let row_in_sprite = test & 0x0F;

                // Sprite code: 7 bits from byte 1 + bank bit from byte 2 bit 6
                let spr_code = (code_byte & 0x7F) as u16 | (((attr_byte & 0x40) as u16) << 1);
                let flip_y = (code_byte & 0x80) != 0;
                let flip_x = (attr_byte & 0x80) != 0;
                let color_attr = (attr_byte & 0x0F) + 0x10 * self.palette_bank;

                let src_py = if flip_y {
                    15 - row_in_sprite
                } else {
                    row_in_sprite
                };

                // X position with hardware offset add_x=0xF7, x = (raw + 0xF8) & 0xFF)
                let sprite_x = x_byte.wrapping_add(0xF8) as i32;

                for px in 0..16i32 {
                    let draw_x = (sprite_x + px) & 0xFF;
                    if draw_x >= NATIVE_WIDTH as i32 {
                        continue;
                    }
                    let src_px = if flip_x { 15 - px as u8 } else { px as u8 };
                    let pixel_value = self.decode_sprite_pixel(spr_code, src_px, src_py);
                    if pixel_value == 0 {
                        continue; // transparent
                    }
                    let (r, g, b) = self.resolve_color(color_attr, pixel_value);
                    let off = row_offset + draw_x as usize * 3;
                    self.scanline_buffer[off] = r;
                    self.scanline_buffer[off + 1] = g;
                    self.scanline_buffer[off + 2] = b;
                }

                // Sprite X wraparound
                if sprite_x >= 240 {
                    for px in 0..16i32 {
                        let draw_x = sprite_x + px - 256;
                        if draw_x < 0 || draw_x >= NATIVE_WIDTH as i32 {
                            continue;
                        }
                        let src_px = if flip_x { 15 - px as u8 } else { px as u8 };
                        let pixel_value = self.decode_sprite_pixel(spr_code, src_px, src_py);
                        if pixel_value == 0 {
                            continue;
                        }
                        let (r, g, b) = self.resolve_color(color_attr, pixel_value);
                        let off = row_offset + draw_x as usize * 3;
                        self.scanline_buffer[off] = r;
                        self.scanline_buffer[off + 1] = g;
                        self.scanline_buffer[off + 2] = b;
                    }
                }
            }

            offs += 4;
        }
    }

    /// Execute one CPU cycle at the Z80 clock rate (3.072 MHz).
    pub fn tick(&mut self) {
        let frame_cycle = self.clock % CYCLES_PER_FRAME;

        // Per-scanline rendering at scanline boundary
        if frame_cycle.is_multiple_of(CYCLES_PER_SCANLINE) {
            let scanline = (frame_cycle / CYCLES_PER_SCANLINE) as u16;
            if scanline < VISIBLE_LINES as u16 {
                self.render_scanline(scanline as usize);
            }
        }

        // VBLANK NMI: assert at scanline 240
        let vblank_cycle = VISIBLE_LINES * CYCLES_PER_SCANLINE;
        if frame_cycle == vblank_cycle {
            self.vblank_nmi_pending = true;
        }
        // Clear NMI at frame boundary (end of VBLANK)
        if frame_cycle == 0 && self.clock > 0 {
            self.vblank_nmi_pending = false;
        }

        // Execute main CPU cycle
        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }

        // Tick sound CPU (Bresenham 25/192 ratio: 400 kHz from 3.072 MHz)
        self.sound_phase_accum += SOUND_TICK_NUM;
        if self.sound_phase_accum >= SOUND_TICK_DEN {
            self.sound_phase_accum -= SOUND_TICK_DEN;
            let bus_ptr: *mut Self = self;
            unsafe {
                let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
                self.sound_cpu.execute_cycle(bus, BusMaster::Cpu(1));
            }
        }

        // Audio accumulation (Bresenham downsample: 3.072 MHz → 44.1 kHz)
        self.sample_accum += self.dac.sample_i16() as i64;
        self.sample_count += 1;
        self.sample_phase += OUTPUT_SAMPLE_RATE;
        if self.sample_phase >= CPU_CLOCK_HZ {
            self.sample_phase -= CPU_CLOCK_HZ;
            if self.sample_count > 0 {
                // Mix DAC (averaged) with discrete sound effects
                let dac_sample = (self.sample_accum / self.sample_count as i64) as i32;
                let discrete_sample = self.discrete.generate_sample() as i32;
                let mixed = (dac_sample + discrete_sample).clamp(-32767, 32767) as i16;
                self.audio_buffer.push(mixed);
            }
            self.sample_accum = 0;
            self.sample_count = 0;
        }

        self.clock += 1;
    }
}

impl Bus for DkongSystem {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, master: BusMaster, addr: u16) -> u8 {
        match master {
            // Main CPU (Z80)
            BusMaster::Cpu(0) => match addr {
                0x0000..=0x3FFF => self.rom[addr as usize],
                0x6000..=0x6BFF => self.ram[(addr - 0x6000) as usize],
                0x7000..=0x73FF => self.sprite_ram[(addr - 0x7000) as usize],
                0x7400..=0x77FF => self.video_ram[(addr - 0x7400) as usize],
                0x7800..=0x7808 => self.dma.read((addr - 0x7800) as u8),
                0x7C00 => self.in0,
                0x7C80 => self.in1,
                0x7D00 => {
                    // IN2: active-high inputs + sound status at bit 6
                    let sound_status = if self.sound_cpu.p2 & 0x10 != 0 {
                        0x00
                    } else {
                        0x40
                    };
                    (self.in2 & !0x40) | sound_status
                }
                0x7D80 => self.dsw0,
                _ => 0x00,
            },

            // Sound CPU (I8035) - program memory
            BusMaster::Cpu(1) => {
                let addr12 = (addr & 0x0FFF) as usize;
                self.sound_rom[addr12]
            }

            _ => 0x00,
        }
    }

    fn write(&mut self, master: BusMaster, addr: u16, data: u8) {
        match master {
            BusMaster::Cpu(0) => match addr {
                0x6000..=0x6BFF => self.ram[(addr - 0x6000) as usize] = data,
                0x7000..=0x73FF => self.sprite_ram[(addr - 0x7000) as usize] = data,
                0x7400..=0x77FF => self.video_ram[(addr - 0x7400) as usize] = data,
                0x7800..=0x7808 => self.dma.write((addr - 0x7800) as u8, data),

                // Sound latch (ls175.3d)
                0x7C00 => self.sound_latch = data,

                // 74LS259 sound control latch: addr bits 0-2 select bit, data bit 0 is value
                0x7D00..=0x7D07 => {
                    let bit = (addr & 0x07) as u8;
                    if data & 1 != 0 {
                        self.sound_control_latch |= 1 << bit;
                    } else {
                        self.sound_control_latch &= !(1 << bit);
                    }
                    // Forward bits 0-2 to discrete sound device
                    if bit < 3 {
                        self.discrete.write_latch(bit, data & 1 != 0);
                    }
                }

                // Sound CPU IRQ trigger
                0x7D80 => {
                    self.sound_irq_pending = data != 0;
                }

                // Flip screen
                0x7D82 => self.flip_screen = (data & 1) != 0,

                // Sprite bank select
                0x7D83 => self.sprite_bank = (data & 1) != 0,

                // NMI mask
                0x7D84 => {
                    self.nmi_mask = (data & 1) != 0;
                    if !self.nmi_mask {
                        self.vblank_nmi_pending = false;
                    }
                }

                // DMA DRQ: trigger sprite DMA transfer from i8257 channel 0
                0x7D85 => {
                    let src_addr = self.dma.channel_address(0);
                    let count = ((self.dma.channel_count(0) & 0x3FFF) + 1)
                        .min(self.sprite_ram.len() as u16);
                    for i in 0..count {
                        let addr = src_addr.wrapping_add(i);
                        let byte = match addr {
                            0x0000..=0x3FFF => self.rom[addr as usize],
                            0x6000..=0x6BFF => self.ram[(addr - 0x6000) as usize],
                            _ => 0x00,
                        };
                        self.sprite_ram[i as usize] = byte;
                    }
                }

                // Palette bank (2-bit, one bit per address)
                0x7D86 => {
                    if data & 1 != 0 {
                        self.palette_bank |= 0x01;
                    } else {
                        self.palette_bank &= !0x01;
                    }
                }
                0x7D87 => {
                    if data & 1 != 0 {
                        self.palette_bank |= 0x02;
                    } else {
                        self.palette_bank &= !0x02;
                    }
                }

                _ => {}
            },

            // Sound CPU writes to program memory are ignored
            BusMaster::Cpu(1) => {}

            _ => {}
        }
    }

    fn io_read(&mut self, master: BusMaster, addr: u16) -> u8 {
        match master {
            // Main CPU: no I/O port reads used on DK
            BusMaster::Cpu(0) => 0xFF,

            // Sound CPU I/O
            BusMaster::Cpu(1) => match addr {
                // MOVX and INS A,BUS: dkong_tune_r logic
                0x00..=0x100 => {
                    if self.sound_cpu.p2 & 0x40 != 0 {
                        // Command mode: read sound latch (lower 4 bits, inverted by ls175.3d)
                        (self.sound_latch & 0x0F) ^ 0x0F
                    } else {
                        // Tune ROM mode: bank select from P2 bits 2-0
                        let bank = (self.sound_cpu.p2 & 0x07) as usize;
                        let offset = (addr & 0xFF) as usize;
                        let rom_addr = bank * 256 + offset;
                        if rom_addr < self.tune_rom.len() {
                            self.tune_rom[rom_addr]
                        } else {
                            0xFF
                        }
                    }
                }

                // IN A,P1: read P1 latch
                0x101 => self.sound_cpu.p1,

                // IN A,P2: virtual port with bit 5 from sound control latch bit 3 (XOR'd)
                0x102 => {
                    let mut val = self.sound_cpu.p2;
                    // Bit 5: read from sound_control_latch bit 3, then XOR with 0x20
                    val = (val & !0x20)
                        | if self.sound_control_latch & 0x08 != 0 {
                            0x20
                        } else {
                            0x00
                        };
                    val ^ 0x20
                }

                // T0: inverted bit 5 of sound control latch
                0x110 => u8::from(self.sound_control_latch & 0x20 == 0),

                // T1: inverted bit 4 of sound control latch
                0x111 => u8::from(self.sound_control_latch & 0x10 == 0),

                _ => 0xFF,
            },

            _ => 0xFF,
        }
    }

    fn io_write(&mut self, master: BusMaster, addr: u16, data: u8) {
        match master {
            BusMaster::Cpu(0) => {}

            BusMaster::Cpu(1) => match addr {
                // OUTL P1,A: DAC output
                0x101 => self.dac.write(data),

                // OUTL P2,A: control port (tracked by I8035 internally)
                0x102 => {}

                _ => {}
            },

            _ => {}
        }
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, target: BusMaster) -> InterruptState {
        match target {
            // Main CPU: VBlank NMI (edge-triggered by Z80)
            BusMaster::Cpu(0) => InterruptState {
                nmi: self.vblank_nmi_pending && self.nmi_mask,
                irq: false,
                firq: false,
                ..Default::default()
            },

            // Sound CPU: IRQ from main CPU
            BusMaster::Cpu(1) => InterruptState {
                nmi: false,
                irq: self.sound_irq_pending,
                firq: false,
                ..Default::default()
            },

            _ => InterruptState::default(),
        }
    }
}

impl Machine for DkongSystem {
    fn display_size(&self) -> (u32, u32) {
        (SCREEN_WIDTH, SCREEN_HEIGHT) // 240×256 (rotated)
    }

    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        // Rotate 90° CCW from native scanline_buffer (256w × 240h)
        // to output buffer (224w × 256h), clipping VBLANK (scanlines 0-15).
        // ox ∈ [0,223] → ny = 239 - ox ∈ [16,239] (visible region only)
        let out_w = SCREEN_WIDTH as usize; // 224
        for oy in 0..SCREEN_HEIGHT as usize {
            for ox in 0..out_w {
                let nx = oy;
                let ny = (NATIVE_HEIGHT - 1) - ox;
                let src = (ny * NATIVE_WIDTH + nx) * 3;
                let dst = (oy * out_w + ox) * 3;
                buffer[dst] = self.scanline_buffer[src];
                buffer[dst + 1] = self.scanline_buffer[src + 1];
                buffer[dst + 2] = self.scanline_buffer[src + 2];
            }
        }
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // IN0: active-high (set bit on press, clear on release)
            INPUT_P1_RIGHT => set_bit_active_high(&mut self.in0, 0, pressed),
            INPUT_P1_LEFT => set_bit_active_high(&mut self.in0, 1, pressed),
            INPUT_P1_UP => set_bit_active_high(&mut self.in0, 2, pressed),
            INPUT_P1_DOWN => set_bit_active_high(&mut self.in0, 3, pressed),
            INPUT_P1_JUMP => set_bit_active_high(&mut self.in0, 4, pressed),

            // IN1: active-high
            INPUT_P2_RIGHT => set_bit_active_high(&mut self.in1, 0, pressed),
            INPUT_P2_LEFT => set_bit_active_high(&mut self.in1, 1, pressed),
            INPUT_P2_UP => set_bit_active_high(&mut self.in1, 2, pressed),
            INPUT_P2_DOWN => set_bit_active_high(&mut self.in1, 3, pressed),
            INPUT_P2_JUMP => set_bit_active_high(&mut self.in1, 4, pressed),

            // IN2: active-high
            INPUT_P1_START => set_bit_active_high(&mut self.in2, 2, pressed),
            INPUT_P2_START => set_bit_active_high(&mut self.in2, 3, pressed),
            INPUT_COIN => set_bit_active_high(&mut self.in2, 7, pressed),

            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        DKONG_INPUT_MAP
    }

    fn reset(&mut self) {
        self.cpu.reset();
        self.sound_cpu.reset();
        self.cpu.pc = 0x0000;

        self.nmi_mask = false;
        self.vblank_nmi_pending = false;
        self.sound_irq_pending = false;
        self.sound_latch = 0;
        self.sound_control_latch = 0;
        self.flip_screen = false;
        self.sprite_bank = false;
        self.palette_bank = 0;
        self.dma = I8257::new();

        self.clock = 0;
        self.sound_phase_accum = 0;
        self.sample_accum = 0;
        self.sample_count = 0;
        self.sample_phase = 0;
        self.audio_buffer.clear();
        self.dac = Mc1408Dac::new();

        self.in0 = 0x00;
        self.in1 = 0x00;
        self.in2 = 0x00;

        self.video_ram = [0; 0x0400];
        self.ram = [0; 0x0C00];
        self.sprite_ram = [0; 0x0400];
        self.scanline_buffer.fill(0);

        self.discrete.reset();
    }

    fn save_nvram(&self) -> Option<&[u8]> {
        None
    }

    fn load_nvram(&mut self, _data: &[u8]) {}

    fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        let n = buffer.len().min(self.audio_buffer.len());
        buffer[..n].copy_from_slice(&self.audio_buffer[..n]);
        self.audio_buffer.drain(..n);
        n
    }

    fn audio_sample_rate(&self) -> u32 {
        44100
    }

    fn frame_rate_hz(&self) -> f64 {
        CPU_CLOCK_HZ as f64 / CYCLES_PER_FRAME as f64
    }
}

/// Active-high bit manipulation: set bit on press, clear on release.
fn set_bit_active_high(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg |= 1 << bit;
    } else {
        *reg &= !(1 << bit);
    }
}

/// Darlington amplifier 3-bit resistor network (for R and G channels).
/// Resistors: 1kΩ, 470Ω, 220Ω with 470Ω pulldown.
fn darlington_3bit(bit0: f64, bit1: f64, bit2: f64) -> u8 {
    // Conductances
    let g0 = bit0 / 1000.0;
    let g1 = bit1 / 470.0;
    let g2 = bit2 / 220.0;
    let g_pull = 1.0 / 470.0;
    let total = g0 + g1 + g2 + g_pull;
    let active = g0 + g1 + g2;
    let voltage = if total > 0.0 { active / total } else { 0.0 };
    (voltage * 255.0).round().min(255.0) as u8
}

/// Emitter follower 2-bit resistor network (for B channel).
/// Resistors: 470Ω, 220Ω with 680Ω pulldown.
fn emitter_2bit(bit0: f64, bit1: f64) -> u8 {
    let g0 = bit0 / 470.0;
    let g1 = bit1 / 220.0;
    let g_pull = 1.0 / 680.0;
    let total = g0 + g1 + g_pull;
    let active = g0 + g1;
    let voltage = if total > 0.0 { active / total } else { 0.0 };
    (voltage * 255.0).round().min(255.0) as u8
}
