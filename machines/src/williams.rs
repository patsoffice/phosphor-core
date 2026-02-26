use phosphor_core::audio::AudioResampler;
use phosphor_core::bus_split;
use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::memory_map::{AccessKind, MemoryMap};
use phosphor_core::core::save_state::{SaveError, Saveable, StateReader, StateWriter};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::m6800::M6800;
use phosphor_core::cpu::m6809::M6809;
use phosphor_core::cpu::state::{M6800State, M6809State};
use phosphor_core::cpu::{Cpu, CpuStateTrait};
use phosphor_core::device::dac::Mc1408Dac;
use phosphor_core::device::pia6820::Pia6820;
use phosphor_core::device::williams_blitter::WilliamsBlitter;
use phosphor_macros::BusDebug;

use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};

// ---------------------------------------------------------------------------
// Memory map region IDs (machine-specific constants for page table dispatch)
// ---------------------------------------------------------------------------

/// Main CPU (M6809) address space region IDs.
pub(crate) mod main_region {
    pub const VIDEO_RAM: u8 = 1; // 0x0000-0xBFFF (48KB, banked ROM overlay at 0x0000-0x8FFF)
    pub const PALETTE: u8 = 2; // 0xC000-0xC00F (16-color palette)
    pub const IO_PIA: u8 = 3; // 0xC800-0xC8FF (Widget PIA + ROM PIA)
    pub const IO_BANK: u8 = 4; // 0xC900-0xC9FF (ROM bank select register)
    pub const IO_BLITTER: u8 = 5; // 0xCA00-0xCAFF (SC1 blitter registers)
    pub const IO_VIDEO: u8 = 6; // 0xCB00-0xCBFF (video counter + watchdog)
    pub const CMOS: u8 = 7; // 0xCC00-0xCFFF (1KB battery-backed CMOS)
    pub const PROGRAM_ROM: u8 = 8; // 0xD000-0xFFFF (12KB program ROM)
    pub const BANKED_ROM: u8 = 9; // (36KB, overlays VIDEO_RAM when bank != 0)
}

/// Sound CPU (M6800) address space region IDs.
pub(crate) mod sound_region {
    pub const RAM: u8 = 1; // 0x0000-0x00FF (256 bytes)
    pub const IO_PIA: u8 = 2; // 0x0400-0x04FF (Sound PIA)
    pub const ROM: u8 = 3; // 0xB000-0xFFFF (4KB mirrored)
}

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
// Shared macros for Williams gen-1 game wrappers
// ---------------------------------------------------------------------------

/// Implements `Renderable` methods for Williams gen-1 games: display_size, render_frame.
///
/// The implementing type must have a `board: WilliamsBoard` field.
macro_rules! impl_williams_renderable {
    () => {
        fn display_size(&self) -> (u32, u32) {
            (
                crate::williams::DISPLAY_WIDTH,
                crate::williams::DISPLAY_HEIGHT,
            )
        }

        fn render_frame(&self, buffer: &mut [u8]) {
            self.board.render_frame(buffer);
        }
    };
}

/// Implements `AudioSource` methods for Williams gen-1 games: fill_audio, audio_sample_rate.
///
/// The implementing type must have a `board: WilliamsBoard` field.
macro_rules! impl_williams_audio {
    () => {
        fn fill_audio(&mut self, buffer: &mut [i16]) -> usize {
            self.board.fill_audio(buffer)
        }

        fn audio_sample_rate(&self) -> u32 {
            44100
        }
    };
}

/// Implements `MachineDebug` methods for Williams gen-1 games:
/// debug_bus, debug_bus_mut, cycles_per_frame.
///
/// The implementing type must have a `board: WilliamsBoard` field.
/// Note: `debug_tick()` is game-specific and must be provided separately.
macro_rules! impl_williams_debug {
    () => {
        fn debug_bus(&self) -> Option<&dyn phosphor_core::core::debug::BusDebug> {
            Some(&self.board)
        }

        fn debug_bus_mut(&mut self) -> Option<&mut dyn phosphor_core::core::debug::BusDebug> {
            Some(&mut self.board)
        }

        fn cycles_per_frame(&self) -> u64 {
            crate::williams::CYCLES_PER_FRAME
        }
    };
}

/// Implements remaining `Machine` methods shared across Williams gen-1 games:
/// save_nvram, load_nvram, frame_rate_hz.
///
/// The implementing type must have a `board: WilliamsBoard` field.
macro_rules! impl_williams_machine_common {
    () => {
        fn save_nvram(&self) -> Option<&[u8]> {
            Some(self.board.save_cmos())
        }

        fn load_nvram(&mut self, data: &[u8]) {
            self.board.load_cmos(data);
        }

        fn frame_rate_hz(&self) -> f64 {
            1_000_000.0 / crate::williams::CYCLES_PER_FRAME as f64
        }
    };
}

/// Implements the 3 Bus methods that are identical across all Williams
/// gen-1 games: write, is_halted_for, check_interrupts.
///
/// Bus::read is NOT included because some games (Joust) have game-specific
/// hooks before delegating to the board.
macro_rules! impl_williams_bus_common {
    () => {
        fn write(&mut self, master: BusMaster, addr: u16, data: u8) {
            self.board.write(master, addr, data);
        }

        fn is_halted_for(&self, master: BusMaster) -> bool {
            self.board.is_halted_for(master)
        }

        fn check_interrupts(
            &mut self,
            target: BusMaster,
        ) -> phosphor_core::core::bus::InterruptState {
            self.board.check_interrupts(target)
        }
    };
}

pub(crate) use impl_williams_audio;
pub(crate) use impl_williams_bus_common;
pub(crate) use impl_williams_debug;
pub(crate) use impl_williams_machine_common;
pub(crate) use impl_williams_renderable;

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
#[derive(BusDebug)]
pub struct WilliamsBoard {
    // CPUs (debug reads/writes auto-routed through matching #[debug_map])
    #[debug_cpu("M6809 Main")]
    pub(crate) cpu: M6809,
    #[debug_cpu("M6800 Sound")]
    pub(crate) sound_cpu: M6800,

    // Peripheral devices
    #[debug_device("Widget PIA")]
    pub(crate) widget_pia: Pia6820, // 0xC804-0xC807: player inputs
    #[debug_device("ROM PIA")]
    pub(crate) rom_pia: Pia6820, // 0xC80C-0xC80F: ROM bank, video timing
    #[debug_device("Blitter")]
    pub(crate) blitter: WilliamsBlitter, // 0xCA00-0xCA07: DMA blitter

    // I/O registers
    pub(crate) rom_bank: u8, // 0xC900: ROM bank select

    // Sound board
    #[debug_device("Sound PIA")]
    pub(crate) sound_pia: Pia6820, // 0x0400-0x0403: Sound PIA

    // Audio output
    #[debug_device("DAC")]
    pub(crate) dac: Mc1408Dac,
    pub(crate) resampler: AudioResampler,

    // Memory maps (page-table dispatch + watchpoints + backing memory)
    // All RAM/ROM storage lives in the MemoryMap backing store.
    #[debug_map(cpu = 0)]
    pub(crate) main_map: MemoryMap,
    #[debug_map(cpu = 1)]
    pub(crate) sound_map: MemoryMap,

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
            widget_pia: Pia6820::new(),
            rom_pia: Pia6820::new(),
            blitter: WilliamsBlitter::new(),
            rom_bank: 0,
            sound_pia: Pia6820::new(),
            dac: Mc1408Dac::new(),
            resampler: AudioResampler::new(1_000_000, 44_100),
            main_map: Self::build_main_map(),
            sound_map: Self::build_sound_map(),
            watchdog_counter: 0,
            clock: 0,
            rom_pia_input: 0,
            scanline_buffer: vec![0u8; DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize * 3],
        }
    }

    fn build_main_map() -> MemoryMap {
        use main_region::*;
        let mut map = MemoryMap::new();
        map.region(
            VIDEO_RAM,
            "Video RAM",
            0x0000,
            0xC000,
            AccessKind::ReadWrite,
        )
        .region(PALETTE, "Palette", 0xC000, 0x100, AccessKind::ReadWrite)
        .region(IO_PIA, "PIAs", 0xC800, 0x100, AccessKind::Io)
        .region(IO_BANK, "ROM Bank", 0xC900, 0x100, AccessKind::Io)
        .region(IO_BLITTER, "Blitter", 0xCA00, 0x100, AccessKind::Io)
        .region(IO_VIDEO, "Video Counter", 0xCB00, 0x100, AccessKind::Io)
        .region(CMOS, "CMOS RAM", 0xCC00, 0x400, AccessKind::ReadWrite)
        .region(
            PROGRAM_ROM,
            "Program ROM",
            0xD000,
            0x3000,
            AccessKind::ReadOnly,
        )
        .backing_region(BANKED_ROM, "Banked ROM", 0x9000);
        map
    }

    fn build_sound_map() -> MemoryMap {
        use sound_region::*;
        let mut map = MemoryMap::new();
        map.region(RAM, "Sound RAM", 0x0000, 0x100, AccessKind::ReadWrite)
            .region(IO_PIA, "Sound PIA", 0x0400, 0x100, AccessKind::Io)
            .region(ROM, "Sound ROM", 0xF000, 0x1000, AccessKind::ReadOnly)
            .mirror(0xB000, 0xF000, 0x1000)
            .mirror(0xC000, 0xF000, 0x1000)
            .mirror(0xD000, 0xF000, 0x1000)
            .mirror(0xE000, 0xF000, 0x1000);
        map
    }

    // --- Accessors ---

    pub fn get_cpu_state(&self) -> M6809State {
        self.cpu.snapshot()
    }

    pub fn get_sound_cpu_state(&self) -> M6800State {
        self.sound_cpu.snapshot()
    }

    pub fn read_video_ram(&self, addr: usize) -> u8 {
        let vram = self.main_map.region_data(main_region::VIDEO_RAM);
        if addr < vram.len() { vram[addr] } else { 0 }
    }

    pub fn write_video_ram(&mut self, addr: usize, data: u8) {
        let vram = self.main_map.region_data_mut(main_region::VIDEO_RAM);
        if addr < vram.len() {
            vram[addr] = data;
        }
    }

    pub fn read_palette(&self, index: usize) -> u8 {
        if index < 16 {
            self.main_map.region_data(main_region::PALETTE)[index]
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
        let cmos = self.main_map.region_data_mut(main_region::CMOS);
        let len = data.len().min(cmos.len());
        cmos[..len].copy_from_slice(&data[..len]);
    }

    pub fn save_cmos(&self) -> &[u8] {
        self.main_map.region_data(main_region::CMOS)
    }

    // --- ROM loading ---

    /// Load program ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the ROM region (0 = address 0xD000).
    pub fn load_program_rom(&mut self, offset: usize, data: &[u8]) {
        self.main_map
            .load_region_at(main_region::PROGRAM_ROM, offset, data);
    }

    /// Load banked ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the banked ROM region (0 = address 0x0000).
    pub fn load_banked_rom(&mut self, offset: usize, data: &[u8]) {
        self.main_map
            .load_region_at(main_region::BANKED_ROM, offset, data);
    }

    /// Load sound ROM from a byte slice at the given offset.
    /// Offset is relative to the start of the sound ROM region (0 = address 0xF000).
    pub fn load_sound_rom(&mut self, offset: usize, data: &[u8]) {
        self.sound_map
            .load_region_at(sound_region::ROM, offset, data);
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
        self.main_map
            .load_region(main_region::BANKED_ROM, &banked_data);

        let rom_data = program_region.load(rom_set)?;
        self.main_map
            .load_region(main_region::PROGRAM_ROM, &rom_data);

        let sound_data = sound_region.load(rom_set)?;
        self.sound_map.load_region(sound_region::ROM, &sound_data);

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

        let palette = self.main_map.region_data(main_region::PALETTE);
        let vram = self.main_map.region_data(main_region::VIDEO_RAM);

        // Decode the current palette (16 entries, BBGGGRRR)
        let mut palette_rgb = [(0u8, 0u8, 0u8); 16];
        for (i, rgb) in palette_rgb.iter_mut().enumerate() {
            let entry = palette[i];
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

            let byte = if vram_addr < vram.len() {
                vram[vram_addr]
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

        bus_split!(self, bus => {
            if self.blitter.is_active() {
                self.blitter.do_dma_cycle(bus);
            } else {
                self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
            }
            // Sound CPU runs every cycle (separate bus, not halted by blitter)
            self.sound_cpu.execute_cycle(bus, BusMaster::Cpu(1));
        });

        // DAC is continuously connected to sound PIA Port A output pins
        let dac_byte = self.sound_pia.read_output_a();
        self.dac.write(dac_byte);

        // Bresenham downsample: 1 MHz CPU clock -> 44.1 kHz output
        self.resampler.tick(self.dac.sample_i16());

        self.clock += 1;
        self.watchdog_counter += 1;
    }

    // --- Reset ---

    pub fn reset(&mut self) {
        // Reset peripherals first so bus is in a known state
        self.widget_pia.reset();
        self.rom_pia.reset();
        self.sound_pia.reset();
        self.blitter.reset();
        self.rom_bank = 0;
        // Ensure pages 0x00-0x8F point to VIDEO_RAM (undo any bank switch)
        self.main_map
            .remap_pages(0x00, 0x90, main_region::VIDEO_RAM, 0);
        self.dac.reset();
        self.resampler.reset();
        self.watchdog_counter = 0;
        self.clock = 0;
        self.rom_pia_input = 0;
        self.scanline_buffer.fill(0);
        // CMOS RAM and video RAM NOT cleared (battery-backed / not cleared by hardware)

        // CPU reset fetches the reset vector from the bus (matching real hardware)
        bus_split!(self, bus => {
            self.cpu.reset(bus, BusMaster::Cpu(0));
            self.sound_cpu.reset(bus, BusMaster::Cpu(1));
        });
    }

    // --- Debug helpers ---

    /// Returns a bitmask of CPUs at instruction boundaries.
    /// Bit 0 = main CPU (M6809), bit 1 = sound CPU (M6800).
    pub fn debug_tick_boundaries(&self) -> u32 {
        let mut result = 0;
        if self.cpu.at_instruction_boundary() {
            result |= 1;
        }
        if self.sound_cpu.at_instruction_boundary() {
            result |= 2;
        }
        result
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
        self.resampler.fill_audio(buffer)
    }

    // --- Save state helpers (called by game wrappers) ---

    pub(crate) fn save_board_state(&self, w: &mut StateWriter) {
        // CPUs
        self.cpu.save_state(w);
        self.sound_cpu.save_state(w);
        // RAM (byte counts must match load_board_state for save state compat)
        w.write_bytes(self.main_map.region_data(main_region::VIDEO_RAM)); // 0xC000
        w.write_bytes(&self.main_map.region_data(main_region::PALETTE)[..16]); // 16
        w.write_bytes(self.main_map.region_data(main_region::CMOS)); // 1024
        w.write_bytes(self.sound_map.region_data(sound_region::RAM)); // 256
        // Peripherals
        self.widget_pia.save_state(w);
        self.rom_pia.save_state(w);
        self.sound_pia.save_state(w);
        self.blitter.save_state(w);
        self.dac.save_state(w);
        // I/O & timing
        w.write_u8(self.rom_bank);
        self.resampler.save_state(w);
        w.write_u32_le(self.watchdog_counter);
        w.write_u64_le(self.clock);
        w.write_u8(self.rom_pia_input);
    }

    pub(crate) fn load_board_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        // CPUs
        self.cpu.load_state(r)?;
        self.sound_cpu.load_state(r)?;
        // RAM
        r.read_bytes_into(self.main_map.region_data_mut(main_region::VIDEO_RAM))?;
        r.read_bytes_into(&mut self.main_map.region_data_mut(main_region::PALETTE)[..16])?;
        r.read_bytes_into(self.main_map.region_data_mut(main_region::CMOS))?;
        r.read_bytes_into(self.sound_map.region_data_mut(sound_region::RAM))?;
        // Peripherals
        self.widget_pia.load_state(r)?;
        self.rom_pia.load_state(r)?;
        self.sound_pia.load_state(r)?;
        self.blitter.load_state(r)?;
        self.dac.load_state(r)?;
        // I/O & timing
        self.rom_bank = r.read_u8()?;
        self.resampler.load_state(r)?;
        self.watchdog_counter = r.read_u32_le()?;
        self.clock = r.read_u64_le()?;
        self.rom_pia_input = r.read_u8()?;
        Ok(())
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
            // Sound board
            let data = match self.sound_map.page(addr).region_id {
                sound_region::IO_PIA => {
                    if (0x0400..=0x0403).contains(&addr) {
                        self.sound_pia.read((addr - 0x0400) as u8)
                    } else {
                        0xFF
                    }
                }
                sound_region::RAM | sound_region::ROM => self.sound_map.read_backing(addr),
                _ => 0xFF,
            };
            self.sound_map.check_read_watch(addr, data);
            return data;
        }

        // DmaVram reads bypass ROM banking — the blitter reads dest
        // directly from VRAM for keepmask blending.
        if master == BusMaster::DmaVram && addr <= 0x8FFF {
            return self.main_map.region_data(main_region::VIDEO_RAM)[addr as usize];
        }

        // Main board — backed regions use page-table dispatch (banking
        // is handled by remap_pages, so read_backing follows automatically)
        let data = match self.main_map.page(addr).region_id {
            main_region::PALETTE => {
                if addr <= 0xC00F {
                    self.main_map.region_data(main_region::PALETTE)[(addr & 0x0F) as usize]
                } else {
                    0xFF
                }
            }
            main_region::IO_PIA => match addr {
                0xC804..=0xC807 => self.widget_pia.read((addr - 0xC804) as u8),
                0xC80C..=0xC80F => self.rom_pia.read((addr - 0xC80C) as u8),
                _ => 0xFF,
            },
            main_region::IO_BANK => self.rom_bank,
            main_region::IO_BLITTER => 0, // write-only on real hardware
            main_region::IO_VIDEO => self.current_scanline() & 0xFC,
            main_region::VIDEO_RAM
            | main_region::BANKED_ROM
            | main_region::CMOS
            | main_region::PROGRAM_ROM => self.main_map.read_backing(addr),
            _ => 0xFF,
        };
        self.main_map.check_read_watch(addr, data);
        data
    }

    fn write(&mut self, master: BusMaster, addr: u16, data: u8) {
        if master == BusMaster::Cpu(1) {
            // Sound board
            match self.sound_map.page(addr).region_id {
                sound_region::RAM => self.sound_map.write_backing(addr, data),
                sound_region::IO_PIA => {
                    if (0x0400..=0x0403).contains(&addr) {
                        self.sound_pia.write((addr - 0x0400) as u8, data);
                    }
                }
                _ => {} // ROM or unmapped: ignored
            }
            self.sound_map.check_write_watch(addr, data);
            return;
        }

        // Main board
        match self.main_map.page(addr).region_id {
            // Writes always go to video RAM, even when banked ROM is overlaid
            main_region::VIDEO_RAM | main_region::BANKED_ROM => {
                self.main_map.region_data_mut(main_region::VIDEO_RAM)[addr as usize] = data;
            }
            main_region::PALETTE => {
                if addr <= 0xC00F {
                    self.main_map.region_data_mut(main_region::PALETTE)[(addr & 0x0F) as usize] =
                        data;
                }
            }
            main_region::IO_PIA => match addr {
                0xC804..=0xC807 => self.widget_pia.write((addr - 0xC804) as u8, data),
                0xC80C..=0xC80F => self.rom_pia.write((addr - 0xC80C) as u8, data),
                _ => {}
            },
            main_region::IO_BANK => {
                self.rom_bank = data;
                // Bank switching: remap pages 0x00-0x8F
                if data != 0 {
                    self.main_map
                        .remap_pages(0x00, 0x90, main_region::BANKED_ROM, 0);
                } else {
                    self.main_map
                        .remap_pages(0x00, 0x90, main_region::VIDEO_RAM, 0);
                }
            }
            main_region::IO_BLITTER => {
                if (0xCA00..=0xCA07).contains(&addr) {
                    self.blitter.write_register((addr - 0xCA00) as u8, data);
                }
            }
            main_region::IO_VIDEO => {
                if addr == 0xCBFF && data == 0x39 {
                    self.watchdog_counter = 0;
                }
            }
            // Only lower 4 bits valid on Williams 5114/6514 SRAM
            main_region::CMOS => self.main_map.write_backing(addr, data | 0xF0),
            _ => {} // ROM or unmapped: ignored
        }
        self.main_map.check_write_watch(addr, data);
    }

    fn is_halted_for(&self, master: BusMaster) -> bool {
        match master {
            BusMaster::Cpu(0) => self.blitter.is_active(),
            _ => false,
        }
    }

    fn check_interrupts(&mut self, target: BusMaster) -> InterruptState {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::cpu::CpuStateTrait;

    #[test]
    fn board_save_load_round_trip() {
        let mut board = WilliamsBoard::new();

        // Set known state across various subsystems
        board.write_video_ram(0, 0xAA);
        board.write_video_ram(0x5FFF, 0xBB);
        board.main_map.region_data_mut(main_region::PALETTE)[3] = 0x42;
        board.sound_map.region_data_mut(sound_region::RAM)[0x10] = 0xCD;
        board.rom_bank = 5;
        board.clock = 123_456;
        board.watchdog_counter = 789;
        board.rom_pia_input = 0x10;
        // Run a few ticks to accumulate some resampler state
        for _ in 0..100 {
            board.dac.write(0xA0);
            board.resampler.tick(board.dac.sample_i16());
        }

        // Write CMOS data
        board.main_map.region_data_mut(main_region::CMOS)[0] = 0xF1;
        board.main_map.region_data_mut(main_region::CMOS)[100] = 0xF9;

        // Save
        let mut w = StateWriter::new();
        board.save_board_state(&mut w);
        let data = w.into_vec();

        // Mutate everything
        let mut board2 = WilliamsBoard::new();
        board2.write_video_ram(0, 0xFF);
        board2.write_video_ram(0x5FFF, 0xFF);
        board2.main_map.region_data_mut(main_region::PALETTE)[3] = 0x00;
        board2.rom_bank = 0;
        board2.clock = 0;
        board2.watchdog_counter = 0;

        // Load
        let mut r = StateReader::new(&data);
        board2.load_board_state(&mut r).unwrap();

        // Verify CPU state matches
        assert_eq!(
            board.cpu.snapshot(),
            board2.cpu.snapshot(),
            "main CPU state mismatch"
        );
        assert_eq!(
            board.sound_cpu.snapshot(),
            board2.sound_cpu.snapshot(),
            "sound CPU state mismatch"
        );

        // Verify RAM
        assert_eq!(board2.read_video_ram(0), 0xAA);
        assert_eq!(board2.read_video_ram(0x5FFF), 0xBB);
        assert_eq!(board2.main_map.region_data(main_region::PALETTE)[3], 0x42);
        assert_eq!(board2.sound_map.region_data(sound_region::RAM)[0x10], 0xCD);

        // Verify CMOS
        assert_eq!(board2.main_map.region_data(main_region::CMOS)[0], 0xF1);
        assert_eq!(board2.main_map.region_data(main_region::CMOS)[100], 0xF9);

        // Verify I/O & timing
        assert_eq!(board2.rom_bank, 5);
        assert_eq!(board2.clock, 123_456);
        assert_eq!(board2.watchdog_counter, 789);
        assert_eq!(board2.rom_pia_input, 0x10);
    }

    #[test]
    fn board_save_load_preserves_rom_unchanged() {
        let mut board = WilliamsBoard::new();
        board.main_map.region_data_mut(main_region::PROGRAM_ROM)[0] = 0xDE;
        board.main_map.region_data_mut(main_region::BANKED_ROM)[0] = 0xAD;
        board.sound_map.region_data_mut(sound_region::ROM)[0] = 0xBE;

        let mut w = StateWriter::new();
        board.save_board_state(&mut w);
        let data = w.into_vec();

        // Load into a board with different ROM contents — ROM should NOT be overwritten
        let mut board2 = WilliamsBoard::new();
        board2.main_map.region_data_mut(main_region::PROGRAM_ROM)[0] = 0x11;
        board2.main_map.region_data_mut(main_region::BANKED_ROM)[0] = 0x22;
        board2.sound_map.region_data_mut(sound_region::ROM)[0] = 0x33;

        let mut r = StateReader::new(&data);
        board2.load_board_state(&mut r).unwrap();

        assert_eq!(
            board2.main_map.region_data(main_region::PROGRAM_ROM)[0],
            0x11,
            "program ROM should be untouched"
        );
        assert_eq!(
            board2.main_map.region_data(main_region::BANKED_ROM)[0],
            0x22,
            "banked ROM should be untouched"
        );
        assert_eq!(
            board2.sound_map.region_data(sound_region::ROM)[0],
            0x33,
            "sound ROM should be untouched"
        );
    }
}
