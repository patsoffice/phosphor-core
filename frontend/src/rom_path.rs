//! ROM path resolution: loads a [`RomSet`] from a MAME-style rompath,
//! a direct ZIP file, or a directory of loose ROM files.

use phosphor_machines::rom_loader::{RomLoadError, RomSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Resolve a ROM path and load all ROM files into a [`RomSet`].
///
/// Resolution order:
/// 1. If `path` ends with `.zip` → load directly as a ZIP archive.
/// 2. If `path` is a directory containing `{machine_name}.zip` → load that ZIP.
/// 3. If `path` is a directory of loose files → load via [`RomSet::from_directory`].
pub fn load_rom_set(machine_name: &str, path: &str) -> Result<RomSet, RomLoadError> {
    let path = Path::new(path);

    // Direct ZIP file
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return load_from_zip(path);
    }

    // MAME-style rompath: directory containing {machine}.zip
    if path.is_dir() {
        let zip_path = path.join(format!("{machine_name}.zip"));
        if zip_path.exists() {
            return load_from_zip(&zip_path);
        }

        // Fallback: directory of loose ROM files
        return RomSet::from_directory(path);
    }

    Err(RomLoadError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("ROM path not found: {}", path.display()),
    )))
}

/// Extract all files from a ZIP archive into a [`RomSet`].
fn load_from_zip(path: &Path) -> Result<RomSet, RomLoadError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("invalid ZIP: {e}"))
    })?;

    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ZIP entry error: {e}"),
            )
        })?;

        // Skip directories
        if entry.is_dir() {
            continue;
        }

        let name = entry.name().to_string();
        let mut data = Vec::with_capacity(entry.size() as usize);
        std::io::Read::read_to_end(&mut entry, &mut data)?;
        entries.push((name, data));
    }

    Ok(RomSet::from_entries(entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_test_zip(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let zip_path = dir.join(name);
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (fname, data) in files {
            zip.start_file(*fname, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        zip_path
    }

    #[test]
    fn resolve_zip_file_directly() {
        let dir = std::env::temp_dir().join("phosphor_rompath_test_zip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let zip_path = create_test_zip(&dir, "joust.zip", &[("rom.bin", &[0xAA; 16])]);

        let rom_set = load_rom_set("joust", zip_path.to_str().unwrap()).unwrap();
        assert_eq!(rom_set.get("rom.bin"), Some(&[0xAA; 16][..]));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_zip_from_rompath_directory() {
        let dir = std::env::temp_dir().join("phosphor_rompath_test_dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        create_test_zip(&dir, "joust.zip", &[("rom.bin", &[0xBB; 8])]);

        let rom_set = load_rom_set("joust", dir.to_str().unwrap()).unwrap();
        assert_eq!(rom_set.get("rom.bin"), Some(&[0xBB; 8][..]));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_loose_directory_fallback() {
        let dir = std::env::temp_dir().join("phosphor_rompath_test_loose");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("test.rom"), [0xCC; 4]).unwrap();

        let rom_set = load_rom_set("joust", dir.to_str().unwrap()).unwrap();
        assert_eq!(rom_set.get("test.rom"), Some(&[0xCC; 4][..]));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
