use phosphor_core::bus_split;
use phosphor_core::core::bus::InterruptState;
use phosphor_core::core::machine::{
    AudioSource, InputButton, InputReceiver, Machine, MachineDebug, Renderable,
};
use phosphor_core::core::save_state::{self, SaveError, StateWriter};
use phosphor_core::core::{Bus, BusMaster};
use phosphor_core::cpu::Cpu;

use crate::mcr2::{self, Mcr2Board};
use crate::registry::MachineEntry;
use crate::rom_loader::{RomEntry, RomLoadError, RomRegion, RomSet};
use crate::set_bit_active_low;

// ---------------------------------------------------------------------------
// ROM definitions (from MAME mcr.cpp — shollow parent set)
// ---------------------------------------------------------------------------

static SHOLLOW_PROGRAM_ROM: RomRegion = RomRegion {
    size: 0xC000, // 48KB
    entries: &[
        RomEntry {
            name: "sh-pro.00",
            size: 0x2000,
            offset: 0x0000,
            crc32: &[0x95e2b800],
        },
        RomEntry {
            name: "sh-pro.01",
            size: 0x2000,
            offset: 0x2000,
            crc32: &[0xb99f6ff8],
        },
        RomEntry {
            name: "sh-pro.02",
            size: 0x2000,
            offset: 0x4000,
            crc32: &[0x1202c7b2],
        },
        RomEntry {
            name: "sh-pro.03",
            size: 0x2000,
            offset: 0x6000,
            crc32: &[0x0a64afb9],
        },
        RomEntry {
            name: "sh-pro.04",
            size: 0x2000,
            offset: 0x8000,
            crc32: &[0x22fa9175],
        },
        RomEntry {
            name: "sh-pro.05",
            size: 0x2000,
            offset: 0xA000,
            crc32: &[0x1716e2bb],
        },
    ],
};

static SHOLLOW_SOUND_ROM: RomRegion = RomRegion {
    size: 0x4000, // 16KB (12KB data + 4KB padding)
    entries: &[
        RomEntry {
            name: "sh-snd.01",
            size: 0x1000,
            offset: 0x0000,
            crc32: &[0x55a297cc],
        },
        RomEntry {
            name: "sh-snd.02",
            size: 0x1000,
            offset: 0x1000,
            crc32: &[0x46fc31f6],
        },
        RomEntry {
            name: "sh-snd.03",
            size: 0x1000,
            offset: 0x2000,
            crc32: &[0xb1f4a6a8],
        },
    ],
};

static SHOLLOW_BG_ROM: RomRegion = RomRegion {
    size: 0x4000, // 16KB (2 × 8KB)
    entries: &[
        RomEntry {
            name: "sh-bg.00",
            size: 0x2000,
            offset: 0x0000,
            crc32: &[0x3e2b333c],
        },
        RomEntry {
            name: "sh-bg.01",
            size: 0x2000,
            offset: 0x2000,
            crc32: &[0xd1d70cc4],
        },
    ],
};

static SHOLLOW_FG_ROM: RomRegion = RomRegion {
    size: 0x8000, // 32KB (4 × 8KB)
    entries: &[
        RomEntry {
            name: "sh-fg.00",
            size: 0x2000,
            offset: 0x0000,
            crc32: &[0x33f4554e],
        },
        RomEntry {
            name: "sh-fg.01",
            size: 0x2000,
            offset: 0x2000,
            crc32: &[0xba1a38b4],
        },
        RomEntry {
            name: "sh-fg.02",
            size: 0x2000,
            offset: 0x4000,
            crc32: &[0x6b57f6da],
        },
        RomEntry {
            name: "sh-fg.03",
            size: 0x2000,
            offset: 0x6000,
            crc32: &[0x37ea9d07],
        },
    ],
};

// ---------------------------------------------------------------------------
// Input definitions
// ---------------------------------------------------------------------------

// IP0: coin/start (active-low)
const INPUT_COIN1: u8 = 0;
const INPUT_COIN2: u8 = 1;
const INPUT_START1: u8 = 2;
const INPUT_START2: u8 = 3;
const INPUT_TILT: u8 = 4;
const INPUT_SERVICE: u8 = 5;

// IP1: player controls (active-low)
const INPUT_LEFT: u8 = 6;
const INPUT_RIGHT: u8 = 7;
const INPUT_SHIELD: u8 = 8;
const INPUT_FIRE: u8 = 9;

const SHOLLOW_INPUT_MAP: &[InputButton] = &[
    InputButton {
        id: INPUT_COIN1,
        name: "Coin 1",
    },
    InputButton {
        id: INPUT_COIN2,
        name: "Coin 2",
    },
    InputButton {
        id: INPUT_START1,
        name: "1P Start",
    },
    InputButton {
        id: INPUT_START2,
        name: "2P Start",
    },
    InputButton {
        id: INPUT_TILT,
        name: "Tilt",
    },
    InputButton {
        id: INPUT_SERVICE,
        name: "Service",
    },
    InputButton {
        id: INPUT_LEFT,
        name: "Left",
    },
    InputButton {
        id: INPUT_RIGHT,
        name: "Right",
    },
    InputButton {
        id: INPUT_SHIELD,
        name: "Shield",
    },
    InputButton {
        id: INPUT_FIRE,
        name: "Fire",
    },
];

// ---------------------------------------------------------------------------
// SatansHollowSystem
// ---------------------------------------------------------------------------

/// Satan's Hollow (1982, Bally Midway) — MCR II platform.
///
/// Thin wrapper around `Mcr2Board` providing game-specific ROM loading,
/// input wiring, and `Bus` implementation for the main Z80's memory/IO map.
pub struct SatansHollowSystem {
    pub board: Mcr2Board,
}

impl SatansHollowSystem {
    pub fn new() -> Self {
        Self {
            board: Mcr2Board::new(),
        }
    }

    pub fn load_rom_set(&mut self, rom_set: &RomSet) -> Result<(), RomLoadError> {
        // Program ROM
        let prog_data = SHOLLOW_PROGRAM_ROM.load(rom_set)?;
        self.board.rom[..prog_data.len()].copy_from_slice(&prog_data);

        // Sound ROM (loaded into SSIO)
        let sound_data = SHOLLOW_SOUND_ROM.load(rom_set)?;
        self.board.ssio.load_rom(&sound_data);

        // GFX ROMs
        let bg_data = SHOLLOW_BG_ROM.load(rom_set)?;
        let fg_data = SHOLLOW_FG_ROM.load(rom_set)?;
        self.board.decode_gfx(&bg_data, &fg_data);

        // Set IP3 to active-high idle (0x00 instead of default 0xFF)
        self.board.ssio.set_input_port(3, 0x00);

        Ok(())
    }
}

impl Default for SatansHollowSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bus — MCR II main CPU memory and I/O map
// ---------------------------------------------------------------------------

impl Bus for SatansHollowSystem {
    type Address = u16;
    type Data = u8;

    fn read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => {
                let a = addr as usize;
                if a < self.board.rom.len() {
                    self.board.rom[a]
                } else {
                    0xFF
                }
            }
            0xC000..=0xC7FF => self.board.nvram[(addr - 0xC000) as usize],
            0xE000..=0xE1FF => self.board.sprite_ram[(addr - 0xE000) as usize],
            0xE800..=0xEFFF => self.board.video_ram[(addr - 0xE800) as usize],
            0xF000..=0xF07F => self.board.palette_ram[(addr - 0xF000) as usize],
            _ => 0xFF,
        }
    }

    fn write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        match addr {
            0xC000..=0xC7FF => self.board.nvram[(addr - 0xC000) as usize] = data,
            0xE000..=0xE1FF => self.board.sprite_ram[(addr - 0xE000) as usize] = data,
            0xE800..=0xEFFF => self.board.video_ram[(addr - 0xE800) as usize] = data,
            0xF000..=0xF07F => {
                let offset = (addr - 0xF000) as usize;
                self.board.palette_ram[offset] = data;
                self.board.update_palette_entry(offset);
            }
            _ => {} // ROM or unmapped
        }
    }

    fn io_read(&mut self, _master: BusMaster, addr: u16) -> u8 {
        let port = addr as u8;
        match port {
            0x00..=0x04 => self.board.ssio.input_port(port as usize),
            0x07 => self.board.ssio.status_read(),
            0xF0..=0xF3 => self.board.ctc.read(port - 0xF0),
            _ => 0xFF,
        }
    }

    fn io_write(&mut self, _master: BusMaster, addr: u16, data: u8) {
        let port = addr as u8;
        match port {
            0x1C..=0x1F => self.board.ssio.latch_write(port - 0x1C, data),
            0xE0 => self.board.watchdog_counter = 0,
            0xF0..=0xF3 => self.board.ctc.write(port - 0xF0, data),
            _ => {}
        }
    }

    fn is_halted_for(&self, _master: BusMaster) -> bool {
        false
    }

    fn check_interrupts(&self, target: BusMaster) -> InterruptState {
        match target {
            BusMaster::Cpu(0) => {
                if self.board.ctc.interrupt_pending() {
                    let vector = self.board.ctc.interrupt_vector();
                    self.board.ctc_vector_latch.set(vector);
                    self.board.ctc_ack_needed.set(true);
                    InterruptState {
                        irq: true,
                        irq_vector: vector,
                        ..Default::default()
                    }
                } else {
                    // Return latched vector for INTA cycle (Z80 reads irq_vector
                    // during interrupt acknowledge regardless of irq flag)
                    InterruptState {
                        irq_vector: self.board.ctc_vector_latch.get(),
                        ..Default::default()
                    }
                }
            }
            _ => InterruptState::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Machine trait implementations
// ---------------------------------------------------------------------------

impl Renderable for SatansHollowSystem {
    mcr2::impl_mcr2_renderable!();
}

impl AudioSource for SatansHollowSystem {
    mcr2::impl_mcr2_audio!();
}

impl InputReceiver for SatansHollowSystem {
    fn set_input(&mut self, button: u8, pressed: bool) {
        let (port, bit) = match button {
            // IP0: coin/start/service (active-low, bits 0-7)
            INPUT_COIN1 => (0usize, 0u8),
            INPUT_COIN2 => (0, 1),
            INPUT_START1 => (0, 2),
            INPUT_START2 => (0, 3),
            INPUT_TILT => (0, 5),
            INPUT_SERVICE => (0, 7),
            // IP1: player controls (active-low)
            INPUT_LEFT => (1, 0),
            INPUT_RIGHT => (1, 1),
            INPUT_SHIELD => (1, 2),
            INPUT_FIRE => (1, 3),
            _ => return,
        };
        let mut val = self.board.ssio.input_port(port);
        set_bit_active_low(&mut val, bit, pressed);
        self.board.ssio.set_input_port(port, val);
    }

    fn input_map(&self) -> &[InputButton] {
        SHOLLOW_INPUT_MAP
    }
}

impl MachineDebug for SatansHollowSystem {
    mcr2::impl_mcr2_debug!();

    fn debug_tick(&mut self) -> u32 {
        bus_split!(self, bus => {
            self.board.tick(bus);
        });
        self.board.debug_tick_boundaries()
    }
}

impl Machine for SatansHollowSystem {
    mcr2::impl_mcr2_machine_common!();

    fn run_frame(&mut self) {
        bus_split!(self, bus => {
            for _ in 0..mcr2::CYCLES_PER_FRAME {
                self.board.tick(bus);
            }
        });
        self.board.render_frame_internal();
    }

    fn reset(&mut self) {
        self.board.reset_board();
        bus_split!(self, bus => {
            self.board.cpu.reset(bus, BusMaster::Cpu(0));
        });
        // Re-initialize IP3 to active-high idle
        self.board.ssio.set_input_port(3, 0x00);
    }

    fn machine_id(&self) -> &str {
        "shollow"
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        let mut w = StateWriter::new();
        save_state::write_header(&mut w, self.machine_id());
        self.board.save_board_state(&mut w);
        Some(w.into_vec())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), SaveError> {
        let mut r = save_state::read_header(data, self.machine_id())?;
        self.board.load_board_state(&mut r)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Machine registry
// ---------------------------------------------------------------------------

fn create_machine(rom_set: &RomSet) -> Result<Box<dyn Machine>, RomLoadError> {
    let mut sys = SatansHollowSystem::new();
    sys.load_rom_set(rom_set)?;
    Ok(Box::new(sys))
}

inventory::submit! {
    MachineEntry::new("shollow", "shollow", create_machine)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use phosphor_core::core::machine::Machine;
    use phosphor_core::cpu::CpuStateTrait;

    #[test]
    fn save_load_round_trip() {
        let mut sys = SatansHollowSystem::new();

        // Set known state
        sys.board.nvram[0x100] = 0xAA;
        sys.board.video_ram[0x50] = 0xBB;
        sys.board.sprite_ram[0x10] = 0xCC;
        sys.board.palette_ram[0] = 0x55;
        sys.board.clock = 50_000;
        sys.board.watchdog_counter = 42;

        // Save
        let data = sys.save_state().expect("save_state should return Some");

        // Capture CPU snapshot
        let cpu_snap = sys.board.cpu.snapshot();

        // Load into fresh system
        let mut sys2 = SatansHollowSystem::new();
        sys2.load_state(&data).unwrap();

        // Verify
        assert_eq!(sys2.board.cpu.snapshot(), cpu_snap);
        assert_eq!(sys2.board.nvram[0x100], 0xAA);
        assert_eq!(sys2.board.video_ram[0x50], 0xBB);
        assert_eq!(sys2.board.sprite_ram[0x10], 0xCC);
        assert_eq!(sys2.board.palette_ram[0], 0x55);
        assert_eq!(sys2.board.clock, 50_000);
        assert_eq!(sys2.board.watchdog_counter, 42);
    }

    #[test]
    fn save_load_machine_id_validated() {
        let sys = SatansHollowSystem::new();
        let data = sys.save_state().unwrap();

        // Tamper with the machine ID
        let mut bad = data.clone();
        let id_offset = 4 + 4 + 4; // magic(4) + version(4) + id_len(4)
        bad[id_offset..id_offset + 7].copy_from_slice(b"badname");

        let mut sys2 = SatansHollowSystem::new();
        let result = sys2.load_state(&bad);
        assert!(result.is_err(), "should reject mismatched machine ID");
    }

    #[test]
    fn save_does_not_include_rom() {
        let mut sys = SatansHollowSystem::new();
        sys.board.rom[0] = 0xDE;

        let data = sys.save_state().unwrap();

        // Load into system with different ROM
        let mut sys2 = SatansHollowSystem::new();
        sys2.board.rom[0] = 0x11;
        sys2.load_state(&data).unwrap();

        assert_eq!(sys2.board.rom[0], 0x11, "ROM should be untouched");
    }

    #[test]
    fn palette_9bit_decode() {
        let mut board = Mcr2Board::new();

        // Entry 0: val9 = 0x1FF (all bits set)
        // R = pal3bit(0x1FF >> 6) = pal3bit(7) = 255
        // G = pal3bit(0x1FF & 7) = pal3bit(7) = 255
        // B = pal3bit((0x1FF >> 3) & 7) = pal3bit(7) = 255
        board.palette_ram[0] = 0xFF; // low byte
        board.palette_ram[1] = 0x01; // high byte (bit 0 = 1)
        board.rebuild_palette();
        assert_eq!(board.palette_rgb[0], (255, 255, 255));

        // Entry 1: val9 = 0x000
        board.palette_ram[2] = 0x00;
        board.palette_ram[3] = 0x00;
        board.rebuild_palette();
        assert_eq!(board.palette_rgb[1], (0, 0, 0));

        // Entry 2: val9 = 0x049 = 0b001_001_001
        // R = pal3bit(1) = 36, G = pal3bit(1) = 36, B = pal3bit(1) = 36
        board.palette_ram[4] = 0x49;
        board.palette_ram[5] = 0x00;
        board.rebuild_palette();
        assert_eq!(board.palette_rgb[2], (36, 36, 36));
    }

    #[test]
    fn input_active_low() {
        let mut sys = SatansHollowSystem::new();

        // Initially all inputs are idle (0xFF for active-low ports)
        assert_eq!(sys.board.ssio.input_port(0), 0xFF);
        assert_eq!(sys.board.ssio.input_port(1), 0xFF);

        // Press coin
        sys.set_input(INPUT_COIN1, true);
        assert_eq!(sys.board.ssio.input_port(0), 0xFE); // bit 0 cleared

        // Release coin
        sys.set_input(INPUT_COIN1, false);
        assert_eq!(sys.board.ssio.input_port(0), 0xFF); // bit 0 set again

        // Press fire
        sys.set_input(INPUT_FIRE, true);
        assert_eq!(sys.board.ssio.input_port(1), 0xF7); // bit 3 cleared
    }

    #[test]
    fn io_read_ssio_input_ports() {
        let mut sys = SatansHollowSystem::new();
        sys.board.ssio.set_input_port(0, 0xAA);
        sys.board.ssio.set_input_port(1, 0xBB);

        assert_eq!(Bus::io_read(&mut sys, BusMaster::Cpu(0), 0x00), 0xAA);
        assert_eq!(Bus::io_read(&mut sys, BusMaster::Cpu(0), 0x01), 0xBB);
    }

    #[test]
    fn io_write_ctc() {
        let mut sys = SatansHollowSystem::new();

        // Write vector base to CTC channel 0
        Bus::io_write(&mut sys, BusMaster::Cpu(0), 0xF0, 0xE0);
        assert_eq!(sys.board.ctc.read(0), 0); // counter value (vector base isn't readable this way)
    }

    #[test]
    fn memory_map_rom_read() {
        let mut sys = SatansHollowSystem::new();
        sys.board.rom[0] = 0x42;
        sys.board.rom[0xBFFF] = 0x77;

        assert_eq!(Bus::read(&mut sys, BusMaster::Cpu(0), 0x0000), 0x42);
        assert_eq!(Bus::read(&mut sys, BusMaster::Cpu(0), 0xBFFF), 0x77);
    }

    #[test]
    fn memory_map_nvram_read_write() {
        let mut sys = SatansHollowSystem::new();

        Bus::write(&mut sys, BusMaster::Cpu(0), 0xC000, 0x55);
        assert_eq!(Bus::read(&mut sys, BusMaster::Cpu(0), 0xC000), 0x55);
        assert_eq!(sys.board.nvram[0], 0x55);
    }
}
