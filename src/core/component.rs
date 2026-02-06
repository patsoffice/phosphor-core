use super::bus::BusMaster;

/// Anything that advances by discrete time units (CPUs, video chips, sound chips)
pub trait Component {
    /// Advance one clock cycle in this component's clock domain.
    /// Returns true if a "significant event" occurred (e.g., instruction boundary, frame ready).
    fn tick(&mut self) -> bool;

    /// Get the master clock cycles consumed per tick (for clock domain crossing).
    fn clock_divider(&self) -> u64 { 1 }
}

/// Extension for components that act as bus masters (CPUs, DMA controllers)
pub trait BusMasterComponent: Component {
    type Bus: super::bus::Bus + ?Sized;

    /// Execute one cycle with bus access. Returns true at instruction boundary.
    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master_id: BusMaster) -> bool;
}