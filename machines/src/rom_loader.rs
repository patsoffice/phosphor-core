//! ROM loading and validation for arcade machine emulation.
//!
//! Supports loading ROM files from pre-extracted MAME ROM directories
//! or programmatic byte slices (for testing). ROM files can include
//! CRC32 checksums for validation, with an option to skip validation
//! for modified or development ROMs.

use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// CRC-32 (private)
// ---------------------------------------------------------------------------

/// CRC-32 lookup table (reflected polynomial 0xEDB88320).
/// Same algorithm as MAME, ZIP, PNG, and Ethernet.
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute the CRC-32 checksum of a byte slice.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    crc ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when loading a ROM set.
#[derive(Debug)]
pub enum RomLoadError {
    /// Underlying I/O error (file not found, permission denied, etc.)
    Io(std::io::Error),

    /// A required ROM file was not found in the set.
    MissingFile(String),

    /// ROM file size does not match the expected size.
    SizeMismatch {
        file: String,
        expected: usize,
        actual: usize,
    },

    /// CRC32 checksum does not match the expected value.
    ChecksumMismatch {
        file: String,
        expected: u32,
        actual: u32,
    },
}

impl std::fmt::Display for RomLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::MissingFile(name) => write!(f, "missing ROM file: {name}"),
            Self::SizeMismatch {
                file,
                expected,
                actual,
            } => write!(f, "ROM {file}: expected {expected} bytes, got {actual}"),
            Self::ChecksumMismatch {
                file,
                expected,
                actual,
            } => write!(
                f,
                "ROM {file}: CRC32 expected 0x{expected:08X}, got 0x{actual:08X}"
            ),
        }
    }
}

impl std::error::Error for RomLoadError {}

impl From<std::io::Error> for RomLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// RomSet
// ---------------------------------------------------------------------------

/// A collection of ROM files loaded from disk or provided programmatically.
pub struct RomSet {
    files: HashMap<String, Vec<u8>>,
}

impl RomSet {
    /// Create a RomSet from a directory of extracted ROM files.
    ///
    /// Reads all files in the directory (non-recursive) and stores
    /// them by filename (without path).
    pub fn from_directory(path: &Path) -> Result<Self, RomLoadError> {
        let mut files = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.is_file() {
                let name = file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let data = std::fs::read(&file_path)?;
                files.insert(name, data);
            }
        }
        Ok(Self { files })
    }

    /// Create a RomSet from programmatic byte slices (for testing).
    ///
    /// Each entry is a (filename, data) pair.
    pub fn from_slices(entries: &[(&str, &[u8])]) -> Self {
        let mut files = HashMap::new();
        for (name, data) in entries {
            files.insert(name.to_string(), data.to_vec());
        }
        Self { files }
    }

    /// Get a ROM file's data by name.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(|v| v.as_slice())
    }

    /// Get a ROM file's data, returning an error if missing.
    pub fn require(&self, name: &str) -> Result<&[u8], RomLoadError> {
        self.get(name)
            .ok_or_else(|| RomLoadError::MissingFile(name.to_string()))
    }

    /// Get a ROM file's data, validating its size.
    pub fn require_sized(&self, name: &str, expected_size: usize) -> Result<&[u8], RomLoadError> {
        let data = self.require(name)?;
        if data.len() != expected_size {
            return Err(RomLoadError::SizeMismatch {
                file: name.to_string(),
                expected: expected_size,
                actual: data.len(),
            });
        }
        Ok(data)
    }

    /// List all file names in the set.
    pub fn file_names(&self) -> Vec<&str> {
        self.files.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// RomEntry / RomRegion
// ---------------------------------------------------------------------------

/// Describes how a single ROM file maps into a memory region.
pub struct RomEntry {
    /// Filename in the ROM set.
    pub name: &'static str,
    /// Expected size in bytes.
    pub size: usize,
    /// Offset within the target memory region where this ROM is loaded.
    pub offset: usize,
    /// Optional CRC32 checksum. `None` means no checksum is defined
    /// and the file is always accepted. `Some(value)` will be validated
    /// during [`RomRegion::load`].
    pub crc32: Option<u32>,
}

/// Describes the complete ROM mapping for a machine or subsystem.
///
/// A region has a total size and a list of ROM entries that fill parts of it.
/// Call [`load`](Self::load) to assemble the region from a [`RomSet`],
/// or [`load_skip_checksums`](Self::load_skip_checksums) to skip CRC32
/// validation.
pub struct RomRegion {
    /// Total size of the memory region in bytes.
    pub size: usize,
    /// Individual ROM file entries.
    pub entries: &'static [RomEntry],
}

impl RomRegion {
    /// Load all ROM files into a contiguous byte array, validating sizes
    /// and CRC32 checksums.
    pub fn load(&self, rom_set: &RomSet) -> Result<Vec<u8>, RomLoadError> {
        self.load_inner(rom_set, true)
    }

    /// Load all ROM files into a contiguous byte array, validating sizes
    /// only. CRC32 checksums are not checked.
    ///
    /// Useful for modified/hacked ROMs or development builds.
    pub fn load_skip_checksums(&self, rom_set: &RomSet) -> Result<Vec<u8>, RomLoadError> {
        self.load_inner(rom_set, false)
    }

    fn load_inner(
        &self,
        rom_set: &RomSet,
        verify_checksums: bool,
    ) -> Result<Vec<u8>, RomLoadError> {
        let mut region = vec![0u8; self.size];

        for entry in self.entries {
            debug_assert!(
                entry.offset + entry.size <= self.size,
                "RomEntry '{}' exceeds region bounds: offset {} + size {} > region size {}",
                entry.name,
                entry.offset,
                entry.size,
                self.size,
            );

            let data = rom_set.require_sized(entry.name, entry.size)?;

            if verify_checksums && let Some(expected_crc) = entry.crc32 {
                let actual_crc = crc32(data);
                if actual_crc != expected_crc {
                    return Err(RomLoadError::ChecksumMismatch {
                        file: entry.name.to_string(),
                        expected: expected_crc,
                        actual: actual_crc,
                    });
                }
            }

            region[entry.offset..entry.offset + entry.size].copy_from_slice(data);
        }

        Ok(region)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- CRC32 ---------------------------------------------------------------

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(&[]), 0x0000_0000);
    }

    #[test]
    fn crc32_canonical_123456789() {
        // Well-known test vector: CRC32("123456789") = 0xCBF43926
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_single_zero_byte() {
        // CRC32 of [0x00] = 0xD202EF8D
        assert_eq!(crc32(&[0x00]), 0xD202_EF8D);
    }

    #[test]
    fn crc32_deterministic() {
        let data = [0xFF; 256];
        let first = crc32(&data);
        let second = crc32(&data);
        assert_eq!(first, second);
        assert_ne!(first, 0);
    }

    // -- RomSet --------------------------------------------------------------

    #[test]
    fn from_slices_creates_romset() {
        let rom_set = RomSet::from_slices(&[
            ("test1.rom", &[0x01, 0x02, 0x03]),
            ("test2.rom", &[0x04, 0x05]),
        ]);
        assert_eq!(rom_set.get("test1.rom"), Some(&[0x01, 0x02, 0x03][..]));
        assert_eq!(rom_set.get("test2.rom"), Some(&[0x04, 0x05][..]));
    }

    #[test]
    fn get_missing_returns_none() {
        let rom_set = RomSet::from_slices(&[("a.rom", &[0x00])]);
        assert!(rom_set.get("missing.rom").is_none());
    }

    #[test]
    fn require_missing_returns_error() {
        let rom_set = RomSet::from_slices(&[]);
        let result = rom_set.require("missing.rom");
        assert!(matches!(result, Err(RomLoadError::MissingFile(_))));
    }

    #[test]
    fn require_sized_correct() {
        let rom_set = RomSet::from_slices(&[("test.rom", &[0u8; 64])]);
        assert!(rom_set.require_sized("test.rom", 64).is_ok());
    }

    #[test]
    fn require_sized_wrong_size() {
        let rom_set = RomSet::from_slices(&[("test.rom", &[0u8; 100])]);
        let result = rom_set.require_sized("test.rom", 64);
        assert!(matches!(result, Err(RomLoadError::SizeMismatch { .. })));
    }

    #[test]
    fn file_names_lists_all() {
        let rom_set = RomSet::from_slices(&[("alpha.rom", &[]), ("beta.rom", &[])]);
        let mut names = rom_set.file_names();
        names.sort();
        assert_eq!(names, vec!["alpha.rom", "beta.rom"]);
    }

    // -- RomRegion::load -----------------------------------------------------

    #[test]
    fn load_single_rom_no_checksum() {
        static ENTRIES: [RomEntry; 1] = [RomEntry {
            name: "test.rom",
            size: 4,
            offset: 0,
            crc32: None,
        }];
        let region = RomRegion {
            size: 4,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("test.rom", &[0xDE, 0xAD, 0xBE, 0xEF])]);
        let result = region.load(&rom_set).unwrap();
        assert_eq!(result, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn load_single_rom_valid_checksum() {
        let data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
        let checksum = crc32(data);

        let entries: &'static [RomEntry] = Box::leak(Box::new([RomEntry {
            name: "test.rom",
            size: 4,
            offset: 0,
            crc32: Some(checksum),
        }]));
        let region = RomRegion { size: 4, entries };
        let rom_set = RomSet::from_slices(&[("test.rom", data)]);
        let result = region.load(&rom_set).unwrap();
        assert_eq!(result, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn load_checksum_mismatch() {
        static ENTRIES: [RomEntry; 1] = [RomEntry {
            name: "test.rom",
            size: 4,
            offset: 0,
            crc32: Some(0xDEAD_BEEF), // wrong checksum
        }];
        let region = RomRegion {
            size: 4,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("test.rom", &[0x01, 0x02, 0x03, 0x04])]);
        let result = region.load(&rom_set);
        assert!(matches!(result, Err(RomLoadError::ChecksumMismatch { .. })));
    }

    #[test]
    fn load_skip_checksums_ignores_mismatch() {
        static ENTRIES: [RomEntry; 1] = [RomEntry {
            name: "test.rom",
            size: 4,
            offset: 0,
            crc32: Some(0xDEAD_BEEF), // wrong checksum
        }];
        let region = RomRegion {
            size: 4,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("test.rom", &[0x01, 0x02, 0x03, 0x04])]);
        let result = region.load_skip_checksums(&rom_set);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn load_size_mismatch_even_with_skip_checksums() {
        static ENTRIES: [RomEntry; 1] = [RomEntry {
            name: "test.rom",
            size: 8,
            offset: 0,
            crc32: None,
        }];
        let region = RomRegion {
            size: 8,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("test.rom", &[0u8; 4])]);
        let result = region.load_skip_checksums(&rom_set);
        assert!(matches!(result, Err(RomLoadError::SizeMismatch { .. })));
    }

    #[test]
    fn load_multiple_roms_at_offsets() {
        static ENTRIES: [RomEntry; 3] = [
            RomEntry {
                name: "rom1.bin",
                size: 8,
                offset: 0,
                crc32: None,
            },
            RomEntry {
                name: "rom2.bin",
                size: 8,
                offset: 8,
                crc32: None,
            },
            RomEntry {
                name: "rom3.bin",
                size: 8,
                offset: 16,
                crc32: None,
            },
        ];
        let region = RomRegion {
            size: 24,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[
            ("rom1.bin", &[0x11; 8]),
            ("rom2.bin", &[0x22; 8]),
            ("rom3.bin", &[0x33; 8]),
        ]);
        let loaded = region.load(&rom_set).unwrap();
        assert_eq!(loaded.len(), 24);
        assert!(loaded[..8].iter().all(|&b| b == 0x11));
        assert!(loaded[8..16].iter().all(|&b| b == 0x22));
        assert!(loaded[16..24].iter().all(|&b| b == 0x33));
    }

    #[test]
    fn load_missing_file_in_region() {
        static ENTRIES: [RomEntry; 2] = [
            RomEntry {
                name: "rom1.bin",
                size: 8,
                offset: 0,
                crc32: None,
            },
            RomEntry {
                name: "rom2.bin",
                size: 8,
                offset: 8,
                crc32: None,
            },
        ];
        let region = RomRegion {
            size: 16,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("rom1.bin", &[0u8; 8])]);
        let result = region.load(&rom_set);
        assert!(matches!(result, Err(RomLoadError::MissingFile(_))));
    }

    #[test]
    fn load_none_checksum_not_validated_even_with_verify() {
        // crc32: None means the file is always accepted, even with load()
        static ENTRIES: [RomEntry; 1] = [RomEntry {
            name: "test.rom",
            size: 4,
            offset: 0,
            crc32: None,
        }];
        let region = RomRegion {
            size: 4,
            entries: &ENTRIES,
        };
        let rom_set = RomSet::from_slices(&[("test.rom", &[0xFF; 4])]);
        assert!(region.load(&rom_set).is_ok());
    }

    // -- Directory loading ---------------------------------------------------

    #[test]
    fn from_directory_loads_files() {
        let dir = std::env::temp_dir().join("phosphor_rom_loader_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.rom"), [0xAA, 0xBB]).unwrap();

        let rom_set = RomSet::from_directory(&dir).unwrap();
        assert_eq!(rom_set.get("test.rom"), Some(&[0xAA, 0xBB][..]));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
