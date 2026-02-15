use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::m6502::M6502;
use phosphor_core::cpu::state::M6502State;
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::pokey::Pokey;

use crate::rom_loader::{RomEntry, RomRegion};

// ---------------------------------------------------------------------------
// Missile Command ROM definitions
// ---------------------------------------------------------------------------

/// Program ROM: 12KB at 0x5000-0x7FFF.
/// The last 2KB (0x7800-0x7FFF) is also mirrored to 0xF800-0xFFFF for vectors.
pub static MISSILE_COMMAND_ROM: RomRegion = RomRegion {
    size: 0x3000, // 12KB
    entries: &[
        RomEntry {
            name: "035820-02.h1",
            size: 0x0800,
            offset: 0x0000,
            crc32: &[0x7a62ce6a],
        },
        RomEntry {
            name: "035821-02.jk1",
            size: 0x0800,
            offset: 0x0800,
            crc32: &[0xdf3bd57f],
        },
        RomEntry {
            name: "035822-03e.kl1",
            size: 0x0800,
            offset: 0x1000,
            crc32: &[0x1a2f599a, 0xa1cd384a], // -03e (parent) and -02 (missile2)
        },
        RomEntry {
            name: "035823-02.lm1",
            size: 0x0800,
            offset: 0x1800,
            crc32: &[0x82e552bb],
        },
        RomEntry {
            name: "035824-02.np1",
            size: 0x0800,
            offset: 0x2000,
            crc32: &[0x606e42e0],
        },
        RomEntry {
            name: "035825-02.r1",
            size: 0x0800,
            offset: 0x2800,
            crc32: &[0xf752eaeb],
        },
    ],
};

// ---------------------------------------------------------------------------
// Input button IDs
// ---------------------------------------------------------------------------
pub const INPUT_COIN: u8 = 0;
pub const INPUT_START1: u8 = 1;
pub const INPUT_START2: u8 = 2;
pub const INPUT_FIRE_LEFT: u8 = 3;
pub const INPUT_FIRE_CENTER: u8 = 4;
pub const INPUT_FIRE_RIGHT: u8 = 5;
pub const INPUT_TRACK_L: u8 = 6;
pub const INPUT_TRACK_R: u8 = 7;
pub const INPUT_TRACK_U: u8 = 8;
pub const INPUT_TRACK_D: u8 = 9;

const MISSILE_INPUT_MAP: &[InputButton] = &[
    InputButton { id: INPUT_COIN, name: "Coin" },
    InputButton { id: INPUT_START1, name: "P1 Start" },
    InputButton { id: INPUT_START2, name: "P2 Start" },
    InputButton { id: INPUT_FIRE_LEFT, name: "Fire Left" },
    InputButton { id: INPUT_FIRE_CENTER, name: "Fire Center" },
    InputButton { id: INPUT_FIRE_RIGHT, name: "Fire Right" },
    InputButton { id: INPUT_TRACK_L, name: "P1 Left" },
    InputButton { id: INPUT_TRACK_R, name: "P1 Right" },
    InputButton { id: INPUT_TRACK_U, name: "P1 Up" },
    InputButton { id: INPUT_TRACK_D, name: "P1 Down" },
];

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------
// Master clock: 10 MHz XTAL
// CPU clock: 10 MHz / 8 = 1.25 MHz
// Pixel clock: 10 MHz / 2 = 5 MHz
// HTOTAL: 320 pixel clocks → 320/4 = 80 CPU cycles per scanline
// VTOTAL: 256 scanlines per frame
// Frame rate: 5 MHz / (320 * 256) ≈ 61.04 Hz
const CYCLES_PER_SCANLINE: u64 = 80;
const SCANLINES_PER_FRAME: u64 = 256;
const CYCLES_PER_FRAME: u64 = SCANLINES_PER_FRAME * CYCLES_PER_SCANLINE;

/// Missile Command Arcade System (Atari, 1980)
///
/// Hardware: MOS 6502 @ 1.25 MHz, POKEY for sound/IO.
/// Video: 256x231 bitmap, bit-planar 2bpp (8-color with 3rd bit region
/// for bottom scanlines), 8-entry programmable palette at 0x4B00.
///
/// Memory map (from MAME atari/missile.cpp):
///   0x0000-0x3FFF  Video/Work RAM (16KB)
///   0x4000-0x400F  POKEY (mirrored across 0x4000-0x47FF)
///   0x4800         Read: IN0 (switches) or trackball (CTRLD-dependent)
///                  Write: Output latch (CTRLD, LEDs, coin counters, flip)
///   0x4900         Read: IN1 (fire buttons, VBLANK, tilt, test)
///   0x4A00         Read: DIP switches (pricing options)
///   0x4B00-0x4B07  Write: Color RAM (8 entries, 1-bit RGB)
///   0x4C00         Write: Watchdog reset
///   0x4D00         Write: IRQ acknowledge
///   0x5000-0x7FFF  Program ROM (12KB)
///   0xF800-0xFFFF  ROM mirror (vectors)
pub struct MissileCommandSystem {
    cpu: M6502,
    pokey: Pokey,

    // Memory
    ram: [u8; 0x4000], // 16KB Video/Work RAM
    rom: [u8; 0x3000], // 12KB Program ROM

    // I/O registers
    // IN0 at 0x4800 (active-low switches, directly stored active-low: 1=released, 0=pressed)
    //   Bit 7: Right Coin    Bit 6: Center Coin   Bit 5: Left Coin
    //   Bit 4: 1P Start      Bit 3: 2P Start
    //   Bit 2-0: Cocktail fire buttons (active-low)
    in0: u8,
    // IN1 at 0x4900 (mixed polarity)
    //   Bit 7: VBLANK (active-high, set dynamically)
    //   Bit 6: Self-test (active-low, normally 1)
    //   Bit 5: SLAM/Tilt (active-low, normally 1)
    //   Bit 4-3: Trackball direction (set dynamically)
    //   Bit 2: Fire Left (active-low, normally 1)
    //   Bit 1: Fire Center (active-low, normally 1)
    //   Bit 0: Fire Right (active-low, normally 1)
    in1: u8,
    // DIP switches at 0x4A00 (pricing options)
    dip_switches: u8,
    // CTRLD: bit 0 of output latch (0x4800 write) — selects trackball vs switches at 0x4800 read
    ctrld: bool,
    // Color RAM: 8 palette entries at 0x4B00-0x4B07
    palette: [u8; 8],

    // Trackball counters (4-bit each, combined into one byte when CTRLD=1)
    trackball_x: u8,
    trackball_y: u8,
    trackball_l_pressed: bool,
    trackball_r_pressed: bool,
    trackball_u_pressed: bool,
    trackball_d_pressed: bool,

    // IRQ state — based on /32V signal (inverted bit 5 of V counter)
    // Asserted at scanlines where 32V=0 (scanlines 0-31, 64-95, 128-159, 192-223)
    // Cleared by writing to 0x4D00 (IRQ acknowledge)
    irq_state: bool,

    // MADSEL circuit: intercepts (zp,X) addressing mode instructions (opcodes with
    // low 5 bits == 0x01) and redirects bus access 5 cycles later to VRAM.
    // This is how the game writes pixels — without it, the screen stays blank.
    madsel_lastcycles: u64,

    // System
    clock: u64,
    watchdog_counter: u32,
}

impl MissileCommandSystem {
    pub fn new() -> Self {
        Self {
            cpu: M6502::new(),
            pokey: Pokey::with_clock(1_250_000, 44100),
            ram: [0; 0x4000],
            rom: [0; 0x3000],
            in0: 0xFF,         // All buttons released (active-low: 1 = not pressed)
            in1: 0x67,         // Fire buttons released (bits 0-2 = 1), test/tilt released (bits 5-6 = 1), VBLANK off
            dip_switches: 0x00, // Default DIP: 1 coin/1 play, English, standard options
            ctrld: false,
            palette: [0; 8],
            trackball_x: 0,
            trackball_y: 0,
            trackball_l_pressed: false,
            trackball_r_pressed: false,
            trackball_u_pressed: false,
            trackball_d_pressed: false,
            irq_state: false,
            madsel_lastcycles: 0,
            clock: 0,
            watchdog_counter: 0,
        }
    }

    /// Current scanline (V counter), 0-255.
    pub fn current_scanline(&self) -> u16 {
        let frame_cycle = self.clock % CYCLES_PER_FRAME;
        (frame_cycle / CYCLES_PER_SCANLINE) as u16
    }

    pub fn tick(&mut self) {
        // Trackball movement simulation: increment counters while direction keys are held
        if self.clock.is_multiple_of(1000) {
            if self.trackball_l_pressed { self.trackball_x = self.trackball_x.wrapping_sub(1) & 0x0F; }
            if self.trackball_r_pressed { self.trackball_x = self.trackball_x.wrapping_add(1) & 0x0F; }
            if self.trackball_u_pressed { self.trackball_y = self.trackball_y.wrapping_sub(1) & 0x0F; }
            if self.trackball_d_pressed { self.trackball_y = self.trackball_y.wrapping_add(1) & 0x0F; }
        }

        // IRQ generation based on /32V signal (from MAME: missile.cpp)
        // /IRQ is clocked by /16V transitions. When not flipped:
        //   At V=0,64,128,192 (32V=0): IRQ asserted
        //   At V=32,96,160,224 (32V=1): IRQ deasserted
        // The IRQ is latched on each SYNC (instruction fetch).
        // For simplicity, we assert IRQ at 16V boundaries based on 32V.
        let frame_cycle = self.clock % CYCLES_PER_FRAME;
        if frame_cycle.is_multiple_of(CYCLES_PER_SCANLINE) {
            let scanline = (frame_cycle / CYCLES_PER_SCANLINE) as u16;
            // Clock at 16V boundaries (every 16 scanlines)
            if scanline.is_multiple_of(16) {
                // /32V = inverted bit 5 of V counter
                let bit_32v = (scanline >> 5) & 1;
                if bit_32v == 0 {
                    self.irq_state = true;
                }
            }
        }

        // Update VBLANK bit in IN1 (bit 7, active-high)
        // VBLANK is active when V < 25 (VBEND=25 from MAME)
        let scanline = self.current_scanline();
        if scanline < 25 {
            self.in1 |= 0x80;
        } else {
            self.in1 &= !0x80;
        }

        // POKEY tick (runs at CPU clock rate = 1.25 MHz)
        self.pokey.tick();

        // CPU clock halving: at scanline 224+, CPU runs at MASTER_CLOCK/16 (0.625 MHz)
        // instead of MASTER_CLOCK/8 (1.25 MHz). We skip every other CPU cycle.
        let run_cpu = if scanline >= 224 {
            self.clock.is_multiple_of(2)
        } else {
            true
        };

        if run_cpu {
            let bus_ptr: *mut Self = self;
            unsafe {
                let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
                self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
            }
        }

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    pub fn load_rom_set(
        &mut self,
        rom_set: &crate::rom_loader::RomSet,
    ) -> Result<(), crate::rom_loader::RomLoadError> {
        let rom_data = MISSILE_COMMAND_ROM.load(rom_set)?;
        self.rom.copy_from_slice(&rom_data);
        Ok(())
    }

    pub fn get_cpu_state(&self) -> M6502State {
        self.cpu.snapshot()
    }

    pub fn read_ram(&self, addr: usize) -> u8 {
        if addr < self.ram.len() { self.ram[addr] } else { 0 }
    }

    pub fn write_ram(&mut self, addr: usize, data: u8) {
        if addr < self.ram.len() { self.ram[addr] = data; }
    }

    pub fn read_palette(&self, index: usize) -> u8 {
        if index < 8 { self.palette[index] } else { 0 }
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Check if the MADSEL signal is active. MADSEL goes high exactly 5 cycles
    /// after arming and stays high for 1 cycle. Resets after firing.
    fn get_madsel(&mut self) -> bool {
        if self.madsel_lastcycles > 0 {
            let elapsed = self.clock.wrapping_sub(self.madsel_lastcycles);
            if elapsed == 5 {
                self.madsel_lastcycles = 0;
                return true;
            }
        }
        false
    }

    /// MADSEL write: redirect bus write to VRAM using bit-planar format.
    /// Address bits select VRAM byte and pixel within it.
    /// Data bits 7:6 select the 2-bit color value.
    /// Data bit 5 provides the 3rd color bit (for bottom scanlines).
    fn vram_madsel_write(&mut self, offset: u16, data: u8) {
        const DATA_LOOKUP: [u8; 4] = [0x00, 0x0F, 0xF0, 0xFF];

        // 2-bit planar write: VRAM address = offset >> 2
        let vramaddr = (offset >> 2) as usize;
        let pixel = offset & 3;
        let vramdata = DATA_LOOKUP[(data >> 6) as usize];
        let vrammask = !(0x11u8 << pixel);

        if vramaddr < 0x4000 {
            self.ram[vramaddr] = (self.ram[vramaddr] & vrammask) | (vramdata & !vrammask);
        }

        // 3rd color bit write (MUSHROOM region): offset & 0xE000 == 0xE000
        if (offset & 0xE000) == 0xE000 {
            let bit3_addr = Self::get_bit3_addr(offset) as usize;
            let bit3_data: u8 = if data & 0x20 != 0 { 0xFF } else { 0x00 };
            let bit3_mask = !(1u8 << (offset & 7));

            if bit3_addr < 0x4000 {
                self.ram[bit3_addr] =
                    (self.ram[bit3_addr] & bit3_mask) | (bit3_data & !bit3_mask);
            }
        }
    }

    /// MADSEL read: extract pixel color from VRAM and return in bits 7:6 (and bit 5
    /// for 3rd color bit region).
    fn vram_madsel_read(&self, offset: u16) -> u8 {
        let vramaddr = (offset >> 2) as usize;
        let vrammask = 0x11u8 << (offset & 3);
        let vramdata = if vramaddr < 0x4000 {
            self.ram[vramaddr] & vrammask
        } else {
            0
        };

        let mut result = 0xFFu8;
        if (vramdata & 0xF0) == 0 {
            result &= !0x80;
        }
        if (vramdata & 0x0F) == 0 {
            result &= !0x40;
        }

        // 3rd color bit read (MUSHROOM region)
        if (offset & 0xE000) == 0xE000 {
            let bit3_addr = Self::get_bit3_addr(offset) as usize;
            let bit3_mask = 1u8 << (offset & 7);
            let bit3_data = if bit3_addr < 0x4000 {
                self.ram[bit3_addr] & bit3_mask
            } else {
                0
            };
            if bit3_data == 0 {
                result &= !0x20;
            }
        }

        result
    }

    /// Convert a 16-bit pixel address to a VRAM address for the 3rd color bit.
    /// Based on MAME's get_bit3_addr() logic from the hardware schematics.
    pub fn get_bit3_addr(pixaddr: u16) -> u16 {
        ((pixaddr & 0x0800) >> 1)
            | ((!pixaddr & 0x0800) >> 2)
            | ((pixaddr & 0x07F8) >> 2)
            | ((pixaddr & 0x1000) >> 12)
    }
}

impl Default for MissileCommandSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for MissileCommandSystem {
    type Address = u16;
    type Data = u8;

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false // No DMA hardware on Missile Command
    }

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        // MADSEL check: if active, redirect read to VRAM (bypasses normal decoding)
        if self.get_madsel() {
            return self.vram_madsel_read(addr);
        }

        // 15-bit address bus masking (global_mask 0x7FFF from MAME).
        // The 6502 vectors at 0xFFFC map through: 0xFFFC & 0x7FFF = 0x7FFC → ROM.
        let addr = addr & 0x7FFF;

        let data = match addr {
            // Video/Work RAM: 0x0000-0x3FFF
            0x0000..=0x3FFF => self.ram[addr as usize],

            // POKEY: 0x4000-0x400F (mirrored across 0x4000-0x47FF)
            0x4000..=0x47FF => self.pokey.read((addr & 0x0F) as u8),

            // IN0/Trackball: 0x4800-0x48FF
            // When CTRLD=0: read switch inputs (IN0, active-low)
            // When CTRLD=1: read trackball (low nibble = horiz, high nibble = vert)
            0x4800..=0x48FF => {
                if self.ctrld {
                    (self.trackball_y << 4) | (self.trackball_x & 0x0F)
                } else {
                    self.in0
                }
            }

            // IN1: 0x4900-0x49FF
            // Fire buttons, VBLANK, self-test, SLAM, trackball direction
            0x4900..=0x49FF => self.in1,

            // DIP switches (pricing): 0x4A00-0x4AFF
            0x4A00..=0x4AFF => self.dip_switches,

            // Program ROM: 0x5000-0x7FFF
            0x5000..=0x7FFF => self.rom[(addr - 0x5000) as usize],

            _ => 0xFF,
        };

        // MADSEL arming: during SYNC (opcode fetch), if the opcode has low 5 bits
        // == 0x01 (indirect X addressing mode) and IRQ is not asserted, arm the
        // MADSEL counter. It will fire 5 cycles later.
        if self.cpu.is_sync()
            && (data & 0x1F) == 0x01
            && !self.irq_state
            && !self.pokey.irq()
        {
            self.madsel_lastcycles = self.clock;
        }

        data
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        // MADSEL check: if active, redirect write to VRAM (bypasses normal decoding)
        if self.get_madsel() {
            self.vram_madsel_write(addr, data);
            return;
        }

        // 15-bit address bus masking
        let addr = addr & 0x7FFF;

        match addr {
            // Video/Work RAM: 0x0000-0x3FFF
            0x0000..=0x3FFF => self.ram[addr as usize] = data,

            // POKEY: 0x4000-0x400F (mirrored across 0x4000-0x47FF)
            0x4000..=0x47FF => self.pokey.write((addr & 0x0F) as u8, data),

            // Output latch: 0x4800-0x48FF
            //   Bit 0: CTRLD (0 = read switches, 1 = read trackball)
            //   Bit 1: 1P Start LED
            //   Bit 2: 2P Start LED
            //   Bit 3-5: Coin counters
            //   Bit 6: Screen flip
            0x4800..=0x48FF => {
                self.ctrld = (data & 1) != 0;
            }

            // Color RAM: 0x4B00-0x4B07 (mirrored across 0x4B00-0x4BFF)
            0x4B00..=0x4BFF => {
                self.palette[(addr & 0x07) as usize] = data;
            }

            // Watchdog reset: 0x4C00-0x4CFF
            0x4C00..=0x4CFF => {
                self.watchdog_counter = 0;
            }

            // IRQ acknowledge: 0x4D00-0x4DFF
            0x4D00..=0x4DFF => {
                self.irq_state = false;
            }

            _ => {}
        }
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: false,
            irq: self.irq_state || self.pokey.irq(),
            firq: false,
        }
    }
}

impl Machine for MissileCommandSystem {
    fn display_size(&self) -> (u32, u32) {
        (256, 231)
    }

    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        let (width, height) = self.display_size();
        let w = width as usize;
        let h = height as usize;

        // Resolve palette: each entry has 1-bit per RGB channel (inverted)
        // Bits 3/2/1 = ~R/~G/~B (from MAME palette_w)
        let mut palette_rgb = [(0u8, 0u8, 0u8); 8];
        for (i, rgb) in palette_rgb.iter_mut().enumerate() {
            let entry = self.palette[i];
            *rgb = (
                if entry & 0x08 == 0 { 255 } else { 0 }, // R = inverted bit 3
                if entry & 0x04 == 0 { 255 } else { 0 }, // G = inverted bit 2
                if entry & 0x02 == 0 { 255 } else { 0 }, // B = inverted bit 1
            );
        }

        // Video RAM layout (from MAME screen_update_missile):
        // Row-major: each row is 64 bytes, 4 pixels per byte.
        // Bit-planar format within each byte:
        //   Lower nibble (bits 0-3) = plane 0 for 4 pixels
        //   Upper nibble (bits 4-7) = plane 1 for 4 pixels
        // Pixel N (0-3) uses bit N (plane 0) and bit N+4 (plane 1).
        //
        // For scanlines >= 224, a 3rd color bit is stored in a separate VRAM
        // region, enabling 8 colors for the bottom 32 scanlines (score area).
        //
        // Visible area: scanlines 25-255 (VBEND=25), 256 pixels wide.

        let ram = &self.ram;

        for screen_y in 0..h {
            let effy = screen_y + 25; // visible starts at V=25 (VBEND)
            let src_base = effy * 64;

            // Compute 3rd color bit base address for bottom scanlines
            let bit3_base = if effy >= 224 {
                Some(Self::get_bit3_addr((effy as u16) << 8) as usize)
            } else {
                None
            };

            for screen_x in 0..w {
                let byte_offset = src_base + screen_x / 4;
                let pixel_in_byte = screen_x & 3;

                let byte = if byte_offset < 0x4000 {
                    ram[byte_offset]
                } else {
                    0
                };

                // Extract 2-bit color from bit-planar format (matches MAME exactly)
                let pix = byte >> pixel_in_byte;
                let mut color_idx = ((pix >> 2) & 4) | ((pix << 1) & 2);

                // Add 3rd color bit for bottom scanlines (effy >= 224)
                if let Some(base) = bit3_base {
                    let bit3_offset = base + (screen_x / 8) * 2;
                    if bit3_offset < 0x4000 {
                        color_idx |= (ram[bit3_offset] >> (screen_x & 7)) & 1;
                    }
                }

                let (r, g, b) = palette_rgb[color_idx as usize];

                let pixel_offset = (screen_y * w + screen_x) * 3;
                buffer[pixel_offset] = r;
                buffer[pixel_offset + 1] = g;
                buffer[pixel_offset + 2] = b;
            }
        }
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // IN0 switches (active-low: clear bit when pressed, set when released)
            INPUT_COIN => set_bit_active_low(&mut self.in0, 5, pressed),    // Left Coin
            INPUT_START1 => set_bit_active_low(&mut self.in0, 4, pressed),  // 1P Start
            INPUT_START2 => set_bit_active_low(&mut self.in0, 3, pressed),  // 2P Start

            // IN1 fire buttons (active-low: clear bit when pressed, set when released)
            INPUT_FIRE_LEFT => set_bit_active_low(&mut self.in1, 2, pressed),   // Left fire
            INPUT_FIRE_CENTER => set_bit_active_low(&mut self.in1, 1, pressed), // Center fire
            INPUT_FIRE_RIGHT => set_bit_active_low(&mut self.in1, 0, pressed),  // Right fire

            // Trackball directions
            INPUT_TRACK_L => self.trackball_l_pressed = pressed,
            INPUT_TRACK_R => self.trackball_r_pressed = pressed,
            INPUT_TRACK_U => self.trackball_u_pressed = pressed,
            INPUT_TRACK_D => self.trackball_d_pressed = pressed,
            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        MISSILE_INPUT_MAP
    }

    fn reset(&mut self) {
        self.cpu.reset();
        // Load reset vector (6502 little-endian at 0xFFFC-0xFFFD)
        // 0xFFFC maps to ROM offset 0x2FFC (0xFFFC - 0x5000 = 0xAFFC, but via mirror:
        // 0xFFFC - 0xF800 + 0x2800 = 0x2FFC)
        let vec_lo = self.rom[0x2FFC];
        let vec_hi = self.rom[0x2FFD];
        self.cpu.pc = u16::from_le_bytes([vec_lo, vec_hi]);
    }

    fn save_nvram(&self) -> Option<&[u8]> { None }
    fn load_nvram(&mut self, _data: &[u8]) {}
}

/// Active-low bit manipulation: clear bit on press, set bit on release.
fn set_bit_active_low(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg &= !(1 << bit);
    } else {
        *reg |= 1 << bit;
    }
}
