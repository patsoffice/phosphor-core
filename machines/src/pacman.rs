use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::state::Z80State;
use phosphor_core::cpu::z80::Z80;
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::namco_wsg::NamcoWsg;

use crate::rom_loader::{RomEntry, RomRegion};

// ---------------------------------------------------------------------------
// Pac-Man ROM definitions ("pacman" Midway set)
// ---------------------------------------------------------------------------

/// Program ROM: 16KB at 0x0000-0x3FFF (four 4KB chips).
pub static PACMAN_PROGRAM_ROM: RomRegion = RomRegion {
    size: 0x4000,
    entries: &[
        RomEntry {
            name: "pacman.6e",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0xc1e6ab10],
        },
        RomEntry {
            name: "pacman.6f",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0x1a6fb2d4],
        },
        RomEntry {
            name: "pacman.6h",
            size: 0x1000,
            offset: 0x2000,
            crc32: &[0xbcdd1beb],
        },
        RomEntry {
            name: "pacman.6j",
            size: 0x1000,
            offset: 0x3000,
            crc32: &[0x817d94e3],
        },
    ],
};

/// GFX ROM: 8KB (tiles at 0x0000-0x0FFF, sprites at 0x1000-0x1FFF).
pub static PACMAN_GFX_ROM: RomRegion = RomRegion {
    size: 0x2000,
    entries: &[
        RomEntry {
            name: "pacman.5e",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0x0c944964],
        },
        RomEntry {
            name: "pacman.5f",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0x958fedf9],
        },
    ],
};

/// Palette PROM (32 bytes) + color lookup table PROM (256 bytes).
pub static PACMAN_COLOR_PROMS: RomRegion = RomRegion {
    size: 0x0120,
    entries: &[
        RomEntry {
            name: "82s123.7f",
            size: 0x0020,
            offset: 0x0000,
            crc32: &[0x2fc650bd],
        },
        RomEntry {
            name: "82s126.4a",
            size: 0x0100,
            offset: 0x0020,
            crc32: &[0x3eb3a8e4],
        },
    ],
};

/// Sound waveform PROM (256 bytes — 8 waveforms × 32 samples × 4 bits).
pub static PACMAN_SOUND_PROM: RomRegion = RomRegion {
    size: 0x0100,
    entries: &[RomEntry {
        name: "82s126.1m",
        size: 0x0100,
        offset: 0x0000,
        crc32: &[0xa9cc86bf],
    }],
};

// ---------------------------------------------------------------------------
// Input button IDs
// ---------------------------------------------------------------------------
pub const INPUT_P1_UP: u8 = 0;
pub const INPUT_P1_LEFT: u8 = 1;
pub const INPUT_P1_RIGHT: u8 = 2;
pub const INPUT_P1_DOWN: u8 = 3;
pub const INPUT_COIN: u8 = 4;
pub const INPUT_P1_START: u8 = 5;
pub const INPUT_P2_START: u8 = 6;
pub const INPUT_P2_UP: u8 = 7;
pub const INPUT_P2_LEFT: u8 = 8;
pub const INPUT_P2_RIGHT: u8 = 9;
pub const INPUT_P2_DOWN: u8 = 10;

const PACMAN_INPUT_MAP: &[InputButton] = &[
    InputButton { id: INPUT_P1_UP, name: "P1 Up" },
    InputButton { id: INPUT_P1_LEFT, name: "P1 Left" },
    InputButton { id: INPUT_P1_RIGHT, name: "P1 Right" },
    InputButton { id: INPUT_P1_DOWN, name: "P1 Down" },
    InputButton { id: INPUT_COIN, name: "Coin" },
    InputButton { id: INPUT_P1_START, name: "P1 Start" },
    InputButton { id: INPUT_P2_START, name: "P2 Start" },
    InputButton { id: INPUT_P2_UP, name: "P2 Up" },
    InputButton { id: INPUT_P2_LEFT, name: "P2 Left" },
    InputButton { id: INPUT_P2_RIGHT, name: "P2 Right" },
    InputButton { id: INPUT_P2_DOWN, name: "P2 Down" },
];

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------
// Master clock:  18.432 MHz
// CPU clock:     18.432 / 6 = 3.072 MHz
// Pixel clock:   18.432 / 3 = 6.144 MHz
// HTOTAL:        384 pixels = 192 CPU cycles per scanline
// VTOTAL:        264 lines
// VBSTART:       224 (visible height)
// Frame:         192 × 264 = 50688 CPU cycles per frame
// Frame rate:    3072000 / 50688 ≈ 60.61 Hz

const CYCLES_PER_SCANLINE: u64 = 192;
const VISIBLE_LINES: u64 = 224;
const TOTAL_LINES: u64 = 264;
const CYCLES_PER_FRAME: u64 = TOTAL_LINES * CYCLES_PER_SCANLINE;

const CPU_CLOCK_HZ: u64 = 3_072_000;

// Screen dimensions: Pac-Man's native 288×224 is rotated 90° CCW
const SCREEN_WIDTH: u32 = 224;
const SCREEN_HEIGHT: u32 = 288;

// Resistor weights for palette PROM
// 3-bit RGB channels with 1K/470/220 ohm resistors
const R_WEIGHTS: [f64; 3] = [1000.0, 470.0, 220.0];
const G_WEIGHTS: [f64; 3] = [1000.0, 470.0, 220.0];
const B_WEIGHTS: [f64; 2] = [470.0, 220.0];

/// Pac-Man Arcade System (Namco/Midway, 1980)
///
/// Hardware: Zilog Z80 @ 3.072 MHz, Namco WSG 3-voice wavetable sound.
/// Video: 36×28 tile playfield + 8 sprites, 2bpp, PROM-based palette.
/// Screen: 288×224 displayed rotated 90° CCW on vertical monitor.
pub struct PacmanSystem {
    cpu: Z80,

    // Memory
    rom: [u8; 0x4000],         // 0x0000-0x3FFF: 16KB program ROM
    video_ram: [u8; 0x400],    // 0x4000-0x43FF: tile codes
    color_ram: [u8; 0x400],    // 0x4400-0x47FF: tile attributes
    ram: [u8; 0x400],          // 0x4C00-0x4FFF: work RAM + sprite attrs
    sprite_coords: [u8; 0x10], // 0x5060-0x506F: sprite X/Y positions

    // Sound
    wsg: NamcoWsg,

    // GFX ROM
    gfx_rom: [u8; 0x2000],

    // PROMs
    palette_prom: [u8; 32],
    color_lut_prom: [u8; 256],

    // Pre-computed palette (32 RGB entries from PROM resistor weighting)
    palette_rgb: [(u8, u8, u8); 32],

    // Scanline-rendered framebuffer (288 x 224 x RGB24 = 193,536 bytes).
    // Native orientation, populated incrementally during run_frame().
    scanline_buffer: Vec<u8>,

    // I/O state (active-low: 0xFF = all released)
    in0: u8,
    in1: u8,
    dip_switches: u8,

    // 74LS259 addressable latch outputs
    irq_enabled: bool,
    sound_enabled: bool,
    flip_screen: bool,

    // Interrupt
    interrupt_vector: u8,
    vblank_irq_pending: bool,

    // Timing
    clock: u64,
    watchdog_counter: u32,
}

impl PacmanSystem {
    pub fn new() -> Self {
        Self {
            cpu: Z80::new(),
            rom: [0; 0x4000],
            video_ram: [0; 0x400],
            color_ram: [0; 0x400],
            ram: [0; 0x400],
            sprite_coords: [0; 0x10],
            wsg: NamcoWsg::new(CPU_CLOCK_HZ),
            gfx_rom: [0; 0x2000],
            palette_prom: [0; 32],
            color_lut_prom: [0; 256],
            palette_rgb: [(0, 0, 0); 32],
            scanline_buffer: vec![0u8; 288 * 224 * 3],
            in0: 0xFF,
            in1: 0xFF,
            // Default DIP: 1 coin/1 credit, 3 lives, 10000 bonus, normal difficulty, normal ghosts
            dip_switches: 0xC9,
            irq_enabled: false,
            sound_enabled: false,
            flip_screen: false,
            interrupt_vector: 0,
            vblank_irq_pending: false,
            clock: 0,
            watchdog_counter: 0,
        }
    }

    /// Pre-compute the 32-entry RGB palette from the palette PROM using
    /// resistor-weighted DAC values.
    fn build_palette(&mut self) {
        // Compute resistor weights
        let r_scale = compute_resistor_scale(&R_WEIGHTS);
        let g_scale = compute_resistor_scale(&G_WEIGHTS);
        let b_scale = compute_resistor_scale(&B_WEIGHTS);

        for i in 0..32 {
            let entry = self.palette_prom[i];

            // Red: bits 0-2
            let r = combine_weights_3(
                &R_WEIGHTS, &r_scale,
                entry & 1, (entry >> 1) & 1, (entry >> 2) & 1,
            );
            // Green: bits 3-5
            let g = combine_weights_3(
                &G_WEIGHTS, &g_scale,
                (entry >> 3) & 1, (entry >> 4) & 1, (entry >> 5) & 1,
            );
            // Blue: bits 6-7
            let b = combine_weights_2(
                &B_WEIGHTS, &b_scale,
                (entry >> 6) & 1, (entry >> 7) & 1,
            );

            self.palette_rgb[i] = (r, g, b);
        }
    }

    pub fn tick(&mut self) {
        let frame_cycle = self.clock % CYCLES_PER_FRAME;

        // Per-scanline rendering: at each scanline boundary, render the current
        // scanline from VRAM + sprites before the CPU processes it, matching
        // hardware CRT read timing.
        if frame_cycle.is_multiple_of(CYCLES_PER_SCANLINE) {
            let scanline = (frame_cycle / CYCLES_PER_SCANLINE) as u16;
            if scanline < VISIBLE_LINES as u16 {
                self.render_scanline(scanline as usize);
            }
        }

        // VBLANK interrupt: fire at the start of VBLANK (scanline 224)
        let vblank_cycle = VISIBLE_LINES * CYCLES_PER_SCANLINE;
        if frame_cycle == vblank_cycle {
            self.vblank_irq_pending = true;
        }

        // WSG tick (runs at CPU clock rate)
        self.wsg.tick();

        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    /// Load all ROM sets from a RomSet.
    pub fn load_rom_set(
        &mut self,
        rom_set: &crate::rom_loader::RomSet,
    ) -> Result<(), crate::rom_loader::RomLoadError> {
        let rom_data = PACMAN_PROGRAM_ROM.load(rom_set)?;
        self.rom.copy_from_slice(&rom_data);

        let gfx_data = PACMAN_GFX_ROM.load(rom_set)?;
        self.gfx_rom.copy_from_slice(&gfx_data);

        let color_data = PACMAN_COLOR_PROMS.load(rom_set)?;
        self.palette_prom.copy_from_slice(&color_data[0..32]);
        self.color_lut_prom.copy_from_slice(&color_data[32..288]);

        let sound_data = PACMAN_SOUND_PROM.load(rom_set)?;
        self.wsg.load_waveform_rom(&sound_data);

        self.build_palette();
        Ok(())
    }

    pub fn get_cpu_state(&self) -> Z80State {
        self.cpu.snapshot()
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Decode a single tile pixel from the GFX ROM.
    /// Returns a 2-bit pixel value (0-3).
    ///
    /// Tile layout (planeoffset { 0, 4 }, MSB-first bit ordering):
    ///   8×8 pixels, 2 bits per pixel, 16 bytes per tile.
    ///   xoffset: { 8*8+0, 8*8+1, 8*8+2, 8*8+3, 0, 1, 2, 3 }
    ///   yoffset: { 0*8, 1*8, 2*8, 3*8, 4*8, 5*8, 6*8, 7*8 }
    ///
    /// Bit extraction uses 0x80 >> (bitnum % 8), i.e. MSB-first within each byte.
    /// Plane 0 (offset 0) maps to the HIGH bit of the 2-bit pixel value.
    fn decode_tile_pixel(&self, tile_code: u16, px: u8, py: u8) -> u8 {
        let base = (tile_code as usize) * 16;
        // Pixel X mapping: pixels 0-3 come from byte offset 8, pixels 4-7 from byte 0
        let (byte_off, bit) = if px < 4 {
            (8, px)    // First 4 pixels from second half
        } else {
            (0, px - 4) // Last 4 pixels from first half
        };
        let byte_addr = base + byte_off + py as usize;
        if byte_addr >= self.gfx_rom.len() {
            return 0;
        }
        let byte = self.gfx_rom[byte_addr];
        // MSB-first: layout bit N within a byte reads actual bit (7 - N)
        // Plane 0 (planeoffset=0, bits 7-4) → pixel bit 1 (high)
        // Plane 1 (planeoffset=4, bits 3-0) → pixel bit 0 (low)
        let plane_hi = (byte >> (7 - bit)) & 1;
        let plane_lo = (byte >> (3 - bit)) & 1;
        plane_lo | (plane_hi << 1)
    }

    /// Decode a single sprite pixel from the GFX ROM.
    /// Returns a 2-bit pixel value (0-3).
    ///
    /// Sprite layout (planeoffset { 0, 4 }, MSB-first bit ordering):
    ///   16×16 pixels, 2 bits per pixel, 64 bytes per sprite.
    ///   xoffset: { 8*8, 8*8+1, 8*8+2, 8*8+3, 16*8, 16*8+1, 16*8+2, 16*8+3,
    ///              24*8, 24*8+1, 24*8+2, 24*8+3, 0, 1, 2, 3 }
    ///   yoffset: { 0*8, 1*8, ..., 7*8, 32*8, 33*8, ..., 39*8 }
    fn decode_sprite_pixel(&self, sprite_code: u16, px: u8, py: u8) -> u8 {
        let base = 0x1000 + (sprite_code as usize) * 64;

        // X mapping: 4 groups of 4 pixels, each from different byte offsets
        let (x_byte_off, bit) = match px {
            0..=3   => (8, px),         // 8*8 + bit
            4..=7   => (16, px - 4),    // 16*8 + bit
            8..=11  => (24, px - 8),    // 24*8 + bit
            12..=15 => (0, px - 12),    // 0 + bit
            _ => unreachable!(),
        };

        // Y mapping: rows 0-7 at offset 0, rows 8-15 at offset 32
        let y_byte_off = if py < 8 { py as usize } else { 32 + (py as usize - 8) };

        let byte_addr = base + x_byte_off + y_byte_off;
        if byte_addr >= self.gfx_rom.len() {
            return 0;
        }
        let byte = self.gfx_rom[byte_addr];
        // MSB-first: layout bit N within a byte reads actual bit (7 - N)
        let plane_hi = (byte >> (7 - bit)) & 1;
        let plane_lo = (byte >> (3 - bit)) & 1;
        plane_lo | (plane_hi << 1)
    }

    /// Resolve a 2-bit pixel value to an RGB color using the palette system.
    ///
    /// The color lookup chain:
    ///   attribute (5 bits) → 4 entries in color_lut_prom → palette index → RGB
    fn resolve_color(&self, attribute: u8, pixel_value: u8) -> (u8, u8, u8) {
        let lut_index = ((attribute & 0x1F) as usize) * 4 + pixel_value as usize;
        let palette_index = if lut_index < 256 {
            (self.color_lut_prom[lut_index] & 0x0F) as usize
        } else {
            0
        };
        self.palette_rgb[palette_index]
    }

    /// Map a tile index in the 36×28 tilemap to a VRAM offset.
    ///
    /// The Pac-Man tilemap uses a non-linear address mapping:
    ///   row += 2; col -= 2;
    ///   if (col & 0x20) return row + ((col & 0x1f) << 5);
    ///   else return col + (row << 5);
    fn tilemap_offset(col: i32, row: i32) -> usize {
        let r = row + 2;
        let c = col - 2;
        if c & 0x20 != 0 {
            (r + ((c & 0x1F) << 5)) as usize
        } else {
            (c + (r << 5)) as usize
        }
    }

    /// Compute sprite transparency mask. Returns a 4-bit mask where bit N
    /// is set if pixel value N maps to palette index 0 (transparent)
    /// through the color LUT PROM.
    fn sprite_trans_mask(&self, attribute: u8) -> u8 {
        let base = (attribute as usize & 0x1F) * 4;
        let mut mask: u8 = 0;
        for pv in 0..4u8 {
            let lut_index = base + pv as usize;
            if (self.color_lut_prom[lut_index] & 0x0F) == 0 {
                mask |= 1 << pv;
            }
        }
        mask
    }

    /// Render a single scanline from current VRAM/sprite state into the scanline buffer.
    /// Composites tiles then sprites for native scanline Y (0-223).
    fn render_scanline(&mut self, scanline: usize) {
        let row_offset = scanline * 288 * 3;

        // Fill scanline with background color
        let bg = self.resolve_color(0, 0);
        for x in 0..288 {
            let off = row_offset + x * 3;
            self.scanline_buffer[off] = bg.0;
            self.scanline_buffer[off + 1] = bg.1;
            self.scanline_buffer[off + 2] = bg.2;
        }

        // Tiles: determine which tile row and pixel row within tile
        let tile_row = (scanline / 8) as i32;
        let py = (scanline % 8) as u8;
        for tile_col in 0..36i32 {
            let offset = Self::tilemap_offset(tile_col, tile_row);
            let tile_code = if offset < 0x400 {
                self.video_ram[offset] as u16
            } else {
                0
            };
            let attribute = if offset < 0x400 {
                self.color_ram[offset]
            } else {
                0
            };
            let screen_x = (tile_col * 8) as usize;

            for px in 0..8u8 {
                let nx = screen_x + px as usize;
                let pixel_value = self.decode_tile_pixel(tile_code, px, py);
                let (r, g, b) = self.resolve_color(attribute, pixel_value);
                let off = row_offset + nx * 3;
                self.scanline_buffer[off] = r;
                self.scanline_buffer[off + 1] = g;
                self.scanline_buffer[off + 2] = b;
            }
        }

        // Sprites: draw in priority order (7→3, then 2→0 with +1 Y offset)
        let clip_x_min = 16i32;
        let clip_x_max = 272i32;
        let y = scanline as i32;

        for pass in 0..2 {
            let (start, end, y_offset): (usize, usize, i32) = if pass == 0 {
                (7, 3, 0)
            } else {
                (2, 0, 1)
            };

            let mut offs = start;
            loop {
                let attr_base = 0x3F0 + offs * 2;
                let coord_base = offs * 2;

                let sprite_byte0 = self.ram[attr_base];
                let sprite_byte1 = self.ram[attr_base + 1];

                let sprite_code = (sprite_byte0 >> 2) as u16;
                let x_flip = (sprite_byte0 & 1) != 0;
                let y_flip = (sprite_byte0 & 2) != 0;
                let attribute = sprite_byte1 & 0x1F;

                let sx = 272i32 - self.sprite_coords[coord_base + 1] as i32;
                let sy = self.sprite_coords[coord_base] as i32 - 31 + y_offset;

                // Check if this scanline intersects the sprite's 16-pixel height
                if y >= sy && y < sy + 16 {
                    let trans_mask = self.sprite_trans_mask(attribute);
                    let spy = (y - sy) as u8;
                    let src_py = if y_flip { 15 - spy } else { spy };

                    // Draw sprite row at primary position
                    for px in 0..16u8 {
                        let draw_x = sx + px as i32;
                        if draw_x < clip_x_min || draw_x >= clip_x_max {
                            continue;
                        }
                        let src_px = if x_flip { 15 - px } else { px };
                        let pixel_value =
                            self.decode_sprite_pixel(sprite_code, src_px, src_py);
                        if (trans_mask >> pixel_value) & 1 != 0 {
                            continue;
                        }
                        let (r, g, b) = self.resolve_color(attribute, pixel_value);
                        let off = row_offset + draw_x as usize * 3;
                        self.scanline_buffer[off] = r;
                        self.scanline_buffer[off + 1] = g;
                        self.scanline_buffer[off + 2] = b;
                    }

                    // Draw with X-256 wraparound (tunnel effect)
                    let sx_wrap = sx - 256;
                    if sx_wrap + 16 > clip_x_min && sx_wrap < clip_x_max {
                        for px in 0..16u8 {
                            let draw_x = sx_wrap + px as i32;
                            if draw_x < clip_x_min || draw_x >= clip_x_max {
                                continue;
                            }
                            let src_px = if x_flip { 15 - px } else { px };
                            let pixel_value =
                                self.decode_sprite_pixel(sprite_code, src_px, src_py);
                            if (trans_mask >> pixel_value) & 1 != 0 {
                                continue;
                            }
                            let (r, g, b) = self.resolve_color(attribute, pixel_value);
                            let off = row_offset + draw_x as usize * 3;
                            self.scanline_buffer[off] = r;
                            self.scanline_buffer[off + 1] = g;
                            self.scanline_buffer[off + 2] = b;
                        }
                    }
                }

                if offs == end {
                    break;
                }
                offs -= 1;
            }
        }
    }

}

impl Default for PacmanSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for PacmanSystem {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        // A15 not connected: 0x8000-0xFFFF mirrors 0x0000-0x7FFF
        let addr = addr & 0x7FFF;

        match addr {
            // Program ROM
            0x0000..=0x3FFF => self.rom[addr as usize],

            // Video RAM
            0x4000..=0x43FF => self.video_ram[(addr - 0x4000) as usize],

            // Color RAM
            0x4400..=0x47FF => self.color_ram[(addr - 0x4400) as usize],

            // Bus float (no device responds — Pac-Man has a bug that writes here)
            0x4800..=0x4BFF => 0xBF,

            // Work RAM (includes sprite attribute RAM at 0x4FF0-0x4FFF)
            0x4C00..=0x4FFF => self.ram[(addr - 0x4C00) as usize],

            // IN0: P1 joystick + coins (active-low)
            0x5000..=0x503F => self.in0,

            // IN1: P2 joystick + start buttons + cabinet (active-low)
            0x5040..=0x507F => self.in1,

            // DSW1: DIP switches
            0x5080..=0x50BF => self.dip_switches,

            // DSW2 (unused on standard Pac-Man)
            0x50C0..=0x50FF => 0xFF,

            _ => 0xFF,
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        let addr = addr & 0x7FFF;

        match addr {
            // Video RAM
            0x4000..=0x43FF => self.video_ram[(addr - 0x4000) as usize] = data,

            // Color RAM
            0x4400..=0x47FF => self.color_ram[(addr - 0x4400) as usize] = data,

            // Work RAM (includes sprite attribute RAM)
            0x4C00..=0x4FFF => self.ram[(addr - 0x4C00) as usize] = data,

            // 74LS259 addressable latch: address bits 0-2 select output, data bit 0 is value
            0x5000..=0x5007 => {
                let bit = (addr & 7) as u8;
                let value = (data & 1) != 0;
                match bit {
                    0 => {
                        self.irq_enabled = value;
                        if !value {
                            self.vblank_irq_pending = false;
                        }
                    }
                    1 => {
                        self.sound_enabled = value;
                        self.wsg.set_sound_enabled(value);
                    }
                    3 => self.flip_screen = value,
                    // 2: unused, 4-5: LEDs (not connected), 6: coin lockout, 7: coin counter
                    _ => {}
                }
            }

            // Namco WSG sound registers (32 nibble registers)
            0x5040..=0x505F => self.wsg.write((addr - 0x5040) as u8, data),

            // Sprite coordinates
            0x5060..=0x506F => self.sprite_coords[(addr - 0x5060) as usize] = data,

            // Watchdog reset
            0x50C0..=0x50FF => self.watchdog_counter = 0,

            _ => { /* ROM or unmapped: ignored */ }
        }
    }

    fn io_read(&mut self, _master: BusMaster, _addr: u16) -> u8 {
        0xFF // No I/O read ports used on Pac-Man
    }

    fn io_write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        // Port 0x00: set interrupt vector byte (used by Z80 IM2)
        if addr & 0xFF == 0x00 {
            self.interrupt_vector = data;
        }
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false // No DMA hardware on Pac-Man
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            irq: self.vblank_irq_pending && self.irq_enabled,
            firq: false,
            irq_vector: self.interrupt_vector,
        }
    }
}

impl Machine for PacmanSystem {
    fn display_size(&self) -> (u32, u32) {
        (SCREEN_WIDTH, SCREEN_HEIGHT) // 224×288 (rotated)
    }

    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        // Rotate 90° CCW from native scanline_buffer (288w × 224h)
        // to output buffer (224w × 288h).
        // Native pixel (nx, ny) → output pixel (223 - ny, nx)
        let out_w = SCREEN_WIDTH as usize; // 224
        for oy in 0..SCREEN_HEIGHT as usize {
            for ox in 0..out_w {
                let nx = oy;
                let ny = 223 - ox;
                let src = (ny * 288 + nx) * 3;
                let dst = (oy * out_w + ox) * 3;
                buffer[dst] = self.scanline_buffer[src];
                buffer[dst + 1] = self.scanline_buffer[src + 1];
                buffer[dst + 2] = self.scanline_buffer[src + 2];
            }
        }
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // IN0 (active-low: clear bit when pressed, set when released)
            INPUT_P1_UP    => set_bit_active_low(&mut self.in0, 0, pressed),
            INPUT_P1_LEFT  => set_bit_active_low(&mut self.in0, 1, pressed),
            INPUT_P1_RIGHT => set_bit_active_low(&mut self.in0, 2, pressed),
            INPUT_P1_DOWN  => set_bit_active_low(&mut self.in0, 3, pressed),
            INPUT_COIN     => set_bit_active_low(&mut self.in0, 5, pressed),

            // IN1 (active-low)
            INPUT_P2_UP    => set_bit_active_low(&mut self.in1, 0, pressed),
            INPUT_P2_LEFT  => set_bit_active_low(&mut self.in1, 1, pressed),
            INPUT_P2_RIGHT => set_bit_active_low(&mut self.in1, 2, pressed),
            INPUT_P2_DOWN  => set_bit_active_low(&mut self.in1, 3, pressed),
            INPUT_P1_START => set_bit_active_low(&mut self.in1, 5, pressed),
            INPUT_P2_START => set_bit_active_low(&mut self.in1, 6, pressed),

            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        PACMAN_INPUT_MAP
    }

    fn reset(&mut self) {
        self.cpu.reset();
        self.wsg.reset();
        self.irq_enabled = false;
        self.sound_enabled = false;
        self.flip_screen = false;
        self.interrupt_vector = 0;
        self.vblank_irq_pending = false;
        self.clock = 0;
        self.watchdog_counter = 0;
        self.in0 = 0xFF;
        self.in1 = 0xFF;
        self.video_ram = [0; 0x400];
        self.color_ram = [0; 0x400];
        self.ram = [0; 0x400];
        self.sprite_coords = [0; 0x10];
        self.scanline_buffer.fill(0);
        // ROM, GFX, PROMs, and palette_rgb are NOT cleared (loaded from ROM set)

        // Z80 reset: PC starts at 0x0000, fetching the first ROM instruction
        self.cpu.pc = 0x0000;
    }

    fn save_nvram(&self) -> Option<&[u8]> {
        None // No battery-backed RAM on Pac-Man
    }

    fn load_nvram(&mut self, _data: &[u8]) {}

    fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        self.wsg.fill_audio(buffer)
    }

    fn audio_sample_rate(&self) -> u32 {
        44100
    }

    fn frame_rate_hz(&self) -> f64 {
        CPU_CLOCK_HZ as f64 / CYCLES_PER_FRAME as f64
    }
}

/// Active-low bit manipulation: clear bit on press, set bit on release.
fn set_bit_active_low(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg &= !(1 << bit);
    } else {
        *reg |= 1 << bit;
    }
}

/// Compute normalization scale factors for resistor-weighted DAC.
fn compute_resistor_scale(weights: &[f64]) -> Vec<f64> {
    // Total conductance when all bits are set
    let total: f64 = weights.iter().map(|w| 1.0 / w).sum();
    weights.iter().map(|w| (1.0 / w) / total).collect()
}

/// Combine 3 resistor-weighted bits into an 8-bit color value.
fn combine_weights_3(
    _weights: &[f64; 3], scale: &[f64],
    bit0: u8, bit1: u8, bit2: u8,
) -> u8 {
    let val = bit0 as f64 * scale[0] + bit1 as f64 * scale[1] + bit2 as f64 * scale[2];
    (val * 255.0).round().min(255.0) as u8
}

/// Combine 2 resistor-weighted bits into an 8-bit color value.
fn combine_weights_2(
    _weights: &[f64; 2], scale: &[f64],
    bit0: u8, bit1: u8,
) -> u8 {
    let val = bit0 as f64 * scale[0] + bit1 as f64 * scale[1];
    (val * 255.0).round().min(255.0) as u8
}
