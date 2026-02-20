//! Machine registry for automatic front-end discovery.
//!
//! Each front-end-capable machine self-registers via [`inventory::submit!`]
//! with a [`MachineEntry`] containing its CLI name, MAME ROM set name, and a
//! factory function. The front-end discovers available machines at runtime
//! without any central list.

use phosphor_core::core::machine::Machine;

use crate::rom_loader::{RomLoadError, RomSet};

/// Describes a front-end-capable arcade machine.
pub struct MachineEntry {
    /// CLI name used to select this machine (e.g., "joust").
    pub name: &'static str,
    /// MAME ROM set name for ZIP lookup (e.g., "joust").
    pub rom_name: &'static str,
    /// Factory: construct a Machine from a loaded ROM set.
    pub create: fn(&RomSet) -> Result<Box<dyn Machine>, RomLoadError>,
}

impl MachineEntry {
    pub const fn new(
        name: &'static str,
        rom_name: &'static str,
        create: fn(&RomSet) -> Result<Box<dyn Machine>, RomLoadError>,
    ) -> Self {
        Self {
            name,
            rom_name,
            create,
        }
    }
}

inventory::collect!(MachineEntry);

/// Return all registered front-end-capable machines, sorted by name.
pub fn all() -> Vec<&'static MachineEntry> {
    let mut entries: Vec<_> = inventory::iter::<MachineEntry>.into_iter().collect();
    entries.sort_by_key(|e| e.name);
    entries
}

/// Look up a machine by its CLI name.
pub fn find(name: &str) -> Option<&'static MachineEntry> {
    inventory::iter::<MachineEntry>
        .into_iter()
        .find(|e| e.name == name)
}
