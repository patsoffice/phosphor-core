use crate::core::component::BusMasterComponent;

/// Generic CPU interface
pub trait Cpu: BusMasterComponent {
    /// Reset vector fetch
    fn reset(&mut self);

    /// Signal a specific interrupt line (implementation-defined)
    fn signal_interrupt(&mut self, int: crate::core::bus::InterruptState);

    /// Query if CPU is halted internally (CWAI, WAI, STOP instruction)
    fn is_sleeping(&self) -> bool;
}

// Re-export specific CPUs
pub mod m6809;
pub use m6809::M6809;

// Placeholder for future
pub mod m6502;
pub use m6502::M6502;
