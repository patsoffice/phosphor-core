pub mod core;
pub mod cpu;
pub mod device;

pub mod prelude {
    pub use crate::core::{bus::InterruptState, Bus, BusMaster, BusMasterComponent, Component};
    pub use crate::cpu::Cpu;
}
