use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::save_state::{self, SaveError, Saveable, StateWriter};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::Cpu;
use phosphor_core::cpu::m6502::M6502;
use phosphor_core::device::dvg::{Dvg, VectorLine};

use crate::registry::MachineEntry;
use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};

// ---------------------------------------------------------------------------
// ROM definitions (MAME `asteroid` parent set)
// ---------------------------------------------------------------------------

/// Program ROM: 6KB at CPU addresses 0x6800–0x7FFF.
static PROGRAM_ROM: RomRegion = RomRegion {
    size: 0x1800,
    entries: &[
        RomEntry {
            name: "035145-04e.ef2",
            size: 0x0800,
            offset: 0x0000,
            crc32: &[0xb503eaf7],
        },
        RomEntry {
            name: "035144-04e.h2",
            size: 0x0800,
            offset: 0x0800,
            crc32: &[0x25233192],
        },
        RomEntry {
            name: "035143-02.j2",
            size: 0x0800,
            offset: 0x1000,
            crc32: &[0x312caa02],
        },
    ],
};

/// Vector ROM: 2KB at CPU address 0x5000–0x57FF.
static VECTOR_ROM: RomRegion = RomRegion {
    size: 0x0800,
    entries: &[RomEntry {
        name: "035127-02.np3",
        size: 0x0800,
        offset: 0x0000,
        crc32: &[0x8b71fd9e],
    }],
};

// ---------------------------------------------------------------------------
// Input button IDs
// ---------------------------------------------------------------------------

pub const INPUT_COIN: u8 = 0;
pub const INPUT_START1: u8 = 1;
pub const INPUT_START2: u8 = 2;
pub const INPUT_THRUST: u8 = 3;
pub const INPUT_FIRE: u8 = 4;
pub const INPUT_HYPERSPACE: u8 = 5;
pub const INPUT_ROT_LEFT: u8 = 6;
pub const INPUT_ROT_RIGHT: u8 = 7;

const ASTEROIDS_INPUT_MAP: &[InputButton] = &[
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
    InputButton {
        id: INPUT_THRUST,
        name: "P1 Up",
    },
    InputButton {
        id: INPUT_FIRE,
        name: "P1 Fire",
    },
    InputButton {
        id: INPUT_HYPERSPACE,
        name: "P1 Jump",
    },
    InputButton {
        id: INPUT_ROT_LEFT,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_ROT_RIGHT,
        name: "P1 Right",
    },
];

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------

// Master clock: 12.096 MHz
// CPU clock: 12.096 / 8 = 1.512 MHz
// NMI: 3 KHz / 12 ≈ 250 Hz → every ~6048 CPU cycles
// Frame: ~60 Hz → ~25200 CPU cycles
const CPU_CLOCK_HZ: u64 = 1_512_000;
const CYCLES_PER_FRAME: u64 = CPU_CLOCK_HZ / 60;
const NMI_PERIOD_CYCLES: u64 = CPU_CLOCK_HZ / 250;

// ---------------------------------------------------------------------------
// Display constants
// ---------------------------------------------------------------------------

const DISPLAY_WIDTH: u32 = 1024;
const DISPLAY_HEIGHT: u32 = 1024;

// ---------------------------------------------------------------------------
// Asteroids system
// ---------------------------------------------------------------------------

/// Asteroids Arcade System (Atari, 1979)
///
/// Hardware: MOS 6502 @ 1.512 MHz, Atari DVG vector display.
/// Video: 1024×1024 vector display via Digital Vector Generator.
/// Audio: Discrete circuits (explosion, thrust, fire, saucer, thump) — stubbed.
///
/// Memory map (15-bit address bus, `addr & 0x7FFF`):
///   0x0000–0x03FF  RAM (1 KB)
///   0x2000–0x2007  IN0 read (buttons, 3 KHz clock, VG_HALT)
///   0x2400–0x2407  IN1 read (coins, start, thrust, rotate)
///   0x2800–0x2803  DSW1 read (DIP switches)
///   0x3000         DVG GO write
///   0x3200         Output latch write (74LS259)
///   0x3400         Watchdog reset write
///   0x3600         Explosion sound write
///   0x3A00         Thump sound write
///   0x3C00–0x3C07  Audio latch write (74LS259)
///   0x3E00         Noise reset write
///   0x4000–0x47FF  Vector RAM (2 KB, shared CPU/DVG)
///   0x5000–0x57FF  Vector ROM (2 KB)
///   0x6800–0x7FFF  Program ROM (6 KB)
pub struct AsteroidsSystem {
    cpu: M6502,
    dvg: Dvg,

    // Memory
    ram: [u8; 0x0400],
    vector_ram: [u8; 0x0800],
    vector_rom: [u8; 0x0800],
    program_rom: [u8; 0x1800],

    // I/O — active-HIGH inputs (default 0x00 = all released)
    in0: u8,
    in1: u8,
    /// DIP switches: default 0x84 (English, 3 lives, 1 coin/1 credit).
    dip_switches: u8,

    // NMI timing
    clock: u64,
    nmi_counter: u64,
    nmi_pending: bool,

    // Watchdog (resets if not written within 8 frames)
    watchdog_frame_count: u8,

    // Vector display
    display_list: Vec<VectorLine>,
}

impl AsteroidsSystem {
    pub fn new() -> Self {
        Self {
            cpu: M6502::new(),
            dvg: Dvg::new(),
            ram: [0; 0x0400],
            vector_ram: [0; 0x0800],
            vector_rom: [0; 0x0800],
            program_rom: [0; 0x1800],
            in0: 0x00,
            in1: 0x00,
            dip_switches: 0x84, // English, 3 lives, 1C/1C
            clock: 0,
            nmi_counter: 0,
            nmi_pending: false,
            watchdog_frame_count: 0,
            display_list: Vec::with_capacity(512),
        }
    }

    pub fn load_rom_set(&mut self, rom_set: &RomSet) -> Result<(), RomLoadError> {
        let prog = PROGRAM_ROM.load(rom_set)?;
        self.program_rom.copy_from_slice(&prog);
        let vrom = VECTOR_ROM.load(rom_set)?;
        self.vector_rom.copy_from_slice(&vrom);
        Ok(())
    }

    fn tick(&mut self) {
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
        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
        }

        self.clock += 1;
    }

    /// Trigger the DVG: assemble vector memory and run to completion.
    ///
    /// The DVG has a 13-bit (8 KB) byte address space:
    ///   0x0000–0x07FF  Vector RAM (2 KB, CPU addresses 0x4000–0x47FF)
    ///   0x0800–0x0FFF  Unmapped gap
    ///   0x1000–0x17FF  Vector ROM (2 KB, CPU addresses 0x5000–0x57FF)
    ///   0x1800–0x1FFF  Unmapped
    fn trigger_dvg(&mut self) {
        let mut vmem = vec![0u8; 0x2000]; // 8 KB DVG address space
        vmem[0x0000..0x0800].copy_from_slice(&self.vector_ram);
        vmem[0x1000..0x1800].copy_from_slice(&self.vector_rom);
        self.dvg.go();
        self.dvg.execute(&vmem);
        self.display_list = self.dvg.take_display_list();
    }
}

impl Default for AsteroidsSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bus implementation
// ---------------------------------------------------------------------------

impl Bus for AsteroidsSystem {
    type Address = u16;
    type Data = u8;

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        // 15-bit address bus (MAME: map.global_mask(0x7fff))
        let addr = addr & 0x7FFF;

        match addr {
            // RAM: 0x0000–0x03FF (with mirror at 0x0400–0x07FF per MAME bankrw)
            0x0000..=0x03FF => self.ram[addr as usize],
            0x0400..=0x07FF => self.ram[(addr - 0x0400) as usize],

            // IN0: 0x2000–0x2007 — 74LS251 8:1 multiplexer.
            // A0–A2 select which input bit to read; the selected bit appears on D7.
            // The 6502 tests it via BIT (N flag = D7).
            //   Bit 0: unused
            //   Bit 1: 3 KHz clock (cpu total_cycles & 0x100)
            //   Bit 2: VG_HALT (active-LOW: 0 = done, 1 = running)
            //   Bit 3: Hyperspace     Bit 4: Fire
            //   Bit 5: Diagnostic     Bit 6: Tilt     Bit 7: Self-test
            0x2000..=0x2007 => {
                let offset = (addr & 7) as u8;
                let mut val = self.in0;
                // Bit 1: 3 KHz clock
                if self.clock & 0x100 != 0 {
                    val |= 0x02;
                } else {
                    val &= !0x02;
                }
                // Bit 2: VG_HALT (0 = halted/done, 1 = running)
                if !self.dvg.is_halted() {
                    val |= 0x04;
                } else {
                    val &= !0x04;
                }
                // Mux: selected bit → D7
                ((val >> offset) & 1) << 7
            }

            // IN1: 0x2400–0x2407 — 74LS251 8:1 multiplexer.
            // Same as IN0: A0–A2 select bit, result in D7.
            //   Bit 0: Left coin   Bit 1: Center coin   Bit 2: Right coin
            //   Bit 3: 1P Start    Bit 4: 2P Start
            //   Bit 5: Thrust      Bit 6: Rotate right   Bit 7: Rotate left
            0x2400..=0x2407 => {
                let offset = (addr & 7) as u8;
                ((self.in1 >> offset) & 1) << 7
            }

            // DSW1: 0x2800–0x2803 — 74LS253 dual 4:1 multiplexer.
            // A0–A1 select a pair of DIP switch bits: even bit → D0, odd bit → D7.
            0x2800..=0x2803 => {
                let offset = (addr & 3) as u8;
                let bit0 = (self.dip_switches >> (offset * 2)) & 1;
                let bit7 = (self.dip_switches >> (offset * 2 + 1)) & 1;
                bit0 | (bit7 << 7)
            }

            // Vector RAM: 0x4000–0x47FF
            0x4000..=0x47FF => self.vector_ram[(addr - 0x4000) as usize],

            // Vector ROM: 0x5000–0x57FF
            0x5000..=0x57FF => self.vector_rom[(addr - 0x5000) as usize],

            // Program ROM: 0x6800–0x7FFF
            0x6800..=0x7FFF => self.program_rom[(addr - 0x6800) as usize],

            _ => 0,
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        let addr = addr & 0x7FFF;

        match addr {
            // RAM
            0x0000..=0x03FF => self.ram[addr as usize] = data,
            0x0400..=0x07FF => self.ram[(addr - 0x0400) as usize] = data,

            // DVG GO
            0x3000 => self.trigger_dvg(),

            // Output latch (74LS259): LEDs, coin counters, RAMSEL
            0x3200 => { /* stub */ }

            // Watchdog reset
            0x3400 => self.watchdog_frame_count = 0,

            // Explosion sound
            0x3600 => { /* audio stub */ }

            // Thump sound
            0x3A00 => { /* audio stub */ }

            // Audio latch (74LS259 discrete sound control)
            0x3C00..=0x3C07 => { /* audio stub */ }

            // Noise reset
            0x3E00 => { /* audio stub */ }

            // Vector RAM
            0x4000..=0x47FF => self.vector_ram[(addr - 0x4000) as usize] = data,

            _ => {}
        }
    }

    fn check_interrupts(&self, _target: BusMaster) -> InterruptState {
        InterruptState {
            nmi: self.nmi_pending,
            irq: false,
            firq: false,
            irq_vector: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Machine implementation
// ---------------------------------------------------------------------------

impl Machine for AsteroidsSystem {
    fn display_size(&self) -> (u32, u32) {
        (DISPLAY_WIDTH, DISPLAY_HEIGHT)
    }

    fn run_frame(&mut self) {
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }

        // Clear NMI at frame boundary to avoid stale edges.
        self.nmi_pending = false;

        // Watchdog
        self.watchdog_frame_count += 1;
        if self.watchdog_frame_count >= 8 {
            self.reset();
        }
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        rasterize_vectors(&self.display_list, buffer);
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // IN1 (active-HIGH: set bit on press, clear on release)
            INPUT_COIN => set_bit_active_high(&mut self.in1, 0, pressed),
            INPUT_START1 => set_bit_active_high(&mut self.in1, 3, pressed),
            INPUT_START2 => set_bit_active_high(&mut self.in1, 4, pressed),
            INPUT_THRUST => set_bit_active_high(&mut self.in1, 5, pressed),
            INPUT_ROT_RIGHT => set_bit_active_high(&mut self.in1, 6, pressed),
            INPUT_ROT_LEFT => set_bit_active_high(&mut self.in1, 7, pressed),

            // IN0 (active-HIGH)
            INPUT_FIRE => set_bit_active_high(&mut self.in0, 4, pressed),
            INPUT_HYPERSPACE => set_bit_active_high(&mut self.in0, 3, pressed),

            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        ASTEROIDS_INPUT_MAP
    }

    fn reset(&mut self) {
        self.dvg.reset();
        self.nmi_pending = false;
        self.nmi_counter = 0;
        self.watchdog_frame_count = 0;
        self.display_list.clear();

        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            self.cpu.reset(bus, BusMaster::Cpu(0));
        }
    }

    fn frame_rate_hz(&self) -> f64 {
        60.0
    }

    fn machine_id(&self) -> &str {
        "asteroids"
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        let mut w = StateWriter::new();
        save_state::write_header(&mut w, self.machine_id());
        self.cpu.save_state(&mut w);
        self.dvg.save_state(&mut w);
        w.write_bytes(&self.ram);
        w.write_bytes(&self.vector_ram);
        w.write_u8(self.in0);
        w.write_u8(self.in1);
        w.write_u64_le(self.clock);
        w.write_u64_le(self.nmi_counter);
        w.write_bool(self.nmi_pending);
        w.write_u8(self.watchdog_frame_count);
        Some(w.into_vec())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), SaveError> {
        let mut r = save_state::read_header(data, self.machine_id())?;
        self.cpu.load_state(&mut r)?;
        self.dvg.load_state(&mut r)?;
        r.read_bytes_into(&mut self.ram)?;
        r.read_bytes_into(&mut self.vector_ram)?;
        self.in0 = r.read_u8()?;
        self.in1 = r.read_u8()?;
        self.clock = r.read_u64_le()?;
        self.nmi_counter = r.read_u64_le()?;
        self.nmi_pending = r.read_bool()?;
        self.watchdog_frame_count = r.read_u8()?;
        self.display_list.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Input helpers
// ---------------------------------------------------------------------------

/// Active-HIGH bit manipulation: set bit on press, clear on release.
fn set_bit_active_high(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg |= 1 << bit;
    } else {
        *reg &= !(1 << bit);
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
            let offset = ((y as usize) * DISPLAY_WIDTH as usize + x as usize) * 3;
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

// ---------------------------------------------------------------------------
// Machine registry
// ---------------------------------------------------------------------------

fn create_machine(rom_set: &RomSet) -> Result<Box<dyn Machine>, RomLoadError> {
    let mut sys = AsteroidsSystem::new();
    sys.load_rom_set(rom_set)?;
    Ok(Box::new(sys))
}

inventory::submit! {
    MachineEntry::new("asteroid", "asteroid", create_machine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::core::machine::Machine;
    use phosphor_core::cpu::CpuStateTrait;

    #[test]
    fn save_load_round_trip() {
        let mut sys = AsteroidsSystem::new();

        // Set known state
        sys.ram[0x100] = 0xAA;
        sys.vector_ram[0x200] = 0xBB;
        sys.in0 = 0x18;
        sys.in1 = 0xE8;
        sys.clock = 75_000;
        sys.nmi_counter = 3000;
        sys.nmi_pending = true;
        sys.watchdog_frame_count = 5;

        // Save
        let data = sys.save_state().expect("save_state should return Some");
        let cpu_snap = sys.cpu.snapshot();

        // Mutate everything
        let mut sys2 = AsteroidsSystem::new();
        sys2.ram[0x100] = 0xFF;
        sys2.in0 = 0xFF;
        sys2.clock = 999;

        // Load
        sys2.load_state(&data).unwrap();

        // Verify CPU
        assert_eq!(sys2.cpu.snapshot(), cpu_snap);

        // Verify memory
        assert_eq!(sys2.ram[0x100], 0xAA);
        assert_eq!(sys2.vector_ram[0x200], 0xBB);

        // Verify I/O and timing state
        assert_eq!(sys2.in0, 0x18);
        assert_eq!(sys2.in1, 0xE8);
        assert_eq!(sys2.clock, 75_000);
        assert_eq!(sys2.nmi_counter, 3000);
        assert!(sys2.nmi_pending);
        assert_eq!(sys2.watchdog_frame_count, 5);

        // Transient state should be cleared
        assert!(sys2.display_list.is_empty());
    }

    #[test]
    fn save_load_machine_id_validated() {
        let sys = AsteroidsSystem::new();
        let data = sys.save_state().unwrap();

        // Tamper with the machine ID
        let mut bad = data.clone();
        let id_offset = 4 + 4 + 4; // magic(4) + version(4) + id_len(4)
        bad[id_offset..id_offset + 9].copy_from_slice(b"xxxxxxxxx");

        let mut sys2 = AsteroidsSystem::new();
        let result = sys2.load_state(&bad);
        assert!(result.is_err(), "should reject mismatched machine ID");
    }

    #[test]
    fn save_does_not_include_rom() {
        let mut sys = AsteroidsSystem::new();
        sys.program_rom[0] = 0xDE;
        sys.vector_rom[0] = 0xAD;

        let data = sys.save_state().unwrap();

        // Load into a fresh system (ROMs are zeroed)
        let mut sys2 = AsteroidsSystem::new();
        sys2.load_state(&data).unwrap();

        // ROMs should remain at their default (zeroed), not overwritten
        assert_eq!(sys2.program_rom[0], 0x00);
        assert_eq!(sys2.vector_rom[0], 0x00);
    }
}
