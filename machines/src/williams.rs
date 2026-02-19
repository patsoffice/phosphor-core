use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::m6800::M6800;
use phosphor_core::cpu::m6809::M6809;
use phosphor_core::cpu::state::{M6800State, M6809State};
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::cmos_ram::CmosRam;
use phosphor_core::device::dac::Mc1408Dac;
use phosphor_core::device::pia6820::Pia6820;
use phosphor_core::device::williams_blitter::WilliamsBlitter;

use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};

// ---------------------------------------------------------------------------
// Williams gen-1 hardware constants
// ---------------------------------------------------------------------------

/// CPU cycles per scanline (1 MHz CPU / ~15.6 kHz horizontal).
pub const CYCLES_PER_SCANLINE: u64 = 64;

/// CPU cycles per frame (260 scanlines × 64 cycles).
pub const CYCLES_PER_FRAME: u64 = 260 * CYCLES_PER_SCANLINE; // 16640

/// Native display width after cropping.
pub const DISPLAY_WIDTH: u32 = 292;

/// Native display height after cropping.
pub const DISPLAY_HEIGHT: u32 = 240;

// ---------------------------------------------------------------------------
// Shared ROM definitions (common to all Williams gen-1 games)
// ---------------------------------------------------------------------------

/// Decoder PROMs: 2 × 512B, identical across all gen-1 boards.
pub static WILLIAMS_DECODER_PROM: RomRegion = RomRegion {
    size: 0x0400,
    entries: &[
        RomEntry {
            name: "decoder_rom_4.3g",
            size: 0x0200,
            offset: 0x0000,
            crc32: &[0xe6631c23],
        },
        RomEntry {
            name: "decoder_rom_6.3c",
            size: 0x0200,
            offset: 0x0200,
            crc32: &[0x83faf25e],
        },
    ],
};

/// SC-1 sound board ROM: 4KB, shared by Joust, Robotron, Bubbles, etc.
pub static WILLIAMS_SOUND_ROM: RomRegion = RomRegion {
    size: 0x1000,
    entries: &[RomEntry {
        name: "video_sound_rom_4_std_780.ic12",
        size: 0x1000,
        offset: 0x0000,
        crc32: &[0xf1835bdd],
    }],
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Active-high bit manipulation: set bit on press, clear on release.
pub(crate) fn set_bit(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg |= 1 << bit;
    } else {
        *reg &= !(1 << bit);
    }
}

// ---------------------------------------------------------------------------
// WilliamsBoard
// ---------------------------------------------------------------------------

/// Williams gen-1 arcade board hardware.
///
/// Contains all shared hardware: M6809E main CPU @ 1 MHz, M6800 sound CPU,
/// 48KB video RAM, two MC6821 PIAs, Williams SC1 blitter, 1KB battery-backed
/// CMOS RAM, 12KB program ROM, sound board with DAC.
///
/// Game-specific machines (Joust, Robotron, etc.) compose this struct and
/// provide their own ROM definitions and input wiring.
pub struct WilliamsBoard {
    // CPUs
    pub(crate) cpu: M6809,
    pub(crate) sound_cpu: M6800,

    // Memory regions
    pub(crate) video_ram: [u8; 0xC000], // 0x0000-0xBFFF: 48KB video/color RAM
    pub(crate) banked_rom: [u8; 0x9000], // 0x0000-0x8FFF: 36KB banked ROM overlay
    pub(crate) palette_ram: [u8; 16],   // 0xC000-0xC00F: 16-color palette
    pub(crate) cmos_ram: CmosRam,       // 0xCC00-0xCFFF: 1KB battery-backed
    pub(crate) program_rom: [u8; 0x3000], // 0xD000-0xFFFF: 12KB program ROM

    // Peripheral devices
    pub(crate) widget_pia: Pia6820, // 0xC804-0xC807: player inputs
    pub(crate) rom_pia: Pia6820,    // 0xC80C-0xC80F: ROM bank, video timing
    pub(crate) blitter: WilliamsBlitter, // 0xCA00-0xCA07: DMA blitter

    // I/O registers
    pub(crate) rom_bank: u8, // 0xC900: ROM bank select

    // Sound board (M6808 stand-in for M6802)
    pub(crate) sound_ram: [u8; 256], // 0x0000-0x00FF: 256 bytes RAM
    pub(crate) sound_pia: Pia6820,   // 0x0400-0x0403: Sound PIA
    pub(crate) sound_rom: [u8; 0x1000], // 0xF000-0xFFFF: 4KB sound ROM

    // Audio output
    pub(crate) dac: Mc1408Dac,
    pub(crate) audio_buffer: Vec<i16>,
    pub(crate) sample_accum: i64,
    pub(crate) sample_count: u32,
    pub(crate) sample_phase: u64,

    // System state
    pub watchdog_counter: u32,
    pub(crate) clock: u64,

    // ROM PIA Port A input (game sets coin/service bits)
    pub(crate) rom_pia_input: u8,

    // Scanline-rendered framebuffer (292 × 240 × RGB24)
    pub(crate) scanline_buffer: Vec<u8>,
}

impl WilliamsBoard {
    pub fn new() -> Self {
        Self {
            cpu: M6809::new(),
            sound_cpu: M6800::new(),
            video_ram: [0; 0xC000],
            banked_rom: [0; 0x9000],
            palette_ram: [0; 16],
            cmos_ram: CmosRam::new(),
            program_rom: [0; 0x3000],
            widget_pia: Pia6820::new(),
            rom_pia: Pia6820::new(),
            blitter: WilliamsBlitter::new(),
            rom_bank: 0,
            sound_ram: [0; 256],
            sound_pia: Pia6820::new(),
            sound_rom: [0; 0x1000],
            dac: Mc1408Dac::new(),
            audio_buffer: Vec::with_capacity(1024),
            sample_accum: 0,
            sample_count: 0,
            sample_phase: 0,
            watchdog_counter: 0,
            clock: 0,
            rom_pia_input: 0,
            scanline_buffer: vec![0u8; DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize * 3],
        }
    }

    // --- Accessors ---

    pub fn get_cpu_state(&self) -> M6809State {
        self.cpu.snapshot()
    }

    pub fn get_sound_cpu_state(&self) -> M6800State {
        self.sound_cpu.snapshot()
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

    // --- ROM loading ---

    /// Load program ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the ROM region (0 = address 0xD000).
    pub fn load_program_rom(&mut self, offset: usize, data: &[u8]) {
        let end = (offset + data.len()).min(self.program_rom.len());
        let len = end - offset;
        self.program_rom[offset..end].copy_from_slice(&data[..len]);
    }

    /// Load banked ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the banked ROM region (0 = address 0x0000).
    pub fn load_banked_rom(&mut self, offset: usize, data: &[u8]) {
        let end = (offset + data.len()).min(self.banked_rom.len());
        let len = end - offset;
        self.banked_rom[offset..end].copy_from_slice(&data[..len]);
    }

    /// Load sound ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the sound ROM region (0 = address 0xF000).
    pub fn load_sound_rom(&mut self, offset: usize, data: &[u8]) {
        let end = (offset + data.len()).min(self.sound_rom.len());
        let len = end - offset;
        self.sound_rom[offset..end].copy_from_slice(&data[..len]);
    }

    /// Load ROMs from a RomSet using game-specific region definitions.
    pub fn load_rom_regions(
        &mut self,
        rom_set: &RomSet,
        banked_region: &RomRegion,
        program_region: &RomRegion,
        sound_region: &RomRegion,
    ) -> Result<(), RomLoadError> {
        let banked_data = banked_region.load(rom_set)?;
        self.banked_rom.copy_from_slice(&banked_data);

        let rom_data = program_region.load(rom_set)?;
        self.program_rom.copy_from_slice(&rom_data);

        let sound_data = sound_region.load(rom_set)?;
        self.sound_rom.copy_from_slice(&sound_data);

        Ok(())
    }

    // --- Internal timing/rendering ---

    /// Current scanline number derived from the master clock.
    fn current_scanline(&self) -> u8 {
        let frame_cycle = self.clock % CYCLES_PER_FRAME;
        (frame_cycle / CYCLES_PER_SCANLINE) as u8
    }

    /// Render a single scanline from VRAM + palette into the internal scanline buffer.
    /// `scanline` is the raw scanline number (0-259); only call for visible lines (7-246).
    fn render_scanline(&mut self, scanline: usize) {
        const CROP_X: usize = 6;
        const CROP_Y: usize = 7;
        const WIDTH: usize = 292;
        const RG_LUT: [u8; 8] = [0, 38, 81, 118, 137, 174, 217, 255];
        const B_LUT: [u8; 4] = [0, 95, 160, 255];

        // Decode the current palette (16 entries, BBGGGRRR)
        let mut palette_rgb = [(0u8, 0u8, 0u8); 16];
        for (i, rgb) in palette_rgb.iter_mut().enumerate() {
            let entry = self.palette_ram[i];
            *rgb = (
                RG_LUT[(entry & 0x07) as usize],
                RG_LUT[((entry >> 3) & 0x07) as usize],
                B_LUT[((entry >> 6) & 0x03) as usize],
            );
        }

        let screen_y = scanline - CROP_Y;
        let row_offset = screen_y * WIDTH * 3;

        for screen_x in 0..WIDTH {
            let pixel_x = screen_x + CROP_X;
            let byte_column = pixel_x / 2;
            let vram_addr = byte_column * 256 + scanline;

            let byte = if vram_addr < self.video_ram.len() {
                self.video_ram[vram_addr]
            } else {
                0
            };

            let color_index = if pixel_x & 1 == 0 {
                (byte >> 4) & 0x0F
            } else {
                byte & 0x0F
            };

            let (r, g, b) = palette_rgb[color_index as usize];
            let pixel_offset = row_offset + screen_x * 3;
            self.scanline_buffer[pixel_offset] = r;
            self.scanline_buffer[pixel_offset + 1] = g;
            self.scanline_buffer[pixel_offset + 2] = b;
        }
    }

    // --- Core tick ---

    pub fn tick(&mut self) {
        // Video timing signals on ROM PIA.
        // VA11 (scanline bit 5) → ROM PIA CB1, count240 → ROM PIA CA1.
        // These drive the main CPU's IRQ via ROM PIA interrupt outputs.
        let frame_cycle = self.clock % CYCLES_PER_FRAME;
        if frame_cycle.is_multiple_of(CYCLES_PER_SCANLINE) {
            let scanline = (frame_cycle / CYCLES_PER_SCANLINE) as u16;

            // Render this scanline from current VRAM + palette before the CPU
            // processes it, matching hardware CRT read timing.
            if (7..=246).contains(&scanline) {
                self.render_scanline(scanline as usize);
            }

            if scanline != 256 {
                // VA11: toggles every 32 scanlines
                self.rom_pia.set_cb1((scanline & 0x20) != 0);
            }
            // count240: asserted from scanline 240 through VBLANK
            self.rom_pia.set_ca1(scanline >= 240);
        }

        // Propagate sound commands from main board ROM PIA to sound board PIA.
        // High two bits are externally pulled high on real hardware.
        // CB1 is held low for 0xFF (silence sentinel), asserted high otherwise to
        // generate an IRQ on the sound CPU.
        if self.rom_pia.take_port_b_written() {
            let command = self.rom_pia.read_output_b() | 0xC0;
            self.sound_pia.set_port_b_input(command);
            self.sound_pia.set_cb1(command != 0xFF);
        }

        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            if self.blitter.is_active() {
                self.blitter.do_dma_cycle(bus);
            } else {
                self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
            }
            // Sound CPU runs every cycle (separate bus, not halted by blitter)
            self.sound_cpu.execute_cycle(bus, BusMaster::Cpu(1));
        }

        // DAC is continuously connected to sound PIA Port A output pins
        let dac_byte = self.sound_pia.read_output_a();
        self.dac.write(dac_byte);

        // Bresenham downsample: 1 MHz CPU clock -> 44.1 kHz output
        const CPU_CLOCK_HZ: u64 = 1_000_000;
        const OUTPUT_SAMPLE_RATE: u64 = 44_100;

        self.sample_accum += self.dac.sample_i16() as i64;
        self.sample_count += 1;
        self.sample_phase += OUTPUT_SAMPLE_RATE;

        if self.sample_phase >= CPU_CLOCK_HZ {
            self.sample_phase -= CPU_CLOCK_HZ;
            let sample = (self.sample_accum / self.sample_count as i64) as i16;
            self.audio_buffer.push(sample);
            self.sample_accum = 0;
            self.sample_count = 0;
        }

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    // --- Reset ---

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.sound_cpu.reset();

        // Reset peripherals first so bus is in a known state
        self.widget_pia = Pia6820::new();
        self.rom_pia = Pia6820::new();
        self.sound_pia = Pia6820::new();
        self.blitter = WilliamsBlitter::new();
        self.rom_bank = 0;
        self.dac = Mc1408Dac::new();
        self.audio_buffer.clear();
        self.sample_accum = 0;
        self.sample_count = 0;
        self.sample_phase = 0;
        self.watchdog_counter = 0;
        self.clock = 0;
        self.rom_pia_input = 0;
        self.scanline_buffer.fill(0);
        // CMOS RAM and video RAM NOT cleared (battery-backed / not cleared by hardware)

        // Fetch reset vectors through the bus (matches hardware behavior)
        let bus_ptr: *mut Self = self;
        unsafe {
            let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
            let main_hi = bus.read(BusMaster::Cpu(0), 0xFFFE);
            let main_lo = bus.read(BusMaster::Cpu(0), 0xFFFF);
            self.cpu.pc = u16::from_be_bytes([main_hi, main_lo]);

            let snd_hi = bus.read(BusMaster::Cpu(1), 0xFFFE);
            let snd_lo = bus.read(BusMaster::Cpu(1), 0xFFFF);
            self.sound_cpu.pc = u16::from_be_bytes([snd_hi, snd_lo]);
        }
    }

    // --- Machine trait helpers (called by game wrappers) ---

    /// Run one frame's worth of cycles. Game wrappers that need per-tick
    /// hooks (e.g. Joust's LS157 mux) should use their own tick loop instead.
    pub fn run_frame(&mut self) {
        self.rom_pia.set_port_a_input(self.rom_pia_input);
        for _ in 0..CYCLES_PER_FRAME {
            self.tick();
        }
    }

    pub fn render_frame(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.scanline_buffer);
    }

    pub fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
        let n = buffer.len().min(self.audio_buffer.len());
        buffer[..n].copy_from_slice(&self.audio_buffer[..n]);
        self.audio_buffer.drain(..n);
        n
    }
}

impl Default for WilliamsBoard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bus implementation — Williams gen-1 memory map
// ---------------------------------------------------------------------------

impl Bus for WilliamsBoard {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, master: BusMaster, addr: u16) -> u8 {
        if master == BusMaster::Cpu(1) {
            // Sound board memory map
            return match addr {
                0x0000..=0x00FF => self.sound_ram[addr as usize],
                0x0400..=0x0403 => self.sound_pia.read((addr - 0x0400) as u8),
                // 4KB ROM mirrored via incomplete address decoding
                0xB000..=0xFFFF => self.sound_rom[(addr & 0x0FFF) as usize],
                _ => 0xFF,
            };
        }

        // Main board memory map
        match addr {
            0x0000..=0x8FFF => {
                // DmaVram reads bypass ROM banking — the blitter reads dest
                // directly from VRAM for keepmask blending.
                if master != BusMaster::DmaVram && self.rom_bank != 0 {
                    self.banked_rom[addr as usize]
                } else {
                    self.video_ram[addr as usize]
                }
            }
            0x9000..=0xBFFF => self.video_ram[addr as usize],
            0xC000..=0xC00F => self.palette_ram[(addr - 0xC000) as usize],
            0xC804..=0xC807 => self.widget_pia.read((addr - 0xC804) as u8),
            0xC80C..=0xC80F => self.rom_pia.read((addr - 0xC80C) as u8),
            0xC900 => self.rom_bank,
            0xCA00..=0xCA07 => 0, // Blitter registers are write-only on real hardware
            0xCB00..=0xCBFF => self.current_scanline() & 0xFC, // Video counter read
            0xCC00..=0xCFFF => self.cmos_ram.read(addr - 0xCC00),
            0xD000..=0xFFFF => self.program_rom[(addr - 0xD000) as usize],
            _ => 0xFF,
        }
    }

    fn write(&mut self, master: BusMaster, addr: u16, data: u8) {
        if master == BusMaster::Cpu(1) {
            // Sound board memory map
            return match addr {
                0x0000..=0x00FF => self.sound_ram[addr as usize] = data,
                0x0400..=0x0403 => self.sound_pia.write((addr - 0x0400) as u8, data),
                _ => { /* ROM or unmapped: ignored */ }
            };
        }

        // Main board memory map
        match addr {
            0x0000..=0xBFFF => self.video_ram[addr as usize] = data,
            0xC000..=0xC00F => self.palette_ram[(addr - 0xC000) as usize] = data,
            0xC804..=0xC807 => self.widget_pia.write((addr - 0xC804) as u8, data),
            0xC80C..=0xC80F => self.rom_pia.write((addr - 0xC80C) as u8, data),
            0xC900 => self.rom_bank = data,
            0xCA00..=0xCA07 => self.blitter.write_register((addr - 0xCA00) as u8, data),
            0xCBFF => {
                if data == 0x39 {
                    self.watchdog_counter = 0;
                }
            }
            // Only lower 4 bits valid on Williams 5114/6514 SRAM
            0xCC00..=0xCFFF => self.cmos_ram.write(addr - 0xCC00, data | 0xF0),
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
            // Only ROM PIA interrupts are wired to the main CPU IRQ line
            // via INPUT_MERGER_ANY_HIGH. Widget PIA IRQs are not connected.
            // FIRQ is not used on Williams gen-1 hardware.
            BusMaster::Cpu(0) => InterruptState {
                nmi: false,
                irq: self.rom_pia.irq_a() || self.rom_pia.irq_b(),
                firq: false,
                ..Default::default()
            },
            BusMaster::Cpu(1) => InterruptState {
                nmi: false,
                irq: self.sound_pia.irq_a() || self.sound_pia.irq_b(),
                firq: false,
                ..Default::default()
            },
            _ => InterruptState::default(),
        }
    }
}
