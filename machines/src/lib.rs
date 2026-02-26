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

pub mod asteroids;
pub mod ccastles;
pub mod donkey_kong;
pub mod donkey_kong_jr;
pub mod gridlee;
pub mod joust;
pub mod mcr2;
pub mod missile_command;
pub mod pacman;
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

pub use asteroids::AsteroidsSystem;
pub use ccastles::CrystalCastlesSystem;
pub use donkey_kong::DkongSystem;
pub use donkey_kong_jr::DkongJrSystem;
pub use gridlee::GridleeSystem;
pub use joust::JoustSystem;
pub use missile_command::MissileCommandSystem;
pub use pacman::PacmanSystem;
pub use robotron::RobotronSystem;
pub use satans_hollow::SatansHollowSystem;
pub use simple6502::Simple6502System;
pub use simple6800::Simple6800System;
pub use simple6809::Simple6809System;
pub use simplez80::SimpleZ80System;
