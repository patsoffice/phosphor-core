pub mod core;
pub mod cpu;
pub mod device;

pub mod prelude {
    pub use crate::core::machine::{InputButton, Machine};
    pub use crate::core::{Bus, BusMaster, BusMasterComponent, Component, bus::InterruptState};
    pub use crate::cpu::Cpu;
}
