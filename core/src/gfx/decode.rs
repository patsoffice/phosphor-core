//! Pre-decoded graphics cache for tile and sprite pixel data.
//!
//! ROM graphics data comes in many different planar layouts across arcade
//! hardware. Rather than parameterising these layouts at render time, we
//! decode once at ROM load into a uniform `[code][py][px] -> palette_index`
//! representation. Each game defines a [`GfxLayout`] describing its ROM
//! format; the generic [`decode_gfx`] function reads bits at the positions
//! specified by the layout and assembles pixel values. The scanline renderer
//! then uses the resulting cache via a simple array lookup.

/// Pre-decoded tile or sprite pixel cache.
///
/// Pixels are stored as palette indices in a flat array indexed by
/// `code * height * width + py * width + px`. Each element is an N-bit
/// value (0-3 for 2bpp, 0-7 for 3bpp, etc.).
pub struct GfxCache {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    count: usize,
    stride: usize, // width * height, cached for fast indexing
}

impl GfxCache {
    /// Create an empty cache with the given element dimensions.
    pub fn new(count: usize, width: usize, height: usize) -> Self {
        let stride = width * height;
        Self {
            pixels: vec![0; count * stride],
            width,
            height,
            count,
            stride,
        }
    }

    /// Look up a single pixel value.
    #[inline]
    pub fn pixel(&self, code: usize, px: usize, py: usize) -> u8 {
        self.pixels[code * self.stride + py * self.width + px]
    }

    /// Return a full row of pixel values for a given code and row.
    ///
    /// The returned slice has `width` elements, one per column. This avoids
    /// repeated per-pixel index arithmetic when the caller needs every pixel
    /// in a row (e.g. 2× upscale loops).
    #[inline]
    pub fn row_slice(&self, code: usize, py: usize) -> &[u8] {
        let start = code * self.stride + py * self.width;
        &self.pixels[start..start + self.width]
    }

    /// Set a single pixel value (used during decode).
    #[inline]
    pub fn set_pixel(&mut self, code: usize, px: usize, py: usize, value: u8) {
        self.pixels[code * self.stride + py * self.width + px] = value;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

// ---------------------------------------------------------------------------
// MAME-style GfxLayout descriptor and generic decoder
// ---------------------------------------------------------------------------

/// Describes a ROM graphics layout using MAME-style bit offsets.
///
/// Each pixel at position `(px, py)` in element `code` is assembled from
/// `plane_offsets.len()` bitplanes. The bit position for plane `p` is:
///
/// ```text
/// bit_pos = base*8 + code * char_increment
///           + plane_offsets[p] + x_offsets[px] + y_offsets[py]
/// ```
///
/// Bits are read MSB-first (matching MAME `readbit`):
/// `byte = bit_pos / 8`, `bit = 7 - (bit_pos % 8)`.
///
/// Plane 0 maps to pixel bit 0 (LSB), plane 1 to bit 1, etc.
pub struct GfxLayout<'a> {
    pub plane_offsets: &'a [usize],
    pub x_offsets: &'a [usize],
    pub y_offsets: &'a [usize],
    pub char_increment: usize,
}

/// Decode ROM graphics into a [`GfxCache`] using a [`GfxLayout`].
///
/// `rom` is the full graphics ROM region. `base` is the byte offset of the
/// first element. `count` is the number of elements to decode.
pub fn decode_gfx(rom: &[u8], base: usize, count: usize, layout: &GfxLayout) -> GfxCache {
    let width = layout.x_offsets.len();
    let height = layout.y_offsets.len();
    let mut cache = GfxCache::new(count, width, height);
    let base_bits = base * 8;

    for code in 0..count {
        let code_bits = base_bits + code * layout.char_increment;
        for (py, &y_off) in layout.y_offsets.iter().enumerate() {
            for (px, &x_off) in layout.x_offsets.iter().enumerate() {
                let mut pixel: u8 = 0;
                let xy_bits = x_off + y_off;
                for (p, &plane_off) in layout.plane_offsets.iter().enumerate() {
                    let bit_pos = code_bits + plane_off + xy_bits;
                    let byte_idx = bit_pos / 8;
                    if byte_idx < rom.len() {
                        pixel |= ((rom[byte_idx] >> (7 - (bit_pos & 7))) & 1) << p;
                    }
                }
                cache.set_pixel(code, px, py, pixel);
            }
        }
    }
    cache
}

/// Re-decode a single element into an existing [`GfxCache`].
///
/// Updates the pixels for `code` in-place. Useful for runtime character RAM
/// updates (e.g. Gottlieb charram writes).
pub fn decode_gfx_element(
    rom: &[u8],
    base: usize,
    code: usize,
    layout: &GfxLayout,
    cache: &mut GfxCache,
) {
    let code_bits = base * 8 + code * layout.char_increment;
    for (py, &y_off) in layout.y_offsets.iter().enumerate() {
        for (px, &x_off) in layout.x_offsets.iter().enumerate() {
            let mut pixel: u8 = 0;
            let xy_bits = x_off + y_off;
            for (p, &plane_off) in layout.plane_offsets.iter().enumerate() {
                let bit_pos = code_bits + plane_off + xy_bits;
                let byte_idx = bit_pos / 8;
                if byte_idx < rom.len() {
                    pixel |= ((rom[byte_idx] >> (7 - (bit_pos & 7))) & 1) << p;
                }
            }
            cache.set_pixel(code, px, py, pixel);
        }
    }
}

// ---------------------------------------------------------------------------
// Gottlieb System 80 (GG-III) ROM layouts
// ---------------------------------------------------------------------------

/// Decode Gottlieb tiles: 8x8, 4bpp packed MSB.
///
/// 32 bytes per tile. Each byte stores two 4-bit pixels: high nibble = left
/// pixel (even column), low nibble = right pixel (odd column). Row stride
/// is 4 bytes (8 pixels / 2 pixels per byte).
pub fn decode_gottlieb_tiles(rom: &[u8], base: usize, count: usize) -> GfxCache {
    let mut cache = GfxCache::new(count, 8, 8);
    for code in 0..count {
        decode_gottlieb_tile_into(&mut cache, code, &rom[base + code * 32..]);
    }
    cache
}

/// Decode a single 8x8 4bpp packed-MSB tile into an existing cache.
///
/// Used for runtime charram re-decode when character generator RAM is written.
/// `data` must be at least 32 bytes.
pub fn decode_gottlieb_tile_into(cache: &mut GfxCache, code: usize, data: &[u8]) {
    for py in 0..8usize {
        for px in 0..8usize {
            let byte = data[py * 4 + px / 2];
            let pixel = if px & 1 == 0 {
                (byte >> 4) & 0x0F
            } else {
                byte & 0x0F
            };
            cache.set_pixel(code, px, py, pixel);
        }
    }
}

/// Decode Gottlieb sprites: 16x16, 4bpp planar.
///
/// ROM data is divided into 4 equal regions, each storing one bitplane.
/// Within each region: 32 bytes per sprite (16 rows × 2 bytes), MSB-first
/// (bit 7 of first byte = leftmost pixel).
pub fn decode_gottlieb_sprites(rom: &[u8], count: usize) -> GfxCache {
    let quarter = rom.len() / 4;
    let mut cache = GfxCache::new(count, 16, 16);
    for code in 0..count {
        for py in 0..16usize {
            for px in 0..16usize {
                let byte_off = code * 32 + py * 2 + px / 8;
                let bit = 7 - (px & 7);
                let mut pixel = 0u8;
                for plane in 0..4 {
                    pixel |= ((rom[quarter * plane + byte_off] >> bit) & 1) << plane;
                }
                cache.set_pixel(code, px, py, pixel);
            }
        }
    }
    cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gfx_cache_basic() {
        let mut cache = GfxCache::new(2, 8, 8);
        assert_eq!(cache.count(), 2);
        assert_eq!(cache.width(), 8);
        assert_eq!(cache.height(), 8);
        assert_eq!(cache.pixel(0, 0, 0), 0);

        cache.set_pixel(0, 3, 5, 2);
        assert_eq!(cache.pixel(0, 3, 5), 2);
        assert_eq!(cache.pixel(1, 3, 5), 0); // different code, still 0
    }

    // -- Gottlieb tile/sprite decode tests --

    #[test]
    fn decode_gottlieb_tiles_packed_msb() {
        // 1 tile, 32 bytes. High nibble = left pixel, low nibble = right pixel.
        let mut rom = [0u8; 32];
        rom[0] = 0xAB; // row 0: px=0 → 0xA, px=1 → 0xB
        rom[1] = 0xCD; // row 0: px=2 → 0xC, px=3 → 0xD

        let cache = decode_gottlieb_tiles(&rom, 0, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0x0A);
        assert_eq!(cache.pixel(0, 1, 0), 0x0B);
        assert_eq!(cache.pixel(0, 2, 0), 0x0C);
        assert_eq!(cache.pixel(0, 3, 0), 0x0D);
    }

    #[test]
    fn decode_gottlieb_tiles_row_stride() {
        // Row 1 starts at byte offset 4.
        let mut rom = [0u8; 32];
        rom[4] = 0x12; // row 1: px=0 → 1, px=1 → 2

        let cache = decode_gottlieb_tiles(&rom, 0, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0); // row 0
        assert_eq!(cache.pixel(0, 0, 1), 1); // row 1
        assert_eq!(cache.pixel(0, 1, 1), 2);
    }

    #[test]
    fn decode_gottlieb_tile_into_charram() {
        let mut cache = GfxCache::new(2, 8, 8);
        let mut data = [0u8; 32];
        data[0] = 0x12;

        decode_gottlieb_tile_into(&mut cache, 1, &data);
        assert_eq!(cache.pixel(1, 0, 0), 0x01);
        assert_eq!(cache.pixel(1, 1, 0), 0x02);
        // Code 0 is untouched
        assert_eq!(cache.pixel(0, 0, 0), 0x00);
    }

    #[test]
    fn decode_gottlieb_sprites_planar() {
        // 1 sprite, 4 planes. Each quarter = 32 bytes.
        let mut rom = vec![0u8; 128];
        // Plane 0: bit 7 set → px=0 has plane 0
        rom[0] = 0x80;
        // Plane 1: bit 7 set → px=0 has plane 1
        rom[32] = 0x80;

        let cache = decode_gottlieb_sprites(&rom, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0x03); // plane 0 + plane 1
        assert_eq!(cache.pixel(0, 1, 0), 0x00);
    }

    #[test]
    fn decode_gottlieb_sprites_all_planes() {
        let mut rom = vec![0u8; 128];
        rom[0] = 0x80; // plane 0
        rom[32] = 0x80; // plane 1
        rom[64] = 0x80; // plane 2
        rom[96] = 0x80; // plane 3

        let cache = decode_gottlieb_sprites(&rom, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0x0F);
    }

    #[test]
    fn decode_gottlieb_sprites_second_byte() {
        // px=8 reads from the second byte (byte offset 1) within each plane.
        let mut rom = vec![0u8; 128];
        rom[1] = 0x80; // plane 0, byte 1, bit 7 → px=8

        let cache = decode_gottlieb_sprites(&rom, 1);
        assert_eq!(cache.pixel(0, 8, 0), 0x01); // only plane 0
        assert_eq!(cache.pixel(0, 0, 0), 0x00); // byte 0 is clear
    }

    // -----------------------------------------------------------------------
    // GfxLayout equivalence tests — verify generic decoder matches each old
    // decoder pixel-for-pixel on pseudo-random ROM data.
    // -----------------------------------------------------------------------

    /// Fill a ROM buffer with deterministic pseudo-random data.
    fn fill_prng(rom: &mut [u8]) {
        for (i, b) in rom.iter_mut().enumerate() {
            *b = (i.wrapping_mul(0x9E37_79B9) >> 24) as u8;
        }
    }

    /// Assert two caches are pixel-identical.
    fn assert_caches_equal(old: &GfxCache, new: &GfxCache, label: &str) {
        assert_eq!(old.count(), new.count(), "{label}: count mismatch");
        assert_eq!(old.width(), new.width(), "{label}: width mismatch");
        assert_eq!(old.height(), new.height(), "{label}: height mismatch");
        for code in 0..old.count() {
            for py in 0..old.height() {
                for px in 0..old.width() {
                    assert_eq!(
                        old.pixel(code, px, py),
                        new.pixel(code, px, py),
                        "{label}: mismatch at code={code}, px={px}, py={py}"
                    );
                }
            }
        }
    }

    #[test]
    fn generic_matches_gottlieb_tiles() {
        let mut rom = vec![0u8; 64 * 32];
        fill_prng(&mut rom);
        let old = decode_gottlieb_tiles(&rom, 0, 64);
        let new = decode_gfx(&rom, 0, 64, &GfxLayout {
            plane_offsets: &[3, 2, 1, 0],
            x_offsets: &[0, 4, 8, 12, 16, 20, 24, 28],
            y_offsets: &[0, 32, 64, 96, 128, 160, 192, 224],
            char_increment: 256,
        });
        assert_caches_equal(&old, &new, "gottlieb_tiles");
    }

    #[test]
    fn generic_matches_gottlieb_sprites() {
        let mut rom = vec![0u8; 4 * 32 * 16]; // 16 sprites
        fill_prng(&mut rom);
        let count = rom.len() / 128;
        let quarter = rom.len() / 4;
        let old = decode_gottlieb_sprites(&rom, count);
        let planes: [usize; 4] = std::array::from_fn(|p| p * quarter * 8);
        let y_offsets: [usize; 16] = std::array::from_fn(|py| py * 16);
        let new = decode_gfx(&rom, 0, count, &GfxLayout {
            plane_offsets: &planes,
            x_offsets: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            y_offsets: &y_offsets,
            char_increment: 256,
        });
        assert_caches_equal(&old, &new, "gottlieb_sprites");
    }

    #[test]
    fn generic_element_matches_gottlieb_tile_into() {
        let mut rom = vec![0u8; 4 * 32];
        fill_prng(&mut rom);
        // Old way: decode one tile into code slot 2
        let mut old_cache = GfxCache::new(4, 8, 8);
        decode_gottlieb_tile_into(&mut old_cache, 2, &rom[2 * 32..]);
        // New way: decode_gfx_element
        let layout = GfxLayout {
            plane_offsets: &[3, 2, 1, 0],
            x_offsets: &[0, 4, 8, 12, 16, 20, 24, 28],
            y_offsets: &[0, 32, 64, 96, 128, 160, 192, 224],
            char_increment: 256,
        };
        let mut new_cache = GfxCache::new(4, 8, 8);
        decode_gfx_element(&rom, 0, 2, &layout, &mut new_cache);
        for py in 0..8 {
            for px in 0..8 {
                assert_eq!(
                    old_cache.pixel(2, px, py),
                    new_cache.pixel(2, px, py),
                    "gottlieb_tile_into: mismatch at px={px}, py={py}"
                );
            }
        }
        // Code 0 should be untouched (still zero)
        assert_eq!(new_cache.pixel(0, 0, 0), 0);
    }

}
