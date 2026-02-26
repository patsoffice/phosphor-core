use phosphor_core::bus_split;
use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::debug::BusDebug;
use phosphor_core::core::machine::{
    AudioSource, InputButton, InputReceiver, Machine, MachineDebug, Renderable,
};
use phosphor_core::core::memory_map::{AccessKind, MemoryMap};
use phosphor_core::core::save_state::{self, SaveError, Saveable, StateWriter};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::state::Z80State;
use phosphor_core::cpu::z80::Z80;
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::namco_wsg::NamcoWsg;
use phosphor_core::gfx;
use phosphor_macros::BusDebug;

use crate::registry::MachineEntry;
use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};
use crate::set_bit_active_low;

mod region {
    pub const ROM: u8 = 1;
    pub const VIDEORAM: u8 = 2;
    pub const COLORRAM: u8 = 3;
    pub const RAM: u8 = 4;
    pub const IO: u8 = 5;
}

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
    InputButton {
        id: INPUT_P1_UP,
        name: "P1 Up",
    },
    InputButton {
        id: INPUT_P1_LEFT,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_P1_RIGHT,
        name: "P1 Right",
    },
    InputButton {
        id: INPUT_P1_DOWN,
        name: "P1 Down",
    },
    InputButton {
        id: INPUT_COIN,
        name: "Coin",
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
        id: INPUT_P2_UP,
        name: "P2 Up",
    },
    InputButton {
        id: INPUT_P2_LEFT,
        name: "P2 Left",
    },
    InputButton {
        id: INPUT_P2_RIGHT,
        name: "P2 Right",
    },
    InputButton {
        id: INPUT_P2_DOWN,
        name: "P2 Down",
    },
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
#[derive(BusDebug)]
pub struct PacmanSystem {
    #[debug_cpu("Z80")]
    cpu: Z80,

    #[debug_map(cpu = 0)]
    map: MemoryMap,

    sprite_coords: [u8; 0x10], // 0x5060-0x506F: sprite X/Y positions (write-only from bus)

    // Sound
    #[debug_device("NamcoWSG")]
    wsg: NamcoWsg,

    // Pre-decoded GFX caches (from GFX ROM)
    tile_cache: gfx::GfxCache,
    sprite_cache: gfx::GfxCache,

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
            map: Self::build_map(),
            sprite_coords: [0; 0x10],
            wsg: NamcoWsg::new(CPU_CLOCK_HZ),
            tile_cache: gfx::GfxCache::new(256, 8, 8),
            sprite_cache: gfx::GfxCache::new(64, 16, 16),
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

    fn build_map() -> MemoryMap {
        use region::*;
        let mut map = MemoryMap::new();
        map.region(ROM, "Program ROM", 0x0000, 0x4000, AccessKind::ReadOnly)
            .region(VIDEORAM, "Video RAM", 0x4000, 0x0400, AccessKind::ReadWrite)
            .region(COLORRAM, "Color RAM", 0x4400, 0x0400, AccessKind::ReadWrite)
            .region(RAM, "RAM", 0x4C00, 0x0400, AccessKind::ReadWrite)
            .region(IO, "I/O", 0x5000, 0x0100, AccessKind::Io);
        map
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
                &R_WEIGHTS,
                &r_scale,
                entry & 1,
                (entry >> 1) & 1,
                (entry >> 2) & 1,
            );
            // Green: bits 3-5
            let g = combine_weights_3(
                &G_WEIGHTS,
                &g_scale,
                (entry >> 3) & 1,
                (entry >> 4) & 1,
                (entry >> 5) & 1,
            );
            // Blue: bits 6-7
            let b = combine_weights_2(&B_WEIGHTS, &b_scale, (entry >> 6) & 1, (entry >> 7) & 1);

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

        bus_split!(self, bus => {
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        });

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    /// Load all ROM sets from a RomSet.
    pub fn load_rom_set(
        &mut self,
        rom_set: &crate::rom_loader::RomSet,
    ) -> Result<(), crate::rom_loader::RomLoadError> {
        let rom_data = PACMAN_PROGRAM_ROM.load(rom_set)?;
        self.map.load_region(region::ROM, &rom_data);

        let gfx_data = PACMAN_GFX_ROM.load(rom_set)?;
        self.tile_cache = gfx::decode::decode_pacman_tiles(&gfx_data, 0x0000, 256);
        self.sprite_cache = gfx::decode::decode_pacman_sprites(&gfx_data, 0x1000, 64);

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

    /// Render a single scanline from current VRAM/sprite state into the scanline buffer.
    /// Composites tiles then sprites for native scanline Y (0-223).
    fn render_scanline(&mut self, scanline: usize) {
        let row_offset = scanline * 288 * 3;

        // Split borrows: immutable refs for closures, mutable ref for buffer
        let video_ram = self.map.region_data(region::VIDEORAM);
        let color_ram = self.map.region_data(region::COLORRAM);
        let color_lut_prom = &self.color_lut_prom;
        let palette_rgb = &self.palette_rgb;
        let tile_cache = &self.tile_cache;
        let sprite_cache = &self.sprite_cache;
        let buf = &mut self.scanline_buffer[row_offset..row_offset + 288 * 3];

        // Inline color resolution (captures split borrows, not &self)
        let resolve = |attribute: u8, pixel_value: u8| -> (u8, u8, u8) {
            let lut_index = ((attribute & 0x1F) as usize) * 4 + pixel_value as usize;
            let palette_index = if lut_index < 256 {
                (color_lut_prom[lut_index] & 0x0F) as usize
            } else {
                0
            };
            palette_rgb[palette_index]
        };

        // Fill scanline with background color
        let bg = resolve(0, 0);
        for x in 0..288 {
            let off = x * 3;
            buf[off] = bg.0;
            buf[off + 1] = bg.1;
            buf[off + 2] = bg.2;
        }

        // Tiles: use shared tilemap renderer
        let config = gfx::TilemapConfig {
            cols: 36,
            rows: 28,
            tile_width: 8,
            tile_height: 8,
        };

        gfx::tilemap::render_tilemap_scanline(
            &config,
            tile_cache,
            scanline,
            |col, row| {
                let offset = Self::tilemap_offset(col as i32, row as i32);
                let tile_code = if offset < 0x400 {
                    video_ram[offset] as u16
                } else {
                    0
                };
                let attribute = if offset < 0x400 { color_ram[offset] } else { 0 };
                (tile_code, attribute)
            },
            resolve,
            buf,
            0,
        );

        // Sprites: draw in priority order (7→3, then 2→0 with +1 Y offset)
        let ram = self.map.region_data(region::RAM);
        let sprite_coords = &self.sprite_coords;
        let y = scanline as i32;

        for pass in 0..2 {
            let (start, end, y_offset): (usize, usize, i32) =
                if pass == 0 { (7, 3, 0) } else { (2, 0, 1) };

            let mut offs = start;
            loop {
                let attr_base = 0x3F0 + offs * 2;
                let coord_base = offs * 2;

                let sprite_byte0 = ram[attr_base];
                let sprite_byte1 = ram[attr_base + 1];

                let sprite_code = (sprite_byte0 >> 2) as u16;
                let x_flip = (sprite_byte0 & 1) != 0;
                let y_flip = (sprite_byte0 & 2) != 0;
                let attribute = sprite_byte1 & 0x1F;

                let sx = 272i32 - sprite_coords[coord_base + 1] as i32;
                let sy = sprite_coords[coord_base] as i32 - 31 + y_offset;

                if y >= sy && y < sy + 16 {
                    let spy = (y - sy) as u8;
                    let src_py = if y_flip { 15 - spy } else { spy };

                    // Pre-compute transparency mask for this sprite's attribute
                    let trans_base = (attribute as usize & 0x1F) * 4;
                    let mut trans_mask: u8 = 0;
                    for pv in 0..4u8 {
                        if (color_lut_prom[trans_base + pv as usize] & 0x0F) == 0 {
                            trans_mask |= 1 << pv;
                        }
                    }

                    let clip = gfx::sprite::SpriteClip {
                        x_min: 16,
                        x_max: 272,
                        wrap_offset: Some(-256), // tunnel wraparound
                    };
                    gfx::sprite::draw_sprite_row(
                        sprite_cache,
                        sprite_code,
                        src_py as usize,
                        sx,
                        x_flip,
                        |pv| (trans_mask >> pv) & 1 != 0,
                        |pv| resolve(attribute, pv),
                        buf,
                        &clip,
                    );
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

        let data = match self.map.page(addr).region_id {
            region::ROM | region::VIDEORAM | region::COLORRAM | region::RAM => {
                self.map.read_backing(addr)
            }

            region::IO => match addr {
                0x5000..=0x503F => self.in0,
                0x5040..=0x507F => self.in1,
                0x5080..=0x50BF => self.dip_switches,
                _ => 0xFF,
            },

            _ => {
                // Bus float at 0x4800-0x4BFF (no device responds)
                if (0x4800..0x4C00).contains(&addr) {
                    0xBF
                } else {
                    0xFF
                }
            }
        };

        self.map.check_read_watch(addr, data);
        data
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        let addr = addr & 0x7FFF;
        self.map.check_write_watch(addr, data);

        match self.map.page(addr).region_id {
            region::VIDEORAM | region::COLORRAM | region::RAM => {
                self.map.write_backing(addr, data);
            }

            region::IO => match addr {
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

                _ => {}
            },

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

impl Renderable for PacmanSystem {
    fn display_size(&self) -> (u32, u32) {
        (SCREEN_WIDTH, SCREEN_HEIGHT) // 224×288 (rotated)
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
}

impl AudioSource for PacmanSystem {
    fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        self.wsg.fill_audio(buffer)
    }

    fn audio_sample_rate(&self) -> u32 {
        44100
    }
}

impl InputReceiver for PacmanSystem {
    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // IN0 (active-low: clear bit when pressed, set when released)
            INPUT_P1_UP => set_bit_active_low(&mut self.in0, 0, pressed),
            INPUT_P1_LEFT => set_bit_active_low(&mut self.in0, 1, pressed),
            INPUT_P1_RIGHT => set_bit_active_low(&mut self.in0, 2, pressed),
            INPUT_P1_DOWN => set_bit_active_low(&mut self.in0, 3, pressed),
            INPUT_COIN => set_bit_active_low(&mut self.in0, 5, pressed),

            // IN1 (active-low)
            INPUT_P2_UP => set_bit_active_low(&mut self.in1, 0, pressed),
            INPUT_P2_LEFT => set_bit_active_low(&mut self.in1, 1, pressed),
            INPUT_P2_RIGHT => set_bit_active_low(&mut self.in1, 2, pressed),
            INPUT_P2_DOWN => set_bit_active_low(&mut self.in1, 3, pressed),
            INPUT_P1_START => set_bit_active_low(&mut self.in1, 5, pressed),
            INPUT_P2_START => set_bit_active_low(&mut self.in1, 6, pressed),

            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        PACMAN_INPUT_MAP
    }
}

impl MachineDebug for PacmanSystem {
    fn debug_bus(&self) -> Option<&dyn BusDebug> {
        Some(self)
    }

    fn debug_bus_mut(&mut self) -> Option<&mut dyn BusDebug> {
        Some(self)
    }

    fn cycles_per_frame(&self) -> u64 {
        CYCLES_PER_FRAME
    }

    fn debug_tick(&mut self) -> u32 {
        self.tick();
        if self.cpu.at_instruction_boundary() {
            1
        } else {
            0
        }
    }
}

impl Machine for PacmanSystem {
    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }
    }

    fn reset(&mut self) {
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
        self.map.region_data_mut(region::VIDEORAM).fill(0);
        self.map.region_data_mut(region::COLORRAM).fill(0);
        self.map.region_data_mut(region::RAM).fill(0);
        self.sprite_coords = [0; 0x10];
        self.scanline_buffer.fill(0);
        // ROM, GFX, PROMs, and palette_rgb are NOT cleared (loaded from ROM set)

        bus_split!(self, bus => {
            self.cpu.reset(bus, BusMaster::Cpu(0));
        });
    }

    fn frame_rate_hz(&self) -> f64 {
        CPU_CLOCK_HZ as f64 / CYCLES_PER_FRAME as f64
    }

    fn machine_id(&self) -> &str {
        "pacman"
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        let mut w = StateWriter::new();
        save_state::write_header(&mut w, self.machine_id());
        self.cpu.save_state(&mut w);
        w.write_bytes(self.map.region_data(region::VIDEORAM));
        w.write_bytes(self.map.region_data(region::COLORRAM));
        w.write_bytes(self.map.region_data(region::RAM));
        w.write_bytes(&self.sprite_coords);
        self.wsg.save_state(&mut w);
        w.write_u8(self.in0);
        w.write_u8(self.in1);
        w.write_bool(self.irq_enabled);
        w.write_bool(self.sound_enabled);
        w.write_bool(self.flip_screen);
        w.write_u8(self.interrupt_vector);
        w.write_bool(self.vblank_irq_pending);
        w.write_u64_le(self.clock);
        w.write_u32_le(self.watchdog_counter);
        Some(w.into_vec())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), SaveError> {
        let mut r = save_state::read_header(data, self.machine_id())?;
        self.cpu.load_state(&mut r)?;
        r.read_bytes_into(self.map.region_data_mut(region::VIDEORAM))?;
        r.read_bytes_into(self.map.region_data_mut(region::COLORRAM))?;
        r.read_bytes_into(self.map.region_data_mut(region::RAM))?;
        r.read_bytes_into(&mut self.sprite_coords)?;
        self.wsg.load_state(&mut r)?;
        self.in0 = r.read_u8()?;
        self.in1 = r.read_u8()?;
        self.irq_enabled = r.read_bool()?;
        self.sound_enabled = r.read_bool()?;
        self.flip_screen = r.read_bool()?;
        self.interrupt_vector = r.read_u8()?;
        self.vblank_irq_pending = r.read_bool()?;
        self.clock = r.read_u64_le()?;
        self.watchdog_counter = r.read_u32_le()?;
        Ok(())
    }
}

/// Compute normalization scale factors for resistor-weighted DAC.
fn compute_resistor_scale(weights: &[f64]) -> Vec<f64> {
    // Total conductance when all bits are set
    let total: f64 = weights.iter().map(|w| 1.0 / w).sum();
    weights.iter().map(|w| (1.0 / w) / total).collect()
}

/// Combine 3 resistor-weighted bits into an 8-bit color value.
fn combine_weights_3(_weights: &[f64; 3], scale: &[f64], bit0: u8, bit1: u8, bit2: u8) -> u8 {
    let val = bit0 as f64 * scale[0] + bit1 as f64 * scale[1] + bit2 as f64 * scale[2];
    (val * 255.0).round().min(255.0) as u8
}

/// Combine 2 resistor-weighted bits into an 8-bit color value.
fn combine_weights_2(_weights: &[f64; 2], scale: &[f64], bit0: u8, bit1: u8) -> u8 {
    let val = bit0 as f64 * scale[0] + bit1 as f64 * scale[1];
    (val * 255.0).round().min(255.0) as u8
}

// ---------------------------------------------------------------------------
// Machine registry
// ---------------------------------------------------------------------------

fn create_machine(
    rom_set: &RomSet,
) -> Result<Box<dyn phosphor_core::core::machine::Machine>, RomLoadError> {
    let mut sys = PacmanSystem::new();
    sys.load_rom_set(rom_set)?;
    Ok(Box::new(sys))
}

inventory::submit! {
    MachineEntry::new("pacman", "pacman", create_machine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::core::machine::Machine;
    use phosphor_core::cpu::CpuStateTrait;

    #[test]
    fn save_load_round_trip() {
        let mut sys = PacmanSystem::new();

        // Set known state
        sys.map.region_data_mut(region::VIDEORAM)[0x100] = 0xAA;
        sys.map.region_data_mut(region::COLORRAM)[0x200] = 0xBB;
        sys.map.region_data_mut(region::RAM)[0x300] = 0xCC;
        sys.sprite_coords[5] = 0xDD;
        sys.in0 = 0xEE;
        sys.in1 = 0x77;
        sys.irq_enabled = true;
        sys.sound_enabled = true;
        sys.flip_screen = true;
        sys.interrupt_vector = 0xCF;
        sys.vblank_irq_pending = true;
        sys.clock = 100_000;
        sys.watchdog_counter = 99;

        // Save
        let data = sys.save_state().expect("save_state should return Some");
        let cpu_snap = sys.cpu.snapshot();

        // Mutate everything
        let mut sys2 = PacmanSystem::new();
        sys2.map.region_data_mut(region::VIDEORAM)[0x100] = 0xFF;
        sys2.in0 = 0x00;
        sys2.clock = 999;

        // Load
        sys2.load_state(&data).unwrap();

        // Verify CPU
        assert_eq!(sys2.cpu.snapshot(), cpu_snap);

        // Verify memory
        assert_eq!(sys2.map.region_data(region::VIDEORAM)[0x100], 0xAA);
        assert_eq!(sys2.map.region_data(region::COLORRAM)[0x200], 0xBB);
        assert_eq!(sys2.map.region_data(region::RAM)[0x300], 0xCC);
        assert_eq!(sys2.sprite_coords[5], 0xDD);

        // Verify I/O and control state
        assert_eq!(sys2.in0, 0xEE);
        assert_eq!(sys2.in1, 0x77);
        assert!(sys2.irq_enabled);
        assert!(sys2.sound_enabled);
        assert!(sys2.flip_screen);
        assert_eq!(sys2.interrupt_vector, 0xCF);
        assert!(sys2.vblank_irq_pending);
        assert_eq!(sys2.clock, 100_000);
        assert_eq!(sys2.watchdog_counter, 99);
    }

    #[test]
    fn save_load_machine_id_validated() {
        let sys = PacmanSystem::new();
        let data = sys.save_state().unwrap();

        // Tamper with the machine ID
        let mut bad = data.clone();
        let id_offset = 4 + 4 + 4; // magic(4) + version(4) + id_len(4)
        bad[id_offset..id_offset + 6].copy_from_slice(b"xxxxxx");

        let mut sys2 = PacmanSystem::new();
        let result = sys2.load_state(&bad);
        assert!(result.is_err(), "should reject mismatched machine ID");
    }

    #[test]
    fn save_does_not_include_rom() {
        let mut sys = PacmanSystem::new();
        sys.map.region_data_mut(region::ROM)[0] = 0xDE;
        sys.tile_cache.set_pixel(0, 0, 0, 3);

        let data = sys.save_state().unwrap();

        // Load into a fresh system (ROMs are zeroed)
        let mut sys2 = PacmanSystem::new();
        sys2.load_state(&data).unwrap();

        // ROMs and GFX caches should remain at their default, not overwritten
        assert_eq!(sys2.map.region_data(region::ROM)[0], 0x00);
        assert_eq!(sys2.tile_cache.pixel(0, 0, 0), 0);
    }
}
