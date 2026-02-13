use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{InputButton, Machine};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::m6809::M6809;
use phosphor_core::cpu::state::M6809State;
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::cmos_ram::CmosRam;
use phosphor_core::device::pia6820::Pia6820;
use phosphor_core::device::williams_blitter::WilliamsBlitter;

use crate::rom_loader::{RomEntry, RomRegion};

/// Joust program ROM region: 12KB at 0xD000-0xFFFF, split across three 4KB files.
pub static JOUST_PROGRAM_ROM: RomRegion = RomRegion {
    size: 0x3000, // 12KB
    entries: &[
        RomEntry {
            name: "joust.wg1",
            size: 0x1000,
            offset: 0x0000, // -> 0xD000-0xDFFF
            crc32: None,
        },
        RomEntry {
            name: "joust.wg2",
            size: 0x1000,
            offset: 0x1000, // -> 0xE000-0xEFFF
            crc32: None,
        },
        RomEntry {
            name: "joust.wg3",
            size: 0x1000,
            offset: 0x2000, // -> 0xF000-0xFFFF
            crc32: None,
        },
    ],
};

// Widget PIA Port A — player controls (active-low)
pub const INPUT_P1_RIGHT: u8 = 0;
pub const INPUT_P1_LEFT: u8 = 1;
pub const INPUT_P1_FLAP: u8 = 2;
pub const INPUT_P2_RIGHT: u8 = 3;
pub const INPUT_P2_LEFT: u8 = 4;
pub const INPUT_P2_FLAP: u8 = 5;
pub const INPUT_P1_START: u8 = 6;
pub const INPUT_P2_START: u8 = 7;

// Widget PIA Port B / control lines
pub const INPUT_COIN: u8 = 8;

const JOUST_INPUT_MAP: &[InputButton] = &[
    InputButton {
        id: INPUT_P1_RIGHT,
        name: "P1 Right",
    },
    InputButton {
        id: INPUT_P1_LEFT,
        name: "P1 Left",
    },
    InputButton {
        id: INPUT_P1_FLAP,
        name: "P1 Flap",
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
        id: INPUT_P2_FLAP,
        name: "P2 Flap",
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
];

/// Williams 2nd-generation arcade board configured for Joust (1982)
///
/// Hardware: Motorola 6809E @ 1 MHz, 48KB video RAM, two MC6821 PIAs,
/// Williams SC1 blitter, 1KB battery-backed CMOS RAM, 12KB program ROM.
pub struct JoustSystem {
    // CPU
    cpu: M6809,

    // Memory regions
    video_ram: [u8; 0xC000],   // 0x0000-0xBFFF: 48KB video/color RAM
    palette_ram: [u8; 16],     // 0xC000-0xC00F: 16-color palette
    cmos_ram: CmosRam,         // 0xCC00-0xCFFF: 1KB battery-backed
    program_rom: [u8; 0x3000], // 0xD000-0xFFFF: 12KB program ROM

    // Peripheral devices
    widget_pia: Pia6820,      // 0xC804-0xC807: player inputs, coins, sound
    rom_pia: Pia6820,         // 0xC80C-0xC80F: ROM bank, screen flip
    blitter: WilliamsBlitter, // 0xCA00-0xCA07: DMA blitter

    // I/O registers
    rom_bank: u8, // 0xC900: ROM bank select

    // System state
    watchdog_counter: u32, // Reset by read/write to 0xCB00
    clock: u64,            // Master clock cycle counter

    // Input state (active-low: 0 = pressed, 1 = released, default all released)
    input_port_a: u8, // Widget PIA Port A: player buttons
    input_port_b: u8, // Widget PIA Port B: coins, DIP switches
}

impl JoustSystem {
    pub fn new() -> Self {
        Self {
            cpu: M6809::new(),
            video_ram: [0; 0xC000],
            palette_ram: [0; 16],
            cmos_ram: CmosRam::new(),
            program_rom: [0; 0x3000],
            widget_pia: Pia6820::new(),
            rom_pia: Pia6820::new(),
            blitter: WilliamsBlitter::new(),
            rom_bank: 0,
            watchdog_counter: 0,
            clock: 0,
            input_port_a: 0xFF, // All buttons released (active-low)
            input_port_b: 0xFF,
        }
    }

    pub fn tick(&mut self) {
        let vblank_cycle = self.clock % 16667;
        if vblank_cycle == 0 {
            self.widget_pia.set_cb1(true); // VBLANK start
        } else if vblank_cycle == 100 {
            self.widget_pia.set_cb1(false); // VBLANK end (~100us pulse)
        }

        if self.blitter.is_active() {
            self.blitter.do_dma_cycle(&mut self.video_ram);
        } else {
            let bus_ptr: *mut Self = self;
            unsafe {
                let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
                self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
            }
        }

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    /// Load program ROM from a byte slice at the given offset (for testing).
    /// Offset is relative to the start of the ROM region (0 = address 0xD000).
    pub fn load_program_rom(&mut self, offset: usize, data: &[u8]) {
        let end = (offset + data.len()).min(self.program_rom.len());
        let len = end - offset;
        self.program_rom[offset..end].copy_from_slice(&data[..len]);
    }

    /// Load program ROM from a RomSet using the Joust ROM mapping.
    pub fn load_rom_set(
        &mut self,
        rom_set: &crate::rom_loader::RomSet,
    ) -> Result<(), crate::rom_loader::RomLoadError> {
        let rom_data = JOUST_PROGRAM_ROM.load(rom_set)?;
        self.program_rom.copy_from_slice(&rom_data);
        Ok(())
    }

    pub fn get_cpu_state(&self) -> M6809State {
        self.cpu.snapshot()
    }

    pub fn read_video_ram(&self, addr: usize) -> u8 {
        if addr < self.video_ram.len() {
            self.video_ram[addr]
        } else {
            0
        }
    }

    pub fn write_video_ram(&mut self, addr: usize, data: u8) {
        if addr < self.video_ram.len() {
            self.video_ram[addr] = data;
        }
    }

    pub fn read_palette(&self, index: usize) -> u8 {
        if index < 16 {
            self.palette_ram[index]
        } else {
            0
        }
    }

    pub fn rom_bank(&self) -> u8 {
        self.rom_bank
    }

    pub fn clock(&self) -> u64 {
        self.clock
    }

    pub fn load_cmos(&mut self, data: &[u8]) {
        self.cmos_ram.load_from(data);
    }

    pub fn save_cmos(&self) -> &[u8; 1024] {
        self.cmos_ram.snapshot()
    }
}

impl Default for JoustSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for JoustSystem {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.video_ram[addr as usize],
            0xC000..=0xC00F => self.palette_ram[(addr - 0xC000) as usize],
            0xC804..=0xC807 => self.widget_pia.read((addr - 0xC804) as u8),
            0xC80C..=0xC80F => self.rom_pia.read((addr - 0xC80C) as u8),
            0xC900 => self.rom_bank,
            0xCA00..=0xCA07 => self.blitter.read_register((addr - 0xCA00) as u8),
            0xCB00 => {
                self.watchdog_counter = 0;
                0
            }
            0xCC00..=0xCFFF => self.cmos_ram.read(addr - 0xCC00),
            0xD000..=0xFFFF => self.program_rom[(addr - 0xD000) as usize],
            _ => 0xFF,
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        match addr {
            0x0000..=0xBFFF => self.video_ram[addr as usize] = data,
            0xC000..=0xC00F => self.palette_ram[(addr - 0xC000) as usize] = data,
            0xC804..=0xC807 => self.widget_pia.write((addr - 0xC804) as u8, data),
            0xC80C..=0xC80F => self.rom_pia.write((addr - 0xC80C) as u8, data),
            0xC900 => self.rom_bank = data,
            0xCA00..=0xCA07 => self.blitter.write_register((addr - 0xCA00) as u8, data),
            0xCB00 => self.watchdog_counter = 0,
            0xCC00..=0xCFFF => self.cmos_ram.write(addr - 0xCC00, data),
            0xD000..=0xFFFF => { /* ROM: ignored */ }
            _ => { /* Unmapped: ignored */ }
        }
    }

    fn is_halted_for(&self, master: BusMaster) -> bool {
        match master {
            BusMaster::Cpu(0) => self.blitter.is_active(),
            _ => false,
        }
    }

    fn check_interrupts(&self, target: BusMaster) -> InterruptState {
        match target {
            BusMaster::Cpu(0) => InterruptState {
                nmi: false,
                irq: self.widget_pia.irq_b(),
                firq: self.rom_pia.irq_a() || self.rom_pia.irq_b(),
            },
            _ => InterruptState::default(),
        }
    }
}

impl Machine for JoustSystem {
    fn display_size(&self) -> (u32, u32) {
        (292, 240)
    }

    fn run_frame(&mut self) {
        self.widget_pia.set_port_a_input(self.input_port_a);
        self.widget_pia.set_port_b_input(self.input_port_b);

        for _ in 0..16667 {
            self.tick();
        }
    }

    fn render_frame(&self, buffer: &mut [u8]) {
        let (width, height) = self.display_size();
        let w = width as usize;
        let h = height as usize;

        // Williams palette: each of the 16 entries is an 8-bit byte encoding RGB
        // in a 3-3-2 format:
        //   Bits 7-5: Red   (3 bits, 0-7) -> scaled to 0-255 via r * 255 / 7
        //   Bits 4-2: Green (3 bits, 0-7) -> scaled to 0-255 via g * 255 / 7
        //   Bits 1-0: Blue  (2 bits, 0-3) -> scaled to 0-255 via b * 255 / 3
        let mut palette_rgb = [(0u8, 0u8, 0u8); 16];
        for (i, rgb) in palette_rgb.iter_mut().enumerate() {
            let entry = self.palette_ram[i] as u16;
            // Widen to u16 before multiply to avoid overflow (e.g. 7 * 255 = 1785)
            *rgb = (
                (((entry >> 5) & 0x07) * 255 / 7) as u8,
                (((entry >> 2) & 0x07) * 255 / 7) as u8,
                ((entry & 0x03) * 255 / 3) as u8,
            );
        }

        // Williams video RAM is organized in column-major order with 2 pixels per byte.
        // Each byte holds two horizontally-adjacent 4-bit pixels:
        //   Upper nibble (bits 7-4) = color index for the even (left) pixel
        //   Lower nibble (bits 3-0) = color index for the odd (right) pixel
        //
        // VRAM addressing: byte_column * 256 + row
        // Each byte_column spans 2 screen pixels, so screen pixel X maps to:
        //   byte_column = X / 2,  upper nibble if X is even, lower nibble if X is odd
        //
        // Visible area: 292 pixels wide (146 byte-columns) x 240 pixels tall,
        // cropped from the full 304x256 frame starting at byte-column 3, row 7.
        const CROP_X: usize = 6;  // First visible byte-column * 2 (pixel offset)
        const CROP_Y: usize = 7;  // First visible row

        for screen_y in 0..h {
            let row = screen_y + CROP_Y;
            for screen_x in 0..w {
                let pixel_x = screen_x + CROP_X;
                let byte_column = pixel_x / 2;
                let vram_addr = byte_column * 256 + row;

                let byte = if vram_addr < self.video_ram.len() {
                    self.video_ram[vram_addr]
                } else {
                    0
                };

                // Even pixel = upper nibble, odd pixel = lower nibble
                let color_index = if pixel_x & 1 == 0 {
                    (byte >> 4) & 0x0F
                } else {
                    byte & 0x0F
                };
                let (r, g, b) = palette_rgb[color_index as usize];
                let pixel_offset = (screen_y * w + screen_x) * 3;
                buffer[pixel_offset] = r;
                buffer[pixel_offset + 1] = g;
                buffer[pixel_offset + 2] = b;
            }
        }
    }

    fn set_input(&mut self, button: u8, pressed: bool) {
        match button {
            // Player buttons (IDs 0-7) map directly to bits 0-7 of Widget PIA
            // Port A. Williams uses active-low logic: clearing a bit means the
            // button is pressed, setting a bit means it is released.
            INPUT_P1_RIGHT..=INPUT_P2_START => {
                if pressed {
                    self.input_port_a &= !(1 << button); // Clear bit = pressed
                } else {
                    self.input_port_a |= 1 << button; // Set bit = released
                }
                self.widget_pia.set_port_a_input(self.input_port_a);
            }
            // Coin insertion triggers the CA1 control line on the Widget PIA.
            // On real hardware this is an active-low edge: the coin switch pulls
            // CA1 low momentarily. The PIA's edge-detect logic generates an
            // interrupt flag on the transition.
            INPUT_COIN => {
                if pressed {
                    self.widget_pia.set_ca1(false);
                } else {
                    self.widget_pia.set_ca1(true);
                }
            }
            _ => {}
        }
    }

    fn input_map(&self) -> &[InputButton] {
        JOUST_INPUT_MAP
    }

    fn reset(&mut self) {
        self.cpu.reset();
        let vec_hi = self.program_rom[0x2FFE];
        let vec_lo = self.program_rom[0x2FFF];
        self.cpu.pc = u16::from_be_bytes([vec_hi, vec_lo]);

        self.widget_pia = Pia6820::new();
        self.rom_pia = Pia6820::new();
        self.blitter = WilliamsBlitter::new();
        self.rom_bank = 0;
        self.watchdog_counter = 0;
        self.clock = 0;
        self.input_port_a = 0xFF;
        self.input_port_b = 0xFF;
        // CMOS RAM and video RAM NOT cleared (battery-backed / not cleared by hardware)
    }
}
