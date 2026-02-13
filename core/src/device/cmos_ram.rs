/// Battery-backed CMOS RAM (1KB)
///
/// Simple read/write memory that can be saved/loaded for persistence
/// across sessions. On Williams arcade hardware, this stores high scores,
/// game settings, and audit counters. The RAM is standard read/write
/// memory; the "battery-backed" aspect is a board-level feature.
pub struct CmosRam {
    data: [u8; 1024],
}

impl CmosRam {
    /// Create a new CMOS RAM initialized to all zeros.
    pub fn new() -> Self {
        Self { data: [0; 1024] }
    }

    /// Read a byte. Offset is masked to 10 bits (0x000-0x3FF).
    pub fn read(&self, offset: u16) -> u8 {
        self.data[(offset & 0x03FF) as usize]
    }

    /// Write a byte. Offset is masked to 10 bits (0x000-0x3FF).
    pub fn write(&mut self, offset: u16, value: u8) {
        self.data[(offset & 0x03FF) as usize] = value;
    }

    /// Load CMOS contents from a byte slice (e.g., from a save file).
    ///
    /// If `src` is shorter than 1024 bytes, only the first `src.len()` bytes
    /// are written. If longer, only the first 1024 bytes are used.
    pub fn load_from(&mut self, src: &[u8]) {
        let len = src.len().min(1024);
        self.data[..len].copy_from_slice(&src[..len]);
    }

    /// Get a reference to the full CMOS contents for saving.
    pub fn snapshot(&self) -> &[u8; 1024] {
        &self.data
    }
}

impl Default for CmosRam {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_zeroed() {
        let ram = CmosRam::new();
        assert!(ram.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn read_write_basic() {
        let mut ram = CmosRam::new();
        ram.write(0x00, 0x42);
        assert_eq!(ram.read(0x00), 0x42);
        ram.write(0x1FF, 0xAB);
        assert_eq!(ram.read(0x1FF), 0xAB);
    }

    #[test]
    fn offset_masking_wraps_at_1024() {
        let mut ram = CmosRam::new();
        ram.write(0, 0xDE);
        // Offset 0x400 (1024) masks to 0x000
        assert_eq!(ram.read(0x400), 0xDE);
    }

    #[test]
    fn offset_masking_high_bits() {
        let mut ram = CmosRam::new();
        ram.write(0x3FF, 0xBE);
        // 0xFFFF & 0x03FF = 0x3FF
        assert_eq!(ram.read(0xFFFF), 0xBE);
    }

    #[test]
    fn last_valid_offset() {
        let mut ram = CmosRam::new();
        ram.write(0x3FF, 0xEF);
        assert_eq!(ram.read(0x3FF), 0xEF);
    }

    #[test]
    fn load_from_exact_size() {
        let mut ram = CmosRam::new();
        let src = [0xAA; 1024];
        ram.load_from(&src);
        assert_eq!(ram.snapshot(), &src);
    }

    #[test]
    fn load_from_short_slice() {
        let mut ram = CmosRam::new();
        ram.write(512, 0xFF); // pre-existing data beyond load range
        let src = [0xBB; 512];
        ram.load_from(&src);
        // First 512 bytes overwritten
        assert_eq!(ram.read(0), 0xBB);
        assert_eq!(ram.read(511), 0xBB);
        // Byte at 512 unchanged
        assert_eq!(ram.read(512), 0xFF);
    }

    #[test]
    fn load_from_long_slice() {
        let mut ram = CmosRam::new();
        let src = [0xCC; 2048];
        ram.load_from(&src);
        // Only first 1024 bytes used
        assert!(ram.data.iter().all(|&b| b == 0xCC));
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut ram1 = CmosRam::new();
        ram1.write(0, 0x11);
        ram1.write(100, 0x22);
        ram1.write(0x3FF, 0x33);

        let saved = *ram1.snapshot();
        let mut ram2 = CmosRam::new();
        ram2.load_from(&saved);

        assert_eq!(ram2.read(0), 0x11);
        assert_eq!(ram2.read(100), 0x22);
        assert_eq!(ram2.read(0x3FF), 0x33);
    }

    #[test]
    fn default_is_same_as_new() {
        let ram = CmosRam::default();
        assert!(ram.data.iter().all(|&b| b == 0));
    }
}
