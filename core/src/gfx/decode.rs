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
}
