use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::m6809::M6809;
use phosphor_core::cpu::state::M6809State;
use phosphor_core::cpu::{Cpu, CpuStateTrait};

use crate::registry::MachineEntry;
use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};

// ---------------------------------------------------------------------------
// Gridlee ROM definitions
// ---------------------------------------------------------------------------
// Gridlee ROMs are freely distributable — original authors (Howard Delman,
// Roger Hector, Ed Rotberg) explicitly allowed distribution.

/// Program ROM: 24KB at 0xA000-0xFFFF (six 4KB chips).
pub static GRIDLEE_PROGRAM_ROM: RomRegion = RomRegion {
    size: 0x6000,
    entries: &[
        RomEntry {
            name: "gridfnla.bin",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0x1c43539e],
        },
        RomEntry {
            name: "gridfnlb.bin",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0xc48b91b8],
        },
        RomEntry {
            name: "gridfnlc.bin",
            size: 0x1000,
            offset: 0x2000,
            crc32: &[0x6ad436dd],
        },
        RomEntry {
            name: "gridfnld.bin",
            size: 0x1000,
            offset: 0x3000,
            crc32: &[0xf7188ddb],
        },
        RomEntry {
            name: "gridfnle.bin",
            size: 0x1000,
            offset: 0x4000,
            crc32: &[0xd5330bee],
        },
        RomEntry {
            name: "gridfnlf.bin",
            size: 0x1000,
            offset: 0x5000,
            crc32: &[0x695d16a3],
        },
    ],
};

/// Sprite/graphics ROM: 16KB (four 4KB chips).
/// Each sprite is 8x16 pixels, 64 bytes (4 bytes/row, packed 2 pixels/byte).
pub static GRIDLEE_GFX_ROM: RomRegion = RomRegion {
    size: 0x4000,
    entries: &[
        RomEntry {
            name: "gridpix0.bin",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0xe6ea15ae],
        },
        RomEntry {
            name: "gridpix1.bin",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0xd722f459],
        },
        RomEntry {
            name: "gridpix2.bin",
            size: 0x1000,
            offset: 0x2000,
            crc32: &[0x1e99143c],
        },
        RomEntry {
            name: "gridpix3.bin",
            size: 0x1000,
            offset: 0x3000,
            crc32: &[0x274342a0],
        },
    ],
};

/// Color PROMs: 3x2KB (R, G, B channels, 4-bit per channel).
/// 2048 palette entries = 64 banks x 32 colors.
pub static GRIDLEE_COLOR_PROMS: RomRegion = RomRegion {
    size: 0x1800,
    entries: &[
        RomEntry {
            name: "grdrprom.bin",
            size: 0x0800,
            offset: 0x0000,
            crc32: &[0xf28f87ed],
        },
        RomEntry {
            name: "grdgprom.bin",
            size: 0x0800,
            offset: 0x0800,
            crc32: &[0x921b0328],
        },
        RomEntry {
            name: "grdbprom.bin",
            size: 0x0800,
            offset: 0x1000,
            crc32: &[0x04350348],
        },
    ],
};

// ---------------------------------------------------------------------------
// Input button IDs
// ---------------------------------------------------------------------------
const INPUT_TRACK_U: u8 = 0;
const INPUT_TRACK_D: u8 = 1;
const INPUT_TRACK_L: u8 = 2;
const INPUT_TRACK_R: u8 = 3;
const INPUT_P1_FIRE: u8 = 4;
const INPUT_COIN: u8 = 5;
const INPUT_START1: u8 = 6;
const INPUT_START2: u8 = 7;

const GRIDLEE_INPUT_MAP: &[InputButton] = &[
    InputButton {
        id: INPUT_TRACK_U,
        name: "P1 Up",
    },
    InputButton {
        id: INPUT_TRACK_D,
        name: "P1 Down",
    },
    InputButton {
        id: INPUT_TRACK_L,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_TRACK_R,
        name: "P1 Right",
    },
    InputButton {
        id: INPUT_P1_FIRE,
        name: "P1 Fire",
    },
    InputButton {
        id: INPUT_COIN,
        name: "Coin",
    },
    InputButton {
        id: INPUT_START1,
        name: "P1 Start",
    },
    InputButton {
        id: INPUT_START2,
        name: "P2 Start",
    },
];

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------
// Master clock: 20 MHz XTAL
// CPU clock: 20 MHz / 16 = 1.25 MHz
// Pixel clock: 20 MHz / 4 = 5 MHz
// HTOTAL: 320 pixel clocks → 320/4 = 80 CPU cycles per scanline
// VTOTAL: 264 scanlines per frame
// Active: 256x240 pixels (VBEND=16, VBSTART=256)
// Frame rate: 1,250,000 / (80 * 264) ≈ 59.185 Hz
const CPU_CLOCK_HZ: u64 = 1_250_000;
const CYCLES_PER_SCANLINE: u64 = 80;
const SCANLINES_PER_FRAME: u64 = 264;
const CYCLES_PER_FRAME: u64 = SCANLINES_PER_FRAME * CYCLES_PER_SCANLINE; // 21120

const SCREEN_WIDTH: u32 = 256;
const SCREEN_HEIGHT: u32 = 240;
const VBEND: u64 = 16; // First visible scanline
const VBSTART: u64 = 256; // First blanking scanline
const FIRQ_SCANLINE: u64 = 92;

// LFSR constants (MM5837 noise generator, same polynomial as POKEY)
const POLY17_SIZE: usize = (1 << 17) - 1; // 131071

/// Gridlee Arcade System (Videa, 1982)
///
/// Hardware: Motorola 6809 @ 1.25 MHz, custom raster video.
/// Video: 256x240 bitmap with 32 hardware sprites (8x16), 2048-color
/// PROM-based palette (64 banks x 32 colors, per-scanline selectable).
///
/// Memory map:
///   0x0000-0x07FF  RAM (first 128 bytes = sprite RAM)
///   0x0800-0x7FFF  Video RAM (30KB, packed 2 pixels/byte)
///   0x9000         LS259 latch (LEDs, coin counter, cocktail flip)
///   0x9200         Palette bank select (6 bits)
///   0x9380         Watchdog reset
///   0x9500-0x9501  Trackball Y/X
///   0x9502         Fire buttons
///   0x9503         Coin/Start switches
///   0x9600         DIP switches
///   0x9700         Status (VBLANK, service)
///   0x9820         Random number generator (17-bit LFSR)
///   0x9828-0x993F  Sound registers
///   0x9C00-0x9CFF  NVRAM (256 bytes)
///   0xA000-0xFFFF  Program ROM (24KB)
pub struct GridleeSystem {
    cpu: M6809,

    // Memory
    ram: [u8; 0x0800], // 0x0000-0x07FF: work RAM (first 128 bytes = sprite RAM)
    video_ram: [u8; 0x7800], // 0x0800-0x7FFF: 30KB video RAM
    program_rom: [u8; 0x6000], // 0xA000-0xFFFF: 24KB program ROM
    nvram: [u8; 256],  // 0x9C00-0x9CFF: battery-backed

    // Graphics ROMs
    gfx_rom: [u8; 0x4000], // 16KB sprite graphics

    // Palette: pre-computed from 3x2KB PROMs (2048 entries, RGB)
    palette_rgb: [(u8, u8, u8); 2048],
    palette_bank: u8, // Current bank (6 bits, 0-63)
    palette_bank_per_scanline: [u8; SCANLINES_PER_FRAME as usize], // Latched per-scanline

    // I/O
    fire_buttons: u8, // 0x9502: bit 0 = P1 fire, bit 1 = P2 fire
    coin_start: u8,   // 0x9503: bits 0-3 = coin/start, bits 4-5 = coinage DIP
    dip_switches: u8, // 0x9600
    cocktail_flip: bool,

    // Trackball state (keyboard emulation → cumulative delta)
    track_u_pressed: bool,
    track_d_pressed: bool,
    track_l_pressed: bool,
    track_r_pressed: bool,
    last_analog_input: [u8; 2],  // Last raw trackball position [Y, X]
    last_analog_output: [u8; 2], // Cumulative output [Y, X]
    trackball_pos: [u8; 2],      // Simulated raw position [Y, X]

    // Random number generator (17-bit LFSR)
    rand17: Vec<u8>, // Pre-computed LFSR table (POLY17_SIZE + 1 entries)

    // Sound
    sound_data: [u8; 24], // Sound register state
    tone_step: u64,       // Phase increment per output sample
    tone_fraction: u64,   // 24-bit phase accumulator
    tone_volume: u8,      // 8-bit volume
    audio_buffer: Vec<i16>,
    audio_phase: u64, // Bresenham phase for 1.25 MHz → 44.1 kHz

    // Interrupt state
    irq_pending: bool,
    firq_pending: bool,

    // Timing
    clock: u64,
    cpu_cycles: u64,
    watchdog_frame_count: u8,

    // Framebuffer (256 x 240 x RGB24)
    scanline_buffer: Vec<u8>,
}

impl GridleeSystem {
    pub fn new() -> Self {
        Self {
            cpu: M6809::new(),
            ram: [0; 0x0800],
            video_ram: [0; 0x7800],
            program_rom: [0; 0x6000],
            nvram: [0; 256],
            gfx_rom: [0; 0x4000],
            palette_rgb: [(0, 0, 0); 2048],
            palette_bank: 0,
            palette_bank_per_scanline: [0; SCANLINES_PER_FRAME as usize],
            fire_buttons: 0,
            coin_start: 0,
            dip_switches: 0x09, // Default: 3 lives (bits 3-2 = 01), bonus at 10000 (bits 1-0 = 01)
            cocktail_flip: false,
            track_u_pressed: false,
            track_d_pressed: false,
            track_l_pressed: false,
            track_r_pressed: false,
            last_analog_input: [0; 2],
            last_analog_output: [0; 2],
            trackball_pos: [0; 2],
            rand17: Vec::new(),
            sound_data: [0; 24],
            tone_step: 0,
            tone_fraction: 0,
            tone_volume: 0,
            audio_buffer: Vec::with_capacity(1024),
            audio_phase: 0,
            irq_pending: false,
            firq_pending: false,
            clock: 0,
            cpu_cycles: 0,
            watchdog_frame_count: 0,
            scanline_buffer: vec![0u8; SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize * 3],
        }
    }

    /// Current scanline (0-263).
    fn current_scanline(&self) -> u64 {
        (self.clock % CYCLES_PER_FRAME) / CYCLES_PER_SCANLINE
    }

    pub fn tick(&mut self) {
        let frame_cycle = self.clock % CYCLES_PER_FRAME;

        // Trackball movement simulation: increment raw position while keys held.
        // Rate of ~1000 cycles matches Missile Command's trackball feel.
        if self.clock.is_multiple_of(1000) {
            if self.track_u_pressed {
                self.trackball_pos[0] = self.trackball_pos[0].wrapping_sub(1);
            }
            if self.track_d_pressed {
                self.trackball_pos[0] = self.trackball_pos[0].wrapping_add(1);
            }
            // X axis is reversed per MAME PORT_REVERSE
            if self.track_l_pressed {
                self.trackball_pos[1] = self.trackball_pos[1].wrapping_add(1);
            }
            if self.track_r_pressed {
                self.trackball_pos[1] = self.trackball_pos[1].wrapping_sub(1);
            }
        }

        // Per-scanline processing at scanline boundaries
        if frame_cycle.is_multiple_of(CYCLES_PER_SCANLINE) {
            let scanline = frame_cycle / CYCLES_PER_SCANLINE;

            // Latch palette bank for this scanline
            self.palette_bank_per_scanline[scanline as usize] = self.palette_bank;

            // Render visible scanlines (VBEND..VBSTART = 16..255)
            if (VBEND..VBSTART).contains(&scanline) {
                self.render_scanline(scanline as usize);
            }

            // IRQ: every 64 scanlines (0, 64, 128, 192), cleared at next scanline
            if scanline.is_multiple_of(64) && scanline < 256 {
                self.irq_pending = true;
            }

            // FIRQ: at scanline 92, cleared at next scanline
            if scanline == FIRQ_SCANLINE {
                self.firq_pending = true;
            }
        }

        // Clear IRQ/FIRQ at HBLANK (end of scanline = next scanline boundary - 1)
        // In practice: clear one cycle before the next scanline boundary.
        let cycle_in_scanline = frame_cycle % CYCLES_PER_SCANLINE;
        if cycle_in_scanline == CYCLES_PER_SCANLINE - 1 {
            self.irq_pending = false;
            self.firq_pending = false;
        }

        // Sound: Bresenham downsampling from 1.25 MHz → 44.1 kHz.
        // Tone phase accumulator advances once per output sample (matching MAME).
        self.audio_phase += 44100;
        if self.audio_phase >= CPU_CLOCK_HZ {
            self.audio_phase -= CPU_CLOCK_HZ;
            let sample = if self.tone_volume > 0 && self.tone_step > 0 {
                self.tone_fraction = self.tone_fraction.wrapping_add(self.tone_step);
                if self.tone_fraction & 0x0800000 != 0 {
                    self.tone_volume as i16 * 128
                } else {
                    0
                }
            } else {
                0
            };
            self.audio_buffer.push(sample);
        }

        // CPU execution
        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }
        self.cpu_cycles += 1;

        self.clock += 1;
    }

    /// Read trackball axis (0=Y, 1=X). Implements the MAME analog_port_r logic:
    /// compute signed delta from last read, filter tiny deltas, accumulate magnitude.
    fn read_trackball(&mut self, axis: usize) -> u8 {
        let newval = self.trackball_pos[axis];
        let mut delta = newval as i16 - self.last_analog_input[axis] as i16;

        // Handle wraparound
        if delta >= 0x80 {
            delta -= 0x100;
        }
        if delta <= -0x80 {
            delta += 0x100;
        }

        // Ignore deltas of -1, 0, or +1 (noise filter)
        if (-1..=1).contains(&delta) {
            return self.last_analog_output[axis];
        }
        self.last_analog_input[axis] = newval;

        let sign: u8 = if delta < 0 { 0x10 } else { 0x00 };
        let magnitude = delta.unsigned_abs() as u8;

        self.last_analog_output[axis] = self.last_analog_output[axis].wrapping_add(magnitude);

        (self.last_analog_output[axis] & 0x0F) | sign
    }

    /// Read the LFSR-based random number generator, keyed to CPU cycle count.
    fn read_rng(&self) -> u8 {
        if self.rand17.is_empty() {
            return 0;
        }
        // CPU at 1.25 MHz, noise source at 100 kHz → multiply by 12.5
        // 12.5 = 8 + 4 + 0.5
        let cc = self.cpu_cycles;
        let index = ((cc << 3).wrapping_add(cc << 2).wrapping_add(cc >> 1)) as usize;
        self.rand17[index & POLY17_SIZE]
    }

    /// Write to LS259 latch. Address bits 6-4 select the output bit; data bit 0 is the value.
    fn write_latch(&mut self, addr: u16, data: u8) {
        let bit = (addr >> 4) & 0x07;
        if bit == 7 {
            self.cocktail_flip = data & 1 != 0;
        }
        // Q0-Q2: LEDs/coin counter (cosmetic), Q6: unknown — ignored
    }

    /// Write to sound registers (offset from 0x9828).
    fn write_sound(&mut self, offset: u16, data: u8) {
        let off = offset as usize;
        if off < self.sound_data.len() {
            self.sound_data[off] = data;
        }

        // Tone frequency: offset 0x10 (address 0x9838)
        // step = freq_to_step * (data * 5), where freq_to_step = (1 << 24) / sample_rate
        // We compute step relative to CPU clock since we accumulate per tick.
        if off == 0x10 {
            if data > 0 {
                // freq_to_step = (1 << 24) / 44100 ≈ 380.468
                // But we accumulate per CPU tick, not per output sample.
                // Step per output sample: freq_to_step * data * 5
                // We need step per CPU tick: that / (CPU_CLOCK / SAMPLE_RATE)
                // Simpler: just use the same formula as MAME (accumulate per output sample)
                // and run the accumulator at output rate in tick().
                let freq_to_step = (1u64 << 24) / 44100;
                self.tone_step = freq_to_step * data as u64 * 5;
            } else {
                self.tone_step = 0;
            }
        }

        // Tone volume: offset 0x11 (address 0x9839)
        if off == 0x11 {
            self.tone_volume = data;
        }
    }

    /// Render one visible scanline into the framebuffer.
    fn render_scanline(&mut self, scanline: usize) {
        let screen_y = scanline - VBEND as usize;
        if screen_y >= SCREEN_HEIGHT as usize {
            return;
        }

        let palette_bank = self.palette_bank_per_scanline[scanline];
        let row_offset = screen_y * SCREEN_WIDTH as usize * 3;

        // Background: read VRAM row. Each byte = 2 pixels (upper nibble = left).
        // VRAM address for this scanline: (scanline - VBEND) * 128
        let vram_y = screen_y;
        let vram_row_start = vram_y * 128;

        for x_pair in 0..128 {
            let vram_idx = vram_row_start + x_pair;
            let vram_byte = if vram_idx < self.video_ram.len() {
                self.video_ram[vram_idx]
            } else {
                0
            };
            let left_idx = (vram_byte >> 4) & 0x0F;
            let right_idx = vram_byte & 0x0F;

            // Background uses palette indices 16-31
            let left_color = self.resolve_color(palette_bank, left_idx + 16);
            let right_color = self.resolve_color(palette_bank, right_idx + 16);

            let px = x_pair * 2;
            let off_l = row_offset + px * 3;
            if off_l + 2 < self.scanline_buffer.len() {
                self.scanline_buffer[off_l] = left_color.0;
                self.scanline_buffer[off_l + 1] = left_color.1;
                self.scanline_buffer[off_l + 2] = left_color.2;
            }

            let off_r = row_offset + (px + 1) * 3;
            if off_r + 2 < self.scanline_buffer.len() {
                self.scanline_buffer[off_r] = right_color.0;
                self.scanline_buffer[off_r + 1] = right_color.1;
                self.scanline_buffer[off_r + 2] = right_color.2;
            }
        }

        // Sprites: 32 sprites from RAM at 0x0000 (4 bytes each).
        // Format: [image_num, unused, y_pos, x_pos]
        // Each sprite is 8 wide x 16 tall, 64 bytes in GFX ROM.
        // Y positions wrap at 256 (matching MAME's `(ypos + 1) & 255`).
        for i in 0..32 {
            let base = i * 4;
            let image_num = self.ram[base] as usize;
            // Start Y = sprite_ram[2] + 17 + VBEND, wrapped to 8 bits
            let sprite_y_start = (self.ram[base + 2] as u16 + 17 + VBEND as u16) as u8;
            let sprite_x = self.ram[base + 3] as usize;

            // Which row of the sprite falls on this scanline? (wrapping subtraction)
            let row_in_sprite = (scanline as u8).wrapping_sub(sprite_y_start);
            if row_in_sprite >= 16 {
                continue;
            }

            // 4 bytes per row in GFX ROM, 64 bytes per image
            let gfx_offset = image_num * 64 + row_in_sprite as usize * 4;

            for x_byte in 0..4 {
                let gfx_idx = gfx_offset + x_byte;
                if gfx_idx >= self.gfx_rom.len() {
                    continue;
                }
                let gfx_byte = self.gfx_rom[gfx_idx];
                let left_idx = (gfx_byte >> 4) & 0x0F;
                let right_idx = gfx_byte & 0x0F;

                // Sprites use palette indices 0-15; index 0 = transparent
                for (dx, idx) in [(0usize, left_idx), (1, right_idx)] {
                    if idx == 0 {
                        continue;
                    }
                    let px = sprite_x + x_byte * 2 + dx;
                    if px >= SCREEN_WIDTH as usize {
                        continue;
                    }
                    let color = self.resolve_color(palette_bank, idx);
                    let off = row_offset + px * 3;
                    if off + 2 < self.scanline_buffer.len() {
                        self.scanline_buffer[off] = color.0;
                        self.scanline_buffer[off + 1] = color.1;
                        self.scanline_buffer[off + 2] = color.2;
                    }
                }
            }
        }
    }

    /// Look up an RGB color from the pre-computed palette.
    fn resolve_color(&self, palette_bank: u8, color_index: u8) -> (u8, u8, u8) {
        let addr = ((palette_bank as usize & 0x3F) << 5) | (color_index as usize & 0x1F);
        self.palette_rgb[addr]
    }

    /// Build the 2048-entry RGB palette from color PROMs.
    fn build_palette(&mut self, prom_data: &[u8]) {
        for i in 0..2048 {
            let r4 = prom_data[i] & 0x0F;
            let g4 = prom_data[0x0800 + i] & 0x0F;
            let b4 = prom_data[0x1000 + i] & 0x0F;
            // Expand 4-bit to 8-bit: 0x0→0x00, 0xF→0xFF
            self.palette_rgb[i] = (r4 * 17, g4 * 17, b4 * 17);
        }
    }

    /// Initialize the 17-bit LFSR polynomial table (MM5837 noise generator).
    fn init_lfsr(&mut self) {
        let mut rand17 = vec![0u8; POLY17_SIZE + 1];
        let mut x: u32 = 0;

        for entry in rand17.iter_mut().take(POLY17_SIZE) {
            // Store random byte (bits 3-10 of state)
            *entry = (x >> 3) as u8;
            // Advance polynomial: x = ((x << 7) + (x >> 10) + 0x18000) & POLY17_SIZE
            x = ((x << 7).wrapping_add(x >> 10).wrapping_add(0x18000)) & POLY17_SIZE as u32;
        }

        self.rand17 = rand17;
    }

    pub fn load_rom_set(&mut self, rom_set: &RomSet) -> Result<(), RomLoadError> {
        let program_data = GRIDLEE_PROGRAM_ROM.load(rom_set)?;
        self.program_rom.copy_from_slice(&program_data);

        let gfx_data = GRIDLEE_GFX_ROM.load(rom_set)?;
        self.gfx_rom.copy_from_slice(&gfx_data);

        let prom_data = GRIDLEE_COLOR_PROMS.load(rom_set)?;
        self.build_palette(&prom_data);

        self.init_lfsr();

        Ok(())
    }

    pub fn get_cpu_state(&self) -> M6809State {
        self.cpu.snapshot()
    }
}

impl Default for GridleeSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bus implementation
// ---------------------------------------------------------------------------

impl Bus for GridleeSystem {
    type Address = u16;
    type Data = u8;

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        match addr {
            // RAM: sprite RAM (0x0000-0x007F) + work RAM (0x0080-0x07FF)
            0x0000..=0x07FF => self.ram[addr as usize],

            // Video RAM (packed 2 pixels/byte)
            0x0800..=0x7FFF => self.video_ram[(addr - 0x0800) as usize],

            // Trackball Y
            0x9500 => self.read_trackball(0),

            // Trackball X
            0x9501 => self.read_trackball(1),

            // Fire buttons: bit 0 = P1, bit 1 = P2
            0x9502 => self.fire_buttons,

            // Coin/Start: bits 0-3 = switches, bits 4-5 = coinage DIP
            0x9503 => self.coin_start,

            // DIP switches
            0x9600 => self.dip_switches,

            // Status: bit 7 = VBLANK, bits 6-5 = service (normally high)
            0x9700 => {
                let scanline = self.current_scanline();
                let vblank = if !(VBEND..VBSTART).contains(&scanline) {
                    0x80
                } else {
                    0x00
                };
                vblank | 0x60 // Service switches not pressed (bits 6,5 high)
            }

            // Random number generator
            0x9820 => self.read_rng(),

            // NVRAM
            0x9C00..=0x9CFF => self.nvram[(addr - 0x9C00) as usize],

            // Program ROM
            0xA000..=0xFFFF => self.program_rom[(addr - 0xA000) as usize],

            _ => 0xFF,
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        match addr {
            // RAM
            0x0000..=0x07FF => self.ram[addr as usize] = data,

            // Video RAM
            0x0800..=0x7FFF => self.video_ram[(addr - 0x0800) as usize] = data,

            // LS259 latch: address bits 6-4 select output bit
            0x9000..=0x907F => self.write_latch(addr, data),

            // Palette bank select
            0x9200 => self.palette_bank = data & 0x3F,

            // Watchdog reset
            0x9380 => self.watchdog_frame_count = 0,

            // Sound registers (base 0x9828)
            0x9828..=0x993F => self.write_sound(addr - 0x9828, data),

            // NVRAM
            0x9C00..=0x9CFF => self.nvram[(addr - 0x9C00) as usize] = data,

            // ROM: writes ignored
            0xA000..=0xFFFF => {}

            _ => {}
        }
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            irq: self.irq_pending,
            firq: self.firq_pending,
            irq_vector: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Machine trait
// ---------------------------------------------------------------------------

impl Machine for GridleeSystem {
    fn display_size(&self) -> (u32, u32) {
        (SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }

        // Watchdog: reset if not serviced within 8 frames
        self.watchdog_frame_count += 1;
        if self.watchdog_frame_count >= 8 {
            self.reset();
        }
    }

    fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        let n = buffer.len().min(self.audio_buffer.len());
        buffer[..n].copy_from_slice(&self.audio_buffer[..n]);
        self.audio_buffer.drain(..n);
        n
    }

    fn audio_sample_rate(&self) -> u32 {
        44100
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.scanline_buffer);
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            INPUT_TRACK_U => self.track_u_pressed = pressed,
            INPUT_TRACK_D => self.track_d_pressed = pressed,
            INPUT_TRACK_L => self.track_l_pressed = pressed,
            INPUT_TRACK_R => self.track_r_pressed = pressed,
            INPUT_P1_FIRE => {
                if pressed {
                    self.fire_buttons |= 0x01;
                } else {
                    self.fire_buttons &= !0x01;
                }
            }
            INPUT_COIN => {
                if pressed {
                    self.coin_start |= 0x01;
                } else {
                    self.coin_start &= !0x01;
                }
            }
            INPUT_START1 => {
                if pressed {
                    self.coin_start |= 0x04;
                } else {
                    self.coin_start &= !0x04;
                }
            }
            INPUT_START2 => {
                if pressed {
                    self.coin_start |= 0x08;
                } else {
                    self.coin_start &= !0x08;
                }
            }
            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        GRIDLEE_INPUT_MAP
    }

    fn reset(&mut self) {
        self.irq_pending = false;
        self.firq_pending = false;
        self.watchdog_frame_count = 0;
        self.clock = 0;
        self.cpu_cycles = 0;
        self.tone_step = 0;
        self.tone_fraction = 0;
        self.tone_volume = 0;
        self.audio_buffer.clear();
        self.audio_phase = 0;
        self.scanline_buffer.fill(0);

        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.reset(bus, BusMaster::Cpu(0));
        }
    }

    fn save_nvram(&self) -> Option<&[u8]> {
        Some(&self.nvram)
    }

    fn load_nvram(&mut self, data: &[u8]) {
        let len = data.len().min(256);
        self.nvram[..len].copy_from_slice(&data[..len]);
    }

    fn frame_rate_hz(&self) -> f64 {
        CPU_CLOCK_HZ as f64 / CYCLES_PER_FRAME as f64
    }
}

// ---------------------------------------------------------------------------
// Machine registry
// ---------------------------------------------------------------------------

fn create_machine(
    rom_set: &RomSet,
) -> Result<Box<dyn phosphor_core::core::machine::Machine>, RomLoadError> {
    let mut sys = GridleeSystem::new();
    sys.load_rom_set(rom_set)?;
    Ok(Box::new(sys))
}

inventory::submit! {
    MachineEntry::new("gridlee", "gridlee", create_machine)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> GridleeSystem {
        let mut sys = GridleeSystem::new();
        sys.init_lfsr();
        sys
    }

    // -----------------------------------------------------------------------
    // Palette
    // -----------------------------------------------------------------------

    #[test]
    fn palette_4bit_to_8bit_expansion() {
        let mut sys = GridleeSystem::new();
        // Build a minimal PROM: R=0xF, G=0x0, B=0x8 at entry 0
        let mut prom = vec![0u8; 0x1800];
        prom[0] = 0x0F; // R = 15
        prom[0x0800] = 0x00; // G = 0
        prom[0x1000] = 0x08; // B = 8
        sys.build_palette(&prom);
        // 0xF * 17 = 255, 0x0 * 17 = 0, 0x8 * 17 = 136
        assert_eq!(sys.palette_rgb[0], (255, 0, 136));
    }

    #[test]
    fn palette_bank_addressing() {
        let mut sys = GridleeSystem::new();
        let mut prom = vec![0u8; 0x1800];
        // Set entry at bank 2, index 5 → address (2 << 5) | 5 = 69
        prom[69] = 0x0A; // R
        prom[0x0800 + 69] = 0x05; // G
        prom[0x1000 + 69] = 0x03; // B
        sys.build_palette(&prom);
        let color = sys.resolve_color(2, 5);
        assert_eq!(color, (0x0A * 17, 0x05 * 17, 0x03 * 17));
    }

    // -----------------------------------------------------------------------
    // LFSR random number generator
    // -----------------------------------------------------------------------

    #[test]
    fn lfsr_table_non_zero() {
        let sys = make_system();
        assert_eq!(sys.rand17.len(), POLY17_SIZE + 1);
        // Table should have non-zero entries (not all zeros)
        let nonzero_count = sys.rand17.iter().filter(|&&b| b != 0).count();
        assert!(nonzero_count > POLY17_SIZE / 2, "LFSR table mostly zero");
    }

    #[test]
    fn rng_returns_different_values_at_different_cycles() {
        let mut sys = make_system();
        sys.cpu_cycles = 100;
        let v1 = sys.read_rng();
        sys.cpu_cycles = 200;
        let v2 = sys.read_rng();
        sys.cpu_cycles = 300;
        let v3 = sys.read_rng();
        // At least two of three should differ
        assert!(
            v1 != v2 || v2 != v3,
            "RNG returned same value at cycles 100, 200, 300"
        );
    }

    // -----------------------------------------------------------------------
    // Memory map
    // -----------------------------------------------------------------------

    #[test]
    fn ram_read_write_roundtrip() {
        let mut sys = make_system();
        sys.write(BusMaster::Cpu(0), 0x0042, 0xAB);
        assert_eq!(sys.read(BusMaster::Cpu(0), 0x0042), 0xAB);
    }

    #[test]
    fn vram_read_write_roundtrip() {
        let mut sys = make_system();
        sys.write(BusMaster::Cpu(0), 0x0800, 0xCD);
        assert_eq!(sys.read(BusMaster::Cpu(0), 0x0800), 0xCD);
    }

    #[test]
    fn nvram_read_write_roundtrip() {
        let mut sys = make_system();
        sys.write(BusMaster::Cpu(0), 0x9C42, 0x77);
        assert_eq!(sys.read(BusMaster::Cpu(0), 0x9C42), 0x77);
    }

    #[test]
    fn rom_write_ignored() {
        let mut sys = make_system();
        sys.program_rom[0] = 0xAA;
        sys.write(BusMaster::Cpu(0), 0xA000, 0x55);
        assert_eq!(sys.read(BusMaster::Cpu(0), 0xA000), 0xAA);
    }

    #[test]
    fn unmapped_reads_return_ff() {
        let mut sys = make_system();
        assert_eq!(sys.read(BusMaster::Cpu(0), 0x8000), 0xFF);
    }

    // -----------------------------------------------------------------------
    // VBLANK
    // -----------------------------------------------------------------------

    #[test]
    fn vblank_active_during_blanking() {
        let mut sys = make_system();
        // Scanline 0 (< VBEND=16): in VBLANK
        sys.clock = 0;
        let status = sys.read(BusMaster::Cpu(0), 0x9700);
        assert_ne!(status & 0x80, 0, "VBLANK should be active at scanline 0");
    }

    #[test]
    fn vblank_inactive_during_active_display() {
        let mut sys = make_system();
        // Scanline 128 (within VBEND..VBSTART): active display
        sys.clock = 128 * CYCLES_PER_SCANLINE;
        let status = sys.read(BusMaster::Cpu(0), 0x9700);
        assert_eq!(
            status & 0x80,
            0,
            "VBLANK should be inactive at scanline 128"
        );
    }

    #[test]
    fn vblank_active_after_vbstart() {
        let mut sys = make_system();
        // Scanline 256 (>= VBSTART): in VBLANK
        sys.clock = 256 * CYCLES_PER_SCANLINE;
        let status = sys.read(BusMaster::Cpu(0), 0x9700);
        assert_ne!(status & 0x80, 0, "VBLANK should be active at scanline 256");
    }

    // -----------------------------------------------------------------------
    // Interrupts
    // -----------------------------------------------------------------------

    #[test]
    fn irq_asserted_at_scanline_0() {
        let mut sys = make_system();
        // Advance to start of scanline 0 (frame_cycle = 0)
        sys.clock = CYCLES_PER_FRAME; // Start of next frame = scanline 0
        sys.tick();
        assert!(sys.irq_pending, "IRQ should be pending at scanline 0");
    }

    #[test]
    fn irq_asserted_at_scanline_64() {
        let mut sys = make_system();
        // Set clock so tick() processes scanline 64
        sys.clock = 64 * CYCLES_PER_SCANLINE;
        sys.tick();
        assert!(sys.irq_pending, "IRQ should be pending at scanline 64");
    }

    #[test]
    fn firq_asserted_at_scanline_92() {
        let mut sys = make_system();
        sys.clock = 92 * CYCLES_PER_SCANLINE;
        sys.tick();
        assert!(sys.firq_pending, "FIRQ should be pending at scanline 92");
    }

    #[test]
    fn irq_cleared_at_hblank() {
        let mut sys = make_system();
        // Assert IRQ at scanline 0
        sys.clock = 0;
        sys.tick();
        assert!(sys.irq_pending);
        // Advance to HBLANK (last cycle of scanline 0)
        sys.clock = CYCLES_PER_SCANLINE - 1;
        sys.tick();
        assert!(!sys.irq_pending, "IRQ should be cleared at HBLANK");
    }

    // -----------------------------------------------------------------------
    // Watchdog
    // -----------------------------------------------------------------------

    #[test]
    fn watchdog_reset_prevents_timeout() {
        let mut sys = make_system();
        sys.watchdog_frame_count = 7;
        // Write to watchdog resets counter
        sys.write(BusMaster::Cpu(0), 0x9380, 0x00);
        assert_eq!(sys.watchdog_frame_count, 0);
    }

    // -----------------------------------------------------------------------
    // Palette bank select
    // -----------------------------------------------------------------------

    #[test]
    fn palette_bank_select_masks_to_6_bits() {
        let mut sys = make_system();
        sys.write(BusMaster::Cpu(0), 0x9200, 0xFF);
        assert_eq!(sys.palette_bank, 0x3F);
    }

    // -----------------------------------------------------------------------
    // Trackball
    // -----------------------------------------------------------------------

    #[test]
    fn trackball_filters_small_deltas() {
        let mut sys = make_system();
        // Move by 1 (should be filtered)
        sys.trackball_pos[0] = 1;
        let result = sys.read_trackball(0);
        assert_eq!(result, 0, "Delta of 1 should be filtered out");
    }

    #[test]
    fn trackball_reports_magnitude_and_sign() {
        let mut sys = make_system();
        // Move by +5
        sys.trackball_pos[0] = 5;
        let result = sys.read_trackball(0);
        // Magnitude = 5 & 0xF = 5, sign = 0 (positive)
        assert_eq!(result & 0x0F, 5);
        assert_eq!(result & 0x10, 0, "Should be positive direction");
    }

    #[test]
    fn trackball_negative_direction() {
        let mut sys = make_system();
        // Move by -5 (wraps to 251)
        sys.trackball_pos[0] = 251;
        let result = sys.read_trackball(0);
        // Magnitude = 5 & 0xF = 5, sign = 0x10 (negative)
        assert_eq!(result & 0x0F, 5);
        assert_eq!(result & 0x10, 0x10, "Should be negative direction");
    }

    // -----------------------------------------------------------------------
    // Sound
    // -----------------------------------------------------------------------

    #[test]
    fn sound_tone_step_zero_when_freq_zero() {
        let mut sys = make_system();
        sys.write_sound(0x10, 0);
        assert_eq!(sys.tone_step, 0);
    }

    #[test]
    fn sound_tone_step_nonzero_when_freq_set() {
        let mut sys = make_system();
        sys.write_sound(0x10, 0x40);
        assert!(
            sys.tone_step > 0,
            "tone_step should be non-zero for freq=0x40"
        );
    }

    #[test]
    fn sound_volume_register() {
        let mut sys = make_system();
        sys.write_sound(0x11, 0xAB);
        assert_eq!(sys.tone_volume, 0xAB);
    }

    // -----------------------------------------------------------------------
    // Input
    // -----------------------------------------------------------------------

    #[test]
    fn fire_button_set_and_clear() {
        let mut sys = make_system();
        sys.set_input(INPUT_P1_FIRE, true);
        assert_eq!(sys.fire_buttons & 0x01, 0x01);
        sys.set_input(INPUT_P1_FIRE, false);
        assert_eq!(sys.fire_buttons & 0x01, 0x00);
    }

    #[test]
    fn coin_and_start_buttons() {
        let mut sys = make_system();
        sys.set_input(INPUT_COIN, true);
        assert_eq!(sys.coin_start & 0x01, 0x01);
        sys.set_input(INPUT_START1, true);
        assert_eq!(sys.coin_start & 0x04, 0x04);
        sys.set_input(INPUT_START2, true);
        assert_eq!(sys.coin_start & 0x08, 0x08);
    }

    // -----------------------------------------------------------------------
    // NVRAM persistence
    // -----------------------------------------------------------------------

    #[test]
    fn nvram_save_load_roundtrip() {
        let mut sys = make_system();
        sys.nvram[0] = 0x42;
        sys.nvram[255] = 0xFF;
        let saved = sys.save_nvram().unwrap().to_vec();
        assert_eq!(saved.len(), 256);

        let mut sys2 = make_system();
        sys2.load_nvram(&saved);
        assert_eq!(sys2.nvram[0], 0x42);
        assert_eq!(sys2.nvram[255], 0xFF);
    }

    // -----------------------------------------------------------------------
    // LS259 latch
    // -----------------------------------------------------------------------

    #[test]
    fn latch_cocktail_flip() {
        let mut sys = make_system();
        // Q7 (bit 7 select) = address bits 6:4 = 0b111, so addr = 0x9070
        sys.write(BusMaster::Cpu(0), 0x9070, 0x01);
        assert!(sys.cocktail_flip);
        sys.write(BusMaster::Cpu(0), 0x9070, 0x00);
        assert!(!sys.cocktail_flip);
    }

    // -----------------------------------------------------------------------
    // Frame rate
    // -----------------------------------------------------------------------

    #[test]
    fn frame_rate_approximately_59hz() {
        let sys = make_system();
        let hz = sys.frame_rate_hz();
        assert!(
            (59.0..60.0).contains(&hz),
            "Frame rate {hz} Hz not in expected range 59-60 Hz"
        );
    }
}
