use phosphor_core::core::machine::{InputButton, TimingConfig};
use phosphor_core::core::save_state::{SaveError, Saveable, StateReader, StateWriter};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::z80::Z80;
use phosphor_core::device::namco_wsg::NamcoWsg;
use phosphor_core::device::namco06::Namco06;
use phosphor_core::device::namco51::Namco51;
use phosphor_core::device::namco53::Namco53;
use phosphor_core::gfx::decode::GfxLayout;

// ---------------------------------------------------------------------------
// Input button IDs (shared across Galaga family)
// ---------------------------------------------------------------------------
pub const INPUT_P1_UP: u8 = 0;
pub const INPUT_P1_RIGHT: u8 = 1;
pub const INPUT_P1_DOWN: u8 = 2;
pub const INPUT_P1_LEFT: u8 = 3;
pub const INPUT_P2_UP: u8 = 4;
pub const INPUT_P2_RIGHT: u8 = 5;
pub const INPUT_P2_DOWN: u8 = 6;
pub const INPUT_P2_LEFT: u8 = 7;
pub const INPUT_P1_BUTTON1: u8 = 8;
pub const INPUT_P2_BUTTON1: u8 = 9;
pub const INPUT_START1: u8 = 10;
pub const INPUT_START2: u8 = 11;
pub const INPUT_COIN1: u8 = 12;
pub const INPUT_COIN2: u8 = 13;
pub const INPUT_SERVICE: u8 = 14;

pub const NAMCO_GALAGA_INPUT_MAP: &[InputButton] = &[
    InputButton {
        id: INPUT_P1_UP,
        name: "P1 Up",
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
        id: INPUT_P1_LEFT,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_P2_UP,
        name: "P2 Up",
    },
    InputButton {
        id: INPUT_P2_RIGHT,
        name: "P2 Right",
    },
    InputButton {
        id: INPUT_P2_DOWN,
        name: "P2 Down",
    },
    InputButton {
        id: INPUT_P2_LEFT,
        name: "P2 Left",
    },
    InputButton {
        id: INPUT_P1_BUTTON1,
        name: "P1 Button",
    },
    InputButton {
        id: INPUT_P2_BUTTON1,
        name: "P2 Button",
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
        id: INPUT_COIN1,
        name: "Coin 1",
    },
    InputButton {
        id: INPUT_COIN2,
        name: "Coin 2",
    },
    InputButton {
        id: INPUT_SERVICE,
        name: "Service",
    },
];

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------
// Master clock:  18.432 MHz
// CPU clock:     18.432 / 6 = 3.072 MHz
// Pixel clock:   18.432 / 3 = 6.144 MHz
// HTOTAL:        384 pixels = 192 CPU cycles per scanline
// VTOTAL:        264 lines
// VBSTART:       224 (visible height)
// Frame:         192 × 264 = 50688 CPU cycles per frame
// Frame rate:    3072000 / 50688 ≈ 60.61 Hz

pub const TIMING: TimingConfig = TimingConfig {
    cpu_clock_hz: 3_072_000,  // 18.432 MHz / 6
    cycles_per_scanline: 192, // 384 pixels / 2
    total_scanlines: 264,     // VTOTAL
    display_width: 224,       // rotated 90° CCW from native 288×224
    display_height: 288,
};

const VISIBLE_LINES: u64 = 224;

/// CPU clock / 06XX clock = 3.072 MHz / 48 kHz = 64.
const NAMCO06_BASE_DIVISOR: u32 = 64;

// Resistor weights for palette PROM (same as Pac-Man)
const R_WEIGHTS: [f64; 3] = [1000.0, 470.0, 220.0];
const G_WEIGHTS: [f64; 3] = [1000.0, 470.0, 220.0];
const B_WEIGHTS: [f64; 2] = [470.0, 220.0];

// ---------------------------------------------------------------------------
// GfxLayout descriptors for Galaga-family hardware
// ---------------------------------------------------------------------------

pub(crate) const GALAGA_SPRITE_LAYOUT: GfxLayout<'static> = GfxLayout {
    plane_offsets: &[4, 0],
    x_offsets: &[
        0, 1, 2, 3, 64, 65, 66, 67, 128, 129, 130, 131, 192, 193, 194, 195,
    ],
    y_offsets: &[
        0, 8, 16, 24, 32, 40, 48, 56, 256, 264, 272, 280, 288, 296, 304, 312,
    ],
    char_increment: 512,
};

// ---------------------------------------------------------------------------
// NamcoGalagaBoard — shared hardware for the Galaga platform
// ---------------------------------------------------------------------------

/// Namco Galaga hardware base (3×Z80 @ 3.072 MHz, Namco WSG, custom I/O chips).
///
/// Shared by Galaga, Dig Dug, Bosconian, and other games on the same PCB.
/// Game wrappers compose this struct, own their RAM arrays, and implement
/// Bus to route memory accesses.
pub struct NamcoGalagaBoard {
    // CPUs
    pub(crate) main_cpu: Z80,
    pub(crate) sub_cpu: Z80,
    pub(crate) sound_cpu: Z80,

    // Per-CPU ROM (each CPU sees different ROM at 0x0000-0x3FFF)
    pub(crate) main_rom: Vec<u8>,
    pub(crate) sub_rom: Vec<u8>,
    pub(crate) sound_rom: Vec<u8>,

    // Devices
    pub(crate) wsg: NamcoWsg,
    pub(crate) namco06: Namco06,
    pub(crate) namco51: Namco51,
    pub(crate) namco53: Namco53,

    // Input ports (active-low: 0xFF = all released)
    pub(crate) in0: u8,
    pub(crate) in1: u8,
    pub(crate) dswa: u8,
    pub(crate) dswb: u8,

    // LS259 misc latch outputs
    pub(crate) main_irq_enabled: bool,  // Q0
    pub(crate) sub_irq_enabled: bool,   // Q1
    pub(crate) sound_nmi_enabled: bool, // Q2 (inverted!)
    pub(crate) sub_reset: bool,         // Q3 (true = sub/sound held in reset)

    // Interrupt state
    pub(crate) main_irq_pending: bool,
    pub(crate) main_nmi_pending: bool, // from 06XX timer
    pub(crate) sub_irq_pending: bool,
    pub(crate) sound_nmi_pending: bool, // from VBLANK, gated by Q2

    // Palette
    pub(crate) palette_prom: [u8; 32],
    pub(crate) palette_rgb: [(u8, u8, u8); 32],

    // Timing
    pub(crate) clock: u64,
    pub(crate) watchdog_counter: u32,
    pub(crate) flip_screen: bool,
}

impl NamcoGalagaBoard {
    pub fn new() -> Self {
        Self {
            main_cpu: Z80::new(),
            sub_cpu: Z80::new(),
            sound_cpu: Z80::new(),

            main_rom: Vec::new(),
            sub_rom: Vec::new(),
            sound_rom: Vec::new(),

            wsg: NamcoWsg::new(TIMING.cpu_clock_hz),
            namco06: Namco06::new(NAMCO06_BASE_DIVISOR),
            namco51: Namco51::new(),
            namco53: Namco53::new(),

            in0: 0xFF,
            in1: 0xFF,
            dswa: 0xFF,
            dswb: 0xFF,

            main_irq_enabled: false,
            sub_irq_enabled: false,
            sound_nmi_enabled: false,
            sub_reset: true, // sub+sound held in reset at power-on

            main_irq_pending: false,
            main_nmi_pending: false,
            sub_irq_pending: false,
            sound_nmi_pending: false,

            palette_prom: [0; 32],
            palette_rgb: [(0, 0, 0); 32],

            clock: 0,
            watchdog_counter: 0,
            flip_screen: false,
        }
    }

    // -----------------------------------------------------------------------
    // Core tick — called from game wrappers via bus_split!
    // -----------------------------------------------------------------------

    pub fn tick(&mut self, bus: &mut dyn Bus<Address = u16, Data = u8>) {
        let frame_cycle = self.clock % TIMING.cycles_per_frame();

        // VBLANK interrupt: fire at the start of VBLANK (scanline 224)
        let vblank_cycle = VISIBLE_LINES * TIMING.cycles_per_scanline;
        if frame_cycle == vblank_cycle {
            self.main_irq_pending = true;
            self.sub_irq_pending = true;
            // Sound CPU NMI from VBLANK, gated by misc latch Q2 (inverted)
            if self.sound_nmi_enabled {
                self.sound_nmi_pending = true;
            }
        }

        // 06XX timer tick — NMI goes to main CPU (ungated)
        self.namco06.tick();
        if self.namco06.take_nmi() {
            self.main_nmi_pending = true;
        }

        // WSG tick (runs at CPU clock rate)
        self.wsg.tick();

        // Execute all 3 CPUs (sub+sound gated by sub_reset)
        self.main_cpu.execute_cycle(bus, BusMaster::Cpu(0));
        if !self.sub_reset {
            self.sub_cpu.execute_cycle(bus, BusMaster::Cpu(1));
            self.sound_cpu.execute_cycle(bus, BusMaster::Cpu(2));
        }

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    // -----------------------------------------------------------------------
    // Bus dispatch helpers — called from game wrapper Bus impls
    // -----------------------------------------------------------------------

    /// Read ROM for the requesting CPU.
    pub fn read_rom(&self, master: BusMaster, addr: u16) -> u8 {
        let offset = addr as usize;
        match master {
            BusMaster::Cpu(0) => self.main_rom.get(offset).copied().unwrap_or(0xFF),
            BusMaster::Cpu(1) => self.sub_rom.get(offset).copied().unwrap_or(0xFF),
            BusMaster::Cpu(2) => self.sound_rom.get(offset).copied().unwrap_or(0xFF),
            _ => 0xFF,
        }
    }

    /// Read the 06XX custom I/O data port. Dispatches to the selected chip
    /// based on the 06XX control register chip-select bits.
    pub fn read_custom_io(&mut self) -> u8 {
        if self.namco06.chip_select(0) {
            // Chip 0: Namco 51XX (input multiplexer)
            self.namco51.read(self.in0, self.in1)
        } else if self.namco06.chip_select(1) {
            // Chip 1: Namco 53XX (DIP switch reader)
            self.namco53.read(self.dswa, self.dswb)
        } else {
            0xFF
        }
    }

    /// Write the 06XX custom I/O data port. Dispatches to the selected chip.
    pub fn write_custom_io(&mut self, data: u8) {
        if self.namco06.chip_select(0) {
            self.namco51.write(data);
        }
        // 53XX has no write interface
    }

    /// Write the LS259 misc latch at 0x6820-0x6827.
    /// `bit` is address & 7, `value` is data bit 0.
    pub fn write_misc_latch(&mut self, bit: u8, value: bool) {
        match bit {
            0 => {
                self.main_irq_enabled = value;
                if !value {
                    self.main_irq_pending = false;
                }
            }
            1 => {
                self.sub_irq_enabled = value;
                if !value {
                    self.sub_irq_pending = false;
                }
            }
            2 => {
                // Sound NMI enable is INVERTED: writing 0 enables NMI
                self.sound_nmi_enabled = !value;
            }
            3 => {
                // Sub/sound CPU reset: 0 = held in reset, 1 = running
                let was_reset = self.sub_reset;
                self.sub_reset = !value;

                // When releasing from reset, actually reset the CPUs
                if was_reset && !self.sub_reset {
                    // CPUs will be reset by the game wrapper (needs bus_split)
                }
            }
            7 => {
                self.flip_screen = value;
            }
            _ => {} // 4-6: game-specific (mod_bits, LEDs, etc.)
        }
    }

    /// Check interrupt state for a given CPU.
    pub fn check_interrupts(
        &mut self,
        target: BusMaster,
    ) -> phosphor_core::core::bus::InterruptState {
        use phosphor_core::core::bus::InterruptState;
        match target {
            BusMaster::Cpu(0) => {
                let irq = self.main_irq_pending && self.main_irq_enabled;
                if irq {
                    self.main_irq_pending = false;
                }
                let nmi = self.main_nmi_pending;
                if nmi {
                    self.main_nmi_pending = false;
                }
                InterruptState {
                    irq,
                    nmi,
                    ..Default::default()
                }
            }
            BusMaster::Cpu(1) => {
                let irq = self.sub_irq_pending && self.sub_irq_enabled;
                if irq {
                    self.sub_irq_pending = false;
                }
                InterruptState {
                    irq,
                    ..Default::default()
                }
            }
            BusMaster::Cpu(2) => {
                let nmi = self.sound_nmi_pending;
                if nmi {
                    self.sound_nmi_pending = false;
                }
                InterruptState {
                    nmi,
                    ..Default::default()
                }
            }
            _ => InterruptState::default(),
        }
    }

    /// Check if a CPU is halted (sub+sound halted when sub_reset is true).
    pub fn is_halted_for(&self, master: BusMaster) -> bool {
        match master {
            BusMaster::Cpu(1) | BusMaster::Cpu(2) => self.sub_reset,
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Palette
    // -----------------------------------------------------------------------

    /// Pre-compute the 32-entry RGB palette from the palette PROM using
    /// resistor-weighted DAC values (same resistor network as Pac-Man).
    pub fn build_palette(&mut self) {
        use phosphor_core::gfx::{combine_weights, compute_resistor_weights};

        let r_w = compute_resistor_weights(&R_WEIGHTS, None);
        let g_w = compute_resistor_weights(&G_WEIGHTS, None);
        let b_w = compute_resistor_weights(&B_WEIGHTS, None);

        for i in 0..32 {
            let entry = self.palette_prom[i];

            let r = combine_weights(&r_w, &[entry & 1, (entry >> 1) & 1, (entry >> 2) & 1]);
            let g = combine_weights(
                &g_w,
                &[(entry >> 3) & 1, (entry >> 4) & 1, (entry >> 5) & 1],
            );
            let b = combine_weights(&b_w, &[(entry >> 6) & 1, (entry >> 7) & 1]);

            self.palette_rgb[i] = (r, g, b);
        }
    }

    // -----------------------------------------------------------------------
    // ROM loading helpers
    // -----------------------------------------------------------------------

    pub fn load_main_rom(&mut self, data: &[u8]) {
        self.main_rom = data.to_vec();
    }

    pub fn load_sub_rom(&mut self, data: &[u8]) {
        self.sub_rom = data.to_vec();
    }

    pub fn load_sound_rom(&mut self, data: &[u8]) {
        self.sound_rom = data.to_vec();
    }

    pub fn load_palette_prom(&mut self, data: &[u8]) {
        let len = data.len().min(32);
        self.palette_prom[..len].copy_from_slice(&data[..len]);
        self.build_palette();
    }

    pub fn load_sound_prom(&mut self, data: &[u8]) {
        self.wsg.load_waveform_rom(data);
    }

    // -----------------------------------------------------------------------
    // Input handling
    // -----------------------------------------------------------------------

    /// Dispatch an input event to the appropriate port bit (active-low).
    pub fn handle_input(&mut self, button: u8, pressed: bool) {
        match button {
            INPUT_P1_UP => crate::set_bit_active_low(&mut self.in0, 0, pressed),
            INPUT_P1_RIGHT => crate::set_bit_active_low(&mut self.in0, 1, pressed),
            INPUT_P1_DOWN => crate::set_bit_active_low(&mut self.in0, 2, pressed),
            INPUT_P1_LEFT => crate::set_bit_active_low(&mut self.in0, 3, pressed),
            INPUT_P2_UP => crate::set_bit_active_low(&mut self.in0, 4, pressed),
            INPUT_P2_RIGHT => crate::set_bit_active_low(&mut self.in0, 5, pressed),
            INPUT_P2_DOWN => crate::set_bit_active_low(&mut self.in0, 6, pressed),
            INPUT_P2_LEFT => crate::set_bit_active_low(&mut self.in0, 7, pressed),
            INPUT_P1_BUTTON1 => crate::set_bit_active_low(&mut self.in1, 0, pressed),
            INPUT_P2_BUTTON1 => crate::set_bit_active_low(&mut self.in1, 1, pressed),
            INPUT_START1 => crate::set_bit_active_low(&mut self.in1, 2, pressed),
            INPUT_START2 => crate::set_bit_active_low(&mut self.in1, 3, pressed),
            INPUT_COIN1 => crate::set_bit_active_low(&mut self.in1, 4, pressed),
            INPUT_COIN2 => crate::set_bit_active_low(&mut self.in1, 5, pressed),
            INPUT_SERVICE => crate::set_bit_active_low(&mut self.in1, 6, pressed),
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Audio
    // -----------------------------------------------------------------------

    pub fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        self.wsg.fill_audio(buffer)
    }

    // -----------------------------------------------------------------------
    // Reset
    // -----------------------------------------------------------------------

    /// Reset all board state except ROMs and palette PROMs.
    /// The caller must reset CPUs separately (requires bus_split).
    pub fn reset_board(&mut self) {
        self.wsg.reset();
        self.namco06.reset();
        self.namco51.reset();
        self.namco53.reset();

        self.in0 = 0xFF;
        self.in1 = 0xFF;

        self.main_irq_enabled = false;
        self.sub_irq_enabled = false;
        self.sound_nmi_enabled = false;
        self.sub_reset = true;

        self.main_irq_pending = false;
        self.main_nmi_pending = false;
        self.sub_irq_pending = false;
        self.sound_nmi_pending = false;

        self.clock = 0;
        self.watchdog_counter = 0;
        self.flip_screen = false;
    }

    // -----------------------------------------------------------------------
    // Debug
    // -----------------------------------------------------------------------

    pub fn debug_tick_boundaries(&self) -> u32 {
        let mut mask = 0u32;
        if self.main_cpu.at_instruction_boundary() {
            mask |= 1;
        }
        if !self.sub_reset && self.sub_cpu.at_instruction_boundary() {
            mask |= 2;
        }
        if !self.sub_reset && self.sound_cpu.at_instruction_boundary() {
            mask |= 4;
        }
        mask
    }

    // -----------------------------------------------------------------------
    // Save / Load state
    // -----------------------------------------------------------------------

    pub(crate) fn save_board_state(&self, w: &mut StateWriter) {
        // CPUs
        self.main_cpu.save_state(w);
        self.sub_cpu.save_state(w);
        self.sound_cpu.save_state(w);

        // Devices
        self.wsg.save_state(w);
        self.namco06.save_state(w);
        self.namco51.save_state(w);
        self.namco53.save_state(w);

        // I/O state
        w.write_u8(self.in0);
        w.write_u8(self.in1);
        w.write_u8(self.dswa);
        w.write_u8(self.dswb);

        // Latch + interrupt state
        w.write_bool(self.main_irq_enabled);
        w.write_bool(self.sub_irq_enabled);
        w.write_bool(self.sound_nmi_enabled);
        w.write_bool(self.sub_reset);
        w.write_bool(self.main_irq_pending);
        w.write_bool(self.main_nmi_pending);
        w.write_bool(self.sub_irq_pending);
        w.write_bool(self.sound_nmi_pending);
        w.write_bool(self.flip_screen);

        // Timing
        w.write_u64_le(self.clock);
        w.write_u32_le(self.watchdog_counter);
    }

    pub(crate) fn load_board_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        // CPUs
        self.main_cpu.load_state(r)?;
        self.sub_cpu.load_state(r)?;
        self.sound_cpu.load_state(r)?;

        // Devices
        self.wsg.load_state(r)?;
        self.namco06.load_state(r)?;
        self.namco51.load_state(r)?;
        self.namco53.load_state(r)?;

        // I/O state
        self.in0 = r.read_u8()?;
        self.in1 = r.read_u8()?;
        self.dswa = r.read_u8()?;
        self.dswb = r.read_u8()?;

        // Latch + interrupt state
        self.main_irq_enabled = r.read_bool()?;
        self.sub_irq_enabled = r.read_bool()?;
        self.sound_nmi_enabled = r.read_bool()?;
        self.sub_reset = r.read_bool()?;
        self.main_irq_pending = r.read_bool()?;
        self.main_nmi_pending = r.read_bool()?;
        self.sub_irq_pending = r.read_bool()?;
        self.sound_nmi_pending = r.read_bool()?;
        self.flip_screen = r.read_bool()?;

        // Timing
        self.clock = r.read_u64_le()?;
        self.watchdog_counter = r.read_u32_le()?;

        Ok(())
    }
}

impl Default for NamcoGalagaBoard {
    fn default() -> Self {
        Self::new()
    }
}
