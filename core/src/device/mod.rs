use crate::core::debug::Debuggable;
use crate::core::save_state::Saveable;

/// Common interface for emulated hardware peripheral devices.
///
/// Every peripheral device (PIA, sound chip, blitter, etc.) implements
/// this trait to establish a uniform contract for debug inspection,
/// save/load state, power-on reset, and optional register access.
///
/// Devices are owned as concrete types by board structs and used via
/// direct method calls (inherent methods shadow trait methods in normal
/// calls, so no existing call sites need updating).
pub trait Device: Debuggable + Saveable {
    /// Human-readable device type name (e.g., "AY-8910", "PIA 6820").
    fn name(&self) -> &'static str;

    /// Reset to power-on state. Configuration (clock rates, ROM data,
    /// variant selection) is preserved; only runtime state is cleared.
    fn reset(&mut self);

    /// Read a device register by offset. Default returns 0xFF (no register file).
    fn read(&mut self, _offset: u8) -> u8 {
        0xFF
    }

    /// Write a device register by offset. Default is a no-op.
    fn write(&mut self, _offset: u8, _data: u8) {}

    /// Advance the device by one clock tick. Default is a no-op.
    fn tick(&mut self) {}
}

pub mod ay8910;
pub mod cmos_ram;
pub mod dac;
pub mod dkong_discrete;
pub mod dvg;
pub mod i8257;
pub mod namco_wsg;
pub mod output_latch;
pub mod pia6820;
pub mod pokey;
pub mod ssio;
pub mod williams_blitter;
pub mod z80ctc;

pub use ay8910::Ay8910;
pub use cmos_ram::CmosRam;
pub use dac::Mc1408Dac;
pub use dkong_discrete::DkongDiscrete;
pub use dvg::Dvg;
pub use i8257::I8257;
pub use namco_wsg::NamcoWsg;
pub use output_latch::OutputLatch;
pub use pia6820::Pia6820;
pub use pokey::Pokey;
pub use ssio::SsioBoard;
pub use williams_blitter::WilliamsBlitter;
pub use z80ctc::Z80Ctc;
