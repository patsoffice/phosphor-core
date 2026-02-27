use super::bus::BusMaster;

/// Extension for components that act as bus masters (CPUs, DMA controllers)
pub trait BusMasterComponent {
    type Bus: super::bus::Bus + ?Sized;

    /// Execute one cycle with bus access. Returns true at instruction boundary.
    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master_id: BusMaster) -> bool;
}
