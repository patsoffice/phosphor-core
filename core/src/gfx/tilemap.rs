use super::decode::GfxCache;

/// Configuration describing a tilemap's dimensions.
pub struct TilemapConfig {
    /// Number of tile columns.
    pub cols: usize,
    /// Number of tile rows.
    pub rows: usize,
    /// Tile width in pixels (typically 8).
    pub tile_width: usize,
    /// Tile height in pixels (typically 8).
    pub tile_height: usize,
}

/// Render one scanline of a tilemap into an RGB24 buffer.
///
/// For each tile column that intersects the given scanline, calls
/// `tile_info_fn(col, row)` to get the tile code and color attribute, then
/// reads the pre-decoded pixel from `tiles` and calls
/// `resolve_color_fn(attribute, pixel_value)` to produce the final RGB triple.
///
/// The result is written into `buffer` starting at byte offset
/// `x_offset * 3`. The buffer must be large enough for the full tile row
/// width; callers typically pass a slice already offset to the correct
/// scanline row in their framebuffer.
pub fn render_tilemap_scanline<F, G>(
    config: &TilemapConfig,
    tiles: &GfxCache,
    scanline: usize,
    tile_info_fn: F,
    resolve_color_fn: G,
    buffer: &mut [u8],
    x_offset: usize,
) where
    F: Fn(usize, usize) -> (u16, u8),
    G: Fn(u8, u8) -> (u8, u8, u8),
{
    let tile_row = scanline / config.tile_height;
    let py = scanline % config.tile_height;

    for tile_col in 0..config.cols {
        let (tile_code, attribute) = tile_info_fn(tile_col, tile_row);
        let screen_x = x_offset + tile_col * config.tile_width;

        for px in 0..config.tile_width {
            let pixel_value = tiles.pixel(tile_code as usize, px, py);
            let (r, g, b) = resolve_color_fn(attribute, pixel_value);
            let off = (screen_x + px) * 3;
            buffer[off] = r;
            buffer[off + 1] = g;
            buffer[off + 2] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_single_tile_scanline() {
        // 2x1 tilemap, 4x2 tiles, scanline 0
        let config = TilemapConfig {
            cols: 2,
            rows: 1,
            tile_width: 4,
            tile_height: 2,
        };

        // Build a cache with 2 tiles (4x2 each)
        let mut cache = GfxCache::new(2, 4, 2);
        // Tile 0: all pixels = 1
        for py in 0..2 {
            for px in 0..4 {
                cache.set_pixel(0, px, py, 1);
            }
        }
        // Tile 1: all pixels = 2
        for py in 0..2 {
            for px in 0..4 {
                cache.set_pixel(1, px, py, 2);
            }
        }

        let mut buffer = vec![0u8; 8 * 3]; // 8 pixels wide

        render_tilemap_scanline(
            &config,
            &cache,
            0,                                       // scanline 0
            |col, _row| (col as u16, col as u8),     // tile 0 at col 0, tile 1 at col 1
            |_attr, pv| (pv * 80, pv * 80, pv * 80), // simple grayscale
            &mut buffer,
            0,
        );

        // First 4 pixels should be tile 0 (pixel value 1 -> RGB 80,80,80)
        for px in 0..4 {
            assert_eq!(buffer[px * 3], 80);
            assert_eq!(buffer[px * 3 + 1], 80);
            assert_eq!(buffer[px * 3 + 2], 80);
        }
        // Next 4 pixels should be tile 1 (pixel value 2 -> RGB 160,160,160)
        for px in 4..8 {
            assert_eq!(buffer[px * 3], 160);
            assert_eq!(buffer[px * 3 + 1], 160);
            assert_eq!(buffer[px * 3 + 2], 160);
        }
    }
}
