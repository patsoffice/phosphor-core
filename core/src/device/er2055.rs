use phosphor_macros::Saveable;

/// GI ER2055 Electrically Alterable Read-Only Memory (EAROM)
///
/// 64×4-bit non-volatile storage used for high score tables on early arcade
/// boards (Atari Asteroids Deluxe, Namco Dig Dug, etc.). Data is retained
/// without power via charge storage on internal MOS capacitors.
///
/// The device has a 6-bit address bus (A0–A5), a 4-bit data bus (D0–D3),
/// and three control signals (C1, C2, CK) plus chip-select (CS1). Writes
/// are committed on a rising clock edge when CS1, C1, and C2 are all
/// asserted. Different boards invert C1 differently — callers handle
/// that mapping before calling [`write_control`].
///
/// Although the real device is 64×4, many boards wire the full 8-bit data
/// bus, so we store 8 bits per cell to match MAME and simplify board code.
#[derive(Saveable)]
#[save_version(1)]
pub struct Er2055 {
    data: [u8; 64],
    write_addr: u8,
    write_data: u8,
    last_clock: bool,
}

impl Er2055 {
    pub fn new() -> Self {
        Self {
            data: [0; 64],
            write_addr: 0,
            write_data: 0,
            last_clock: false,
        }
    }

    /// Reset latches to power-on state. Data is preserved (non-volatile).
    pub fn reset(&mut self) {
        self.write_addr = 0;
        self.write_data = 0;
        self.last_clock = false;
    }

    /// Read a byte from EAROM. Offset is masked to 6 bits (0x00–0x3F).
    pub fn read(&self, offset: u16) -> u8 {
        self.data[(offset & 0x3F) as usize]
    }

    /// Latch address and data for a subsequent control commit.
    ///
    /// Called when the CPU writes to the EAROM data/address port. The offset
    /// selects the 6-bit cell address and `data` is the value to be written
    /// once [`write_control`] sees a valid falling clock edge.
    pub fn latch(&mut self, offset: u16, data: u8) {
        self.write_addr = (offset & 0x3F) as u8;
        self.write_data = data;
    }

    /// Process a control register write.
    ///
    /// The caller must decode the board-specific bit layout into these
    /// boolean signals:
    /// - `clock`: CK pin — write commits on falling edge (per ER2055 datasheet)
    /// - `cs1`: chip-select 1
    /// - `c1`: control line 1 (active polarity already resolved by caller)
    /// - `c2`: control line 2
    pub fn write_control(&mut self, clock: bool, cs1: bool, c1: bool, c2: bool) {
        if !clock && self.last_clock && cs1 && c1 && c2 {
            self.data[self.write_addr as usize] = self.write_data;
        }
        self.last_clock = clock;
    }

    /// Load EAROM contents from a byte slice (e.g., from an NVRAM file).
    pub fn load_from(&mut self, src: &[u8]) {
        let len = src.len().min(64);
        self.data[..len].copy_from_slice(&src[..len]);
    }

    /// Get a reference to the full EAROM contents for saving.
    pub fn snapshot(&self) -> &[u8; 64] {
        &self.data
    }
}

impl Default for Er2055 {
    fn default() -> Self {
        Self::new()
    }
}

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for Er2055 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "addr",
                value: self.write_addr as u64,
                width: 8,
            },
            DebugRegister {
                name: "data",
                value: self.write_data as u64,
                width: 8,
            },
        ]
    }
}

impl super::Device for Er2055 {
    fn name(&self) -> &'static str {
        "ER2055"
    }

    fn reset(&mut self) {
        Self::reset(self);
    }

    fn read(&mut self, offset: u16) -> u8 {
        self.data[(offset & 0x3F) as usize]
    }

    fn write(&mut self, offset: u16, data: u8) {
        self.latch(offset, data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_zeroed() {
        let earom = Er2055::new();
        assert!(earom.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn write_requires_falling_clock_edge() {
        let mut earom = Er2055::new();
        earom.latch(0x05, 0xAB);

        // Clock high — no falling edge yet
        earom.write_control(true, true, true, true);
        assert_eq!(earom.read(0x05), 0x00);

        // Falling clock edge with all control lines active — commits write
        earom.write_control(false, true, true, true);
        assert_eq!(earom.read(0x05), 0xAB);
    }

    #[test]
    fn no_write_without_cs1() {
        let mut earom = Er2055::new();
        earom.latch(0x00, 0xFF);
        earom.write_control(true, false, true, true); // clock high
        earom.write_control(false, false, true, true); // falling edge, but cs1 = false
        assert_eq!(earom.read(0x00), 0x00);
    }

    #[test]
    fn no_write_without_c1() {
        let mut earom = Er2055::new();
        earom.latch(0x00, 0xFF);
        earom.write_control(true, true, false, true); // clock high
        earom.write_control(false, true, false, true); // falling edge, but c1 = false
        assert_eq!(earom.read(0x00), 0x00);
    }

    #[test]
    fn no_write_without_c2() {
        let mut earom = Er2055::new();
        earom.latch(0x00, 0xFF);
        earom.write_control(true, true, true, false); // clock high
        earom.write_control(false, true, true, false); // falling edge, but c2 = false
        assert_eq!(earom.read(0x00), 0x00);
    }

    #[test]
    fn no_write_on_low_clock_without_falling_edge() {
        let mut earom = Er2055::new();

        // Bring clock high then low (falling edge commits default latch: addr 0, data 0)
        earom.write_control(true, true, true, true);
        earom.write_control(false, true, true, true);

        // Latch new data while clock is already low
        earom.latch(0x01, 0x42);
        earom.write_control(false, true, true, true); // still low, no falling edge
        assert_eq!(earom.read(0x01), 0x00);

        // Cycle clock: high then low (falling edge)
        earom.write_control(true, true, true, true);
        earom.write_control(false, true, true, true);
        assert_eq!(earom.read(0x01), 0x42);
    }

    #[test]
    fn offset_masking() {
        let mut earom = Er2055::new();
        earom.latch(0x3F, 0xDE);
        earom.write_control(true, true, true, true);
        earom.write_control(false, true, true, true); // falling edge commits
        // Offset 0xFF masks to 0x3F
        assert_eq!(earom.read(0xFF), 0xDE);
    }

    #[test]
    fn load_from_and_snapshot() {
        let mut earom = Er2055::new();
        let mut src = [0u8; 64];
        src[0] = 0x11;
        src[32] = 0x22;
        src[63] = 0x33;
        earom.load_from(&src);

        assert_eq!(earom.read(0), 0x11);
        assert_eq!(earom.read(32), 0x22);
        assert_eq!(earom.read(63), 0x33);
        assert_eq!(earom.snapshot(), &src);
    }

    #[test]
    fn load_from_short_slice() {
        let mut earom = Er2055::new();
        earom.latch(32, 0xFF);
        earom.write_control(true, true, true, true);
        earom.write_control(false, true, true, true); // falling edge commits

        let src = [0xBB; 16];
        earom.load_from(&src);
        assert_eq!(earom.read(0), 0xBB);
        assert_eq!(earom.read(15), 0xBB);
        assert_eq!(earom.read(32), 0xFF); // untouched
    }

    #[test]
    fn reset_preserves_data() {
        let mut earom = Er2055::new();
        earom.latch(0x0A, 0x77);
        earom.write_control(true, true, true, true);
        earom.write_control(false, true, true, true); // falling edge commits

        earom.reset();
        assert_eq!(earom.read(0x0A), 0x77); // data survives reset
        assert_eq!(earom.write_addr, 0);
        assert_eq!(earom.write_data, 0);
        assert!(!earom.last_clock);
    }
}
