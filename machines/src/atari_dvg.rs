use phosphor_core::core::machine::Renderable;
use phosphor_core::core::memory_map::MemoryMap;
use phosphor_core::core::save_state::{SaveError, Saveable, StateReader, StateWriter};
use phosphor_core::core::{Bus, BusMaster, TimingConfig};
use phosphor_core::cpu::m6502::M6502;
use phosphor_core::device::dvg::{Dvg, VectorLine};
use phosphor_macros::{BusDebug, MemoryRegion};

// ---------------------------------------------------------------------------
// Memory regions (shared by Asteroids, Asteroids Deluxe, Lunar Lander)
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, MemoryRegion)]
pub(crate) enum Region {
    Ram = 1,
    Io = 2,
    VectorRam = 3,
    VectorRom = 4,
    ProgramRom = 5,
}

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------

// Master clock: 12.096 MHz
// CPU clock: 12.096 / 8 = 1.512 MHz
// NMI: 3 KHz / 12 ≈ 250 Hz → every ~6048 CPU cycles
// Frame: ~60 Hz → ~25200 CPU cycles
pub const TIMING: TimingConfig = TimingConfig {
    cpu_clock_hz: 1_512_000,     // 12.096 MHz / 8
    cycles_per_scanline: 25_200, // no scanline hardware; whole frame
    total_scanlines: 1,
    display_width: 1024, // vector display
    display_height: 1024,
};

pub const NMI_PERIOD_CYCLES: u64 = TIMING.cpu_clock_hz / 250;

// ---------------------------------------------------------------------------
// Atari DVG board
// ---------------------------------------------------------------------------

/// Shared hardware for Atari DVG-based arcade games (1979–1980).
///
/// Hardware: MOS 6502 @ 1.512 MHz, Atari DVG vector display.
/// Video: 1024×1024 vector display via Digital Vector Generator.
/// Used by: Asteroids, Asteroids Deluxe, Lunar Lander.
///
/// Each game provides its own memory map, I/O decode, and ROM definitions
/// via a thin wrapper struct that owns this board and implements `Bus`.
#[derive(BusDebug)]
pub struct AtariDvgBoard {
    #[debug_cpu("M6502")]
    pub(crate) cpu: M6502,
    #[debug_device("DVG")]
    pub(crate) dvg: Dvg,

    #[debug_map(cpu = 0)]
    pub(crate) map: MemoryMap,

    // NMI timing
    pub(crate) clock: u64,
    pub(crate) nmi_counter: u64,
    pub(crate) nmi_pending: bool,

    // Watchdog (resets if not written within 8 frames)
    pub(crate) watchdog_frame_count: u8,

    // Vector display
    pub(crate) display_list: Vec<VectorLine>,

    // DVG vector ROM placement in the 8 KB DVG address space.
    // Vector RAM always occupies DVG 0x0000–0x07FF.
    // Vector ROM offset and size vary per game:
    //   Asteroids:        offset 0x1000, size 0x0800 (2 KB)
    //   Asteroids Deluxe: offset 0x0800, size 0x1000 (4 KB)
    //   Lunar Lander:     offset 0x0800, size 0x1800 (6 KB)
    vrom_dvg_offset: usize,
    vrom_size: usize,
}

impl AtariDvgBoard {
    /// Create a new board with a pre-configured memory map and DVG ROM placement.
    pub fn new(map: MemoryMap, vrom_dvg_offset: usize, vrom_size: usize) -> Self {
        Self {
            cpu: M6502::new(),
            dvg: Dvg::new(),
            map,
            clock: 0,
            nmi_counter: 0,
            nmi_pending: false,
            watchdog_frame_count: 0,
            display_list: Vec::with_capacity(512),
            vrom_dvg_offset,
            vrom_size,
        }
    }

    /// Tick one cycle: NMI timing + CPU execution.
    ///
    /// The caller provides a `Bus` reference (created via `bus_split!` on the
    /// game wrapper) so the CPU's memory accesses route through game-specific
    /// I/O decode logic.
    pub fn tick(&mut self, bus: &mut dyn Bus<Address = u16, Data = u8>) {
        // NMI generation: 3 KHz / 12 ≈ 250 Hz
        self.nmi_counter += 1;
        if self.nmi_counter >= NMI_PERIOD_CYCLES {
            self.nmi_counter = 0;
            self.nmi_pending = true;
        }
        // Clear NMI pulse after 16 cycles (long enough for CPU to detect the edge).
        if self.nmi_pending && self.nmi_counter == 16 {
            self.nmi_pending = false;
        }

        // CPU tick
        self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        self.clock += 1;
    }

    /// Trigger the DVG: assemble vector memory and run to completion.
    ///
    /// The DVG has a 13-bit (8 KB) byte address space:
    ///   0x0000–0x07FF  Vector RAM (always)
    ///   0x0800–0x1FFF  Vector ROM (game-specific offset and size)
    pub fn trigger_dvg(&mut self) {
        let mut vmem = vec![0u8; 0x2000]; // 8 KB DVG address space
        vmem[0x0000..0x0800].copy_from_slice(self.map.region_data(Region::VectorRam));
        let vrom = self.map.region_data(Region::VectorRom);
        let end = self.vrom_dvg_offset + self.vrom_size;
        vmem[self.vrom_dvg_offset..end].copy_from_slice(&vrom[..self.vrom_size]);
        self.dvg.go();
        self.dvg.execute(&vmem);
        self.display_list = self.dvg.take_display_list();
    }

    /// Reset board state. CPU reset must be done separately by the wrapper
    /// via `bus_split!` (since the CPU needs a Bus reference).
    pub fn reset(&mut self) {
        self.dvg.reset();
        self.nmi_pending = false;
        self.nmi_counter = 0;
        self.watchdog_frame_count = 0;
        self.display_list.clear();
    }

    /// Render the vector display list into an RGB24 framebuffer.
    pub fn render_frame(&self, buffer: &mut [u8]) {
        rasterize_vectors(&self.display_list, buffer);
    }

    /// Check if the CPU is at an instruction boundary (for debug stepping).
    pub fn debug_tick_boundaries(&self) -> u32 {
        if self.cpu.at_instruction_boundary() {
            1
        } else {
            0
        }
    }

    // --- Save/Load state helpers ---

    pub(crate) fn save_board_state(&self, w: &mut StateWriter) {
        self.cpu.save_state(w);
        self.dvg.save_state(w);
        w.write_bytes(self.map.region_data(Region::Ram));
        w.write_bytes(self.map.region_data(Region::VectorRam));
        w.write_u64_le(self.clock);
        w.write_u64_le(self.nmi_counter);
        w.write_bool(self.nmi_pending);
        w.write_u8(self.watchdog_frame_count);
    }

    pub(crate) fn load_board_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        self.cpu.load_state(r)?;
        self.dvg.load_state(r)?;
        r.read_bytes_into(self.map.region_data_mut(Region::Ram))?;
        r.read_bytes_into(self.map.region_data_mut(Region::VectorRam))?;
        self.clock = r.read_u64_le()?;
        self.nmi_counter = r.read_u64_le()?;
        self.nmi_pending = r.read_bool()?;
        self.watchdog_frame_count = r.read_u8()?;
        self.display_list.clear();
        Ok(())
    }
}

impl Renderable for AtariDvgBoard {
    fn display_size(&self) -> (u32, u32) {
        TIMING.display_size()
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        rasterize_vectors(&self.display_list, buffer);
    }

    fn vector_display_list(&self) -> Option<&[VectorLine]> {
        Some(&self.display_list)
    }
}

// ---------------------------------------------------------------------------
// Vector rasterizer
// ---------------------------------------------------------------------------

/// Intensity-to-brightness lookup table (4-bit, 0 = invisible).
const INTENSITY_LUT: [u8; 16] = [
    0, 20, 40, 60, 80, 100, 120, 140, 160, 175, 190, 205, 220, 232, 244, 255,
];

/// Rasterize a display list of vector line segments into an RGB24 framebuffer.
///
/// Uses Bresenham line drawing with additive blending (saturating add) so
/// crossing lines appear brighter. Coordinates are in DVG space (0–1023),
/// with Y=0 at bottom; the framebuffer uses Y=0 at top.
fn rasterize_vectors(display_list: &[VectorLine], buffer: &mut [u8]) {
    buffer.fill(0);

    for line in display_list {
        if line.intensity == 0 {
            continue;
        }
        let brightness = INTENSITY_LUT[(line.intensity & 0xF) as usize];

        let x0 = line.x0.clamp(0, 1023);
        let y0 = line.y0.clamp(0, 1023);
        let x1 = line.x1.clamp(0, 1023);
        let y1 = line.y1.clamp(0, 1023);

        // Flip Y: DVG Y=0 is bottom, screen Y=0 is top.
        let sy0 = 1023 - y0;
        let sy1 = 1023 - y1;

        draw_line(buffer, x0, sy0, x1, sy1, brightness);
    }
}

/// Bresenham line drawing with additive blending.
fn draw_line(buffer: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, brightness: u8) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        // Plot pixel with additive blending.
        if (0..1024).contains(&x) && (0..1024).contains(&y) {
            let offset = ((y as usize) * TIMING.display_width as usize + x as usize) * 3;
            buffer[offset] = buffer[offset].saturating_add(brightness);
            buffer[offset + 1] = buffer[offset + 1].saturating_add(brightness);
            buffer[offset + 2] = buffer[offset + 2].saturating_add(brightness);
        }

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
