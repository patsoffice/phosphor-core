use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Save an RGB24 framebuffer as a PNG file.
///
/// Returns the path of the written file. The filename is generated from
/// `prefix` (typically the machine name) and the current timestamp.
pub fn save_screenshot(
    rgb24: &[u8],
    width: u32,
    height: u32,
    dir: &Path,
    prefix: &str,
) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert epoch seconds to a readable YYYYMMDD_HHMMSS string.
    let secs = timestamp;
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let (year, month, day) = epoch_days_to_ymd(days);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    let filename =
        format!("{prefix}_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}.png");
    let path = dir.join(&filename);

    let file = File::create(&path)?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);

    let mut png_writer = encoder.write_header().map_err(io::Error::other)?;
    png_writer
        .write_image_data(rgb24)
        .map_err(io::Error::other)?;

    Ok(path)
}

/// Convert days since Unix epoch to (year, month, day).
fn epoch_days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Civil calendar algorithm (Euclidean affine from Howard Hinnant).
    days += 719_468;
    let era = days / 146_097;
    let doe = days % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
