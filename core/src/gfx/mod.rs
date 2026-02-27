pub mod decode;
pub mod sprite;
pub mod tilemap;

pub use decode::GfxCache;
pub use tilemap::TilemapConfig;

/// Rotate an RGB24 buffer 90° counter-clockwise.
///
/// Transforms a `src_w × src_h` image into a `src_h × src_w` output.
/// Native pixel `(nx, ny)` maps to output pixel `(src_h - 1 - ny, nx)`.
pub fn rotate_90_ccw(src: &[u8], dst: &mut [u8], src_w: usize, src_h: usize) {
    let dst_w = src_h;
    for ny in 0..src_h {
        for nx in 0..src_w {
            let ox = (src_h - 1) - ny;
            let oy = nx;
            let si = (ny * src_w + nx) * 3;
            let di = (oy * dst_w + ox) * 3;
            dst[di] = src[si];
            dst[di + 1] = src[si + 1];
            dst[di + 2] = src[si + 2];
        }
    }
}

/// Rotate an indexed pixel buffer 90° counter-clockwise, applying an RGB palette.
///
/// Performs the same rotation as `rotate_90_ccw` but converts indexed pixels
/// to RGB24 in a single pass. Each source byte is used as an index into
/// `palette` (masked to `palette.len() - 1`).
pub fn rotate_90_ccw_indexed(
    src: &[u8],
    dst: &mut [u8],
    src_w: usize,
    src_h: usize,
    palette: &[(u8, u8, u8)],
) {
    let dst_w = src_h;
    let mask = palette.len() - 1;
    for ny in 0..src_h {
        let ox = (src_h - 1) - ny;
        for nx in 0..src_w {
            let oy = nx;
            let idx = src[ny * src_w + nx] as usize & mask;
            let (r, g, b) = palette[idx];
            let di = (oy * dst_w + ox) * 3;
            dst[di] = r;
            dst[di + 1] = g;
            dst[di + 2] = b;
        }
    }
}
