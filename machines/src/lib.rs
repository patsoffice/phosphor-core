/// Active-high bit manipulation: set bit on press, clear on release.
pub(crate) fn set_bit_active_high(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg |= 1 << bit;
    } else {
        *reg &= !(1 << bit);
    }
}

/// Active-low bit manipulation: clear bit on press, set bit on release.
pub(crate) fn set_bit_active_low(reg: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *reg &= !(1 << bit);
    } else {
        *reg |= 1 << bit;
    }
}

/// Implements `MachineDebug` for standalone machines (single CPU, flat bus).
///
/// Requires the type to:
/// - Have a `cpu` field with `at_instruction_boundary()`
/// - Have a `tick()` method
/// - Implement `BusDebug` on `Self`
/// - Have `CYCLES_PER_FRAME` in scope
macro_rules! impl_standalone_debug {
    ($type:ty) => {
        impl phosphor_core::core::machine::MachineDebug for $type {
            fn debug_bus(&self) -> Option<&dyn phosphor_core::core::debug::BusDebug> {
                Some(self)
            }

            fn debug_bus_mut(&mut self) -> Option<&mut dyn phosphor_core::core::debug::BusDebug> {
                Some(self)
            }

            fn cycles_per_frame(&self) -> u64 {
                TIMING.cycles_per_frame()
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
    };
}
pub(crate) use impl_standalone_debug;

pub mod astdelux;
pub mod asteroids;
pub mod atari_dvg;
pub mod ccastles;
pub mod digdug;
pub mod donkey_kong;
pub mod donkey_kong_jr;
pub mod gottlieb;
pub mod gridlee;
pub mod joust;
pub mod llander;
pub mod mcr2;
pub mod missile_command;
pub mod mspacman;
pub mod namco_galaga;
pub mod namco_pac;
pub mod pacman;
pub mod qbert;
pub mod registry;
pub mod robotron;
pub mod rom_loader;
pub mod satans_hollow;
pub mod simple6502;
pub mod simple6800;
pub mod simple6809;
pub mod simplez80;
pub mod tkg04;
pub mod williams;

pub use astdelux::AsteroidsDeluxeSystem;
pub use asteroids::AsteroidsSystem;
pub use atari_dvg::AtariDvgBoard;
pub use ccastles::CrystalCastlesSystem;
pub use digdug::DigDugSystem;
pub use donkey_kong::DkongSystem;
pub use donkey_kong_jr::DkongJrSystem;
pub use gridlee::GridleeSystem;
pub use joust::JoustSystem;
pub use llander::LunarLanderSystem;
pub use missile_command::MissileCommandSystem;
pub use mspacman::MsPacmanSystem;
pub use pacman::PacmanSystem;
pub use qbert::QbertSystem;
pub use robotron::RobotronSystem;
pub use satans_hollow::SatansHollowSystem;
pub use simple6502::Simple6502System;
pub use simple6800::Simple6800System;
pub use simple6809::Simple6809System;
pub use simplez80::SimpleZ80System;
