pub mod core;
pub mod cpu;
pub mod device;
pub mod gfx;

pub mod prelude {
    pub use crate::core::machine::{AnalogInput, InputButton, Machine};
    pub use crate::core::{
        Bus, BusMaster, BusMasterComponent, Component, SaveError, Saveable, StateReader,
        StateWriter, bus::InterruptState,
    };
    pub use crate::cpu::Cpu;
}
