//! Pre-decoded graphics cache for tile and sprite pixel data.
//!
//! ROM graphics data comes in many different planar layouts across arcade
//! hardware. Rather than parameterising these layouts at render time, we
//! decode once at ROM load into a uniform `[code][py][px] -> palette_index`
//! representation. Each game provides a decode function matching its ROM
//! format; the scanline renderer then uses the resulting cache via a simple
//! array lookup.

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
// Pac-Man / Pengo family ROM layouts
// ---------------------------------------------------------------------------

/// Decode Pac-Man style tiles: 8x8, 2bpp.
///
/// ROM layout (planeoffset {0, 4}, MSB-first):
///   16 bytes per tile. Within each byte, bits 7-4 are plane 0 (high bit
///   of pixel) and bits 3-0 are plane 1 (low bit). Pixel X mapping is
///   non-sequential: px 0-3 come from byte offset +8, px 4-7 from offset +0.
///   Y offset is simply row * 1 byte.
pub fn decode_pacman_tiles(rom: &[u8], base: usize, count: usize) -> GfxCache {
    let mut cache = GfxCache::new(count, 8, 8);
    for code in 0..count {
        let tile_base = base + code * 16;
        for py in 0..8usize {
            for px in 0..8usize {
                let (byte_off, bit) = if px < 4 {
                    (8, px) // px 0-3 from second half
                } else {
                    (0, px - 4) // px 4-7 from first half
                };
                let byte_addr = tile_base + byte_off + py;
                if byte_addr >= rom.len() {
                    continue;
                }
                let byte = rom[byte_addr];
                let plane_hi = (byte >> (7 - bit)) & 1;
                let plane_lo = (byte >> (3 - bit)) & 1;
                cache.set_pixel(code, px, py, plane_lo | (plane_hi << 1));
            }
        }
    }
    cache
}

/// Decode Pac-Man style sprites: 16x16, 2bpp.
///
/// Same plane interleaving as tiles ({0, 4} within each byte). 64 bytes per
/// sprite. X mapping uses 4 groups of 4 pixels at byte offsets [8, 16, 24, 0].
/// Y mapping splits at row 8: rows 0-7 at offset +0, rows 8-15 at offset +32.
pub fn decode_pacman_sprites(rom: &[u8], base: usize, count: usize) -> GfxCache {
    let mut cache = GfxCache::new(count, 16, 16);
    for code in 0..count {
        let spr_base = base + code * 64;
        for py in 0..16usize {
            for px in 0..16usize {
                let (x_byte_off, bit) = match px {
                    0..=3 => (8, px),
                    4..=7 => (16, px - 4),
                    8..=11 => (24, px - 8),
                    12..=15 => (0, px - 12),
                    _ => unreachable!(),
                };
                let y_byte_off = if py < 8 { py } else { 32 + (py - 8) };
                let byte_addr = spr_base + x_byte_off + y_byte_off;
                if byte_addr >= rom.len() {
                    continue;
                }
                let byte = rom[byte_addr];
                let plane_hi = (byte >> (7 - bit)) & 1;
                let plane_lo = (byte >> (3 - bit)) & 1;
                cache.set_pixel(code, px, py, plane_lo | (plane_hi << 1));
            }
        }
    }
    cache
}

// ---------------------------------------------------------------------------
// Donkey Kong / TKG-04 family ROM layouts
// ---------------------------------------------------------------------------

/// Decode separated-plane 2bpp tiles: 8x8.
///
/// Plane 0 is at `base`, plane 1 is at `base + plane1_offset`. Each plane
/// stores 8 bytes per tile (one byte per row), MSB-first (bit 7 = leftmost
/// pixel). 8 bytes per tile per plane.
pub fn decode_planar_2bpp_tiles(
    rom: &[u8],
    base: usize,
    plane1_offset: usize,
    count: usize,
) -> GfxCache {
    let mut cache = GfxCache::new(count, 8, 8);
    for code in 0..count {
        let tile_offset = base + code * 8;
        for py in 0..8usize {
            let addr0 = tile_offset + py;
            let addr1 = tile_offset + plane1_offset + py;
            let plane0 = if addr0 < rom.len() { rom[addr0] } else { 0 };
            let plane1 = if addr1 < rom.len() { rom[addr1] } else { 0 };
            for px in 0..8usize {
                let bit_mask = 0x80 >> px;
                let p0 = u8::from(plane0 & bit_mask != 0);
                let p1 = u8::from(plane1 & bit_mask != 0);
                cache.set_pixel(code, px, py, p0 | (p1 << 1));
            }
        }
    }
    cache
}

/// Decode Donkey Kong family sprites: 16x16, 2bpp, 4-ROM interleaved.
///
/// 4 ROM regions of 2KB each store left/right halves of planes 0/1:
///   - `base + 0x0000`: plane 0, left half (px 0-7)
///   - `base + 0x0800`: plane 0, right half (px 8-15)
///   - `base + 0x1000`: plane 1, left half (px 0-7)
///   - `base + 0x1800`: plane 1, right half (px 8-15)
///
/// Within each region: 16 bytes per sprite (one byte per row), MSB-first.
pub fn decode_dkong_sprites(rom: &[u8], base: usize, count: usize) -> GfxCache {
    let mut cache = GfxCache::new(count, 16, 16);
    for code in 0..count {
        let spr_offset = base + code * 16;
        for py in 0..16usize {
            for px in 0..16usize {
                let (p0_base, p1_base) = if px < 8 {
                    (spr_offset, 0x1000 + spr_offset)
                } else {
                    (0x0800 + spr_offset, 0x1800 + spr_offset)
                };
                let addr0 = p0_base + py;
                let addr1 = p1_base + py;
                let byte0 = if addr0 < rom.len() { rom[addr0] } else { 0 };
                let byte1 = if addr1 < rom.len() { rom[addr1] } else { 0 };
                let bit_mask = 0x80 >> (px & 7);
                let p0 = u8::from(byte0 & bit_mask != 0);
                let p1 = u8::from(byte1 & bit_mask != 0);
                cache.set_pixel(code, px, py, p0 | (p1 << 1));
            }
        }
    }
    cache
}

// ---------------------------------------------------------------------------
// Midway MCR family ROM layouts
// ---------------------------------------------------------------------------

/// Decode MCR tiles: 8x8, 4bpp.
///
/// MAME layout `mcr_bg_layout`: ROM is split in two halves. Each half
/// stores two interleaved bitplanes in 2-byte words (16 bytes per tile).
///
/// Plane mapping (index 0 = MSB of pixel):
///   - Planes 0,1 from second half of ROM
///   - Planes 2,3 from first half of ROM
///
/// Within each half: pixel bits are packed at 2-bit intervals across
/// each 2-byte row word (8 pixels per row, 2 bits per pixel per half).
pub fn decode_mcr_tiles(rom: &[u8], count: usize) -> GfxCache {
    let half = rom.len() / 2;
    let mut cache = GfxCache::new(count, 8, 8);
    for code in 0..count {
        for py in 0..8usize {
            for px in 0..8usize {
                let byte_off = code * 16 + py * 2 + px / 4;
                let local_px = px % 4;

                let lo_byte = rom[byte_off]; // first half
                let hi_byte = rom[half + byte_off]; // second half

                // MSB-first bit ordering (matching MAME readbit: 0x80 >> bitnum)
                // Plane ordering: planeoffs {half, half+1, 0, 1}
                // Plane 0,1 from second half; Plane 2,3 from first half
                let p0 = (hi_byte >> (7 - local_px * 2)) & 1; // plane 0 → pixel bit 3
                let p1 = (hi_byte >> (6 - local_px * 2)) & 1; // plane 1 → pixel bit 2
                let p2 = (lo_byte >> (7 - local_px * 2)) & 1; // plane 2 → pixel bit 1
                let p3 = (lo_byte >> (6 - local_px * 2)) & 1; // plane 3 → pixel bit 0

                cache.set_pixel(code, px, py, (p0 << 3) | (p1 << 2) | (p2 << 1) | p3);
            }
        }
    }
    cache
}

/// Decode MCR sprites: 32x32, 4bpp.
///
/// MAME layout `mcr_sprite_layout`: 4 ROM chips concatenated into one
/// region (each chip is one quarter). Plane offsets {0,1,2,3} are packed
/// into nibbles. Each sprite uses 128 bytes per ROM chip (4 bytes/row × 32 rows).
///
/// X columns are distributed across the 4 ROMs in pairs:
///   - Columns 0-1,8-9,16-17,24-25 from ROM 0
///   - Columns 2-3,10-11,18-19,26-27 from ROM 1
///   - Columns 4-5,12-13,20-21,28-29 from ROM 2
///   - Columns 6-7,14-15,22-23,30-31 from ROM 3
///
/// Within each ROM, each row occupies 4 bytes. Each byte holds two pixels
/// (low nibble = even column, high nibble = odd column), with nibble bits
/// reversed (bit 0 of nibble = MSB of pixel value).
pub fn decode_mcr_sprites(rom: &[u8], count: usize) -> GfxCache {
    let quarter = rom.len() / 4;
    let mut cache = GfxCache::new(count, 32, 32);
    for code in 0..count {
        for py in 0..32usize {
            for px in 0..32usize {
                let rom_idx = (px / 2) % 4;
                let group = px / 8;
                let sub_pixel = px % 2;

                let rom_offset = rom_idx * quarter;
                let byte_off = code * 128 + py * 4 + group;
                let byte = rom[rom_offset + byte_off];

                // MSB-first: even px = high nibble, odd px = low nibble.
                // Plane bits map directly to pixel bits (no reversal).
                let pixel = (byte >> ((1 - sub_pixel) * 4)) & 0x0F;

                cache.set_pixel(code, px, py, pixel);
            }
        }
    }
    cache
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

    #[test]
    fn decode_pacman_tiles_known_pattern() {
        // Construct a minimal 1-tile ROM with a known bit pattern.
        // Tile layout: 16 bytes per tile.
        // Byte at offset 8 (for px 0-3, py=0): test value 0xA5 = 10100101
        //   px=0 (bit=0): hi=(byte>>7)&1=1, lo=(byte>>3)&1=0 -> pixel=2
        //   px=1 (bit=1): hi=(byte>>6)&1=0, lo=(byte>>2)&1=1 -> pixel=1
        //   px=2 (bit=2): hi=(byte>>5)&1=1, lo=(byte>>1)&1=0 -> pixel=2
        //   px=3 (bit=3): hi=(byte>>4)&1=0, lo=(byte>>0)&1=1 -> pixel=1
        let mut rom = [0u8; 16];
        rom[8] = 0xA5; // py=0, px 0-3

        let cache = decode_pacman_tiles(&rom, 0, 1);
        assert_eq!(cache.pixel(0, 0, 0), 2); // px=0, py=0
        assert_eq!(cache.pixel(0, 1, 0), 1); // px=1, py=0
        assert_eq!(cache.pixel(0, 2, 0), 2); // px=2, py=0
        assert_eq!(cache.pixel(0, 3, 0), 1); // px=3, py=0
    }

    #[test]
    fn decode_planar_2bpp_tiles_known_pattern() {
        // 1 tile, plane1 offset = 8.
        // Plane 0, row 0: 0b11000000 -> px0=1, px1=1, rest 0
        // Plane 1, row 0: 0b10000000 -> px0=1, rest 0
        // pixel = p0 | (p1 << 1): px0 = 1|2 = 3, px1 = 1|0 = 1
        let mut rom = [0u8; 16];
        rom[0] = 0xC0; // plane 0, row 0
        rom[8] = 0x80; // plane 1, row 0

        let cache = decode_planar_2bpp_tiles(&rom, 0, 8, 1);
        assert_eq!(cache.pixel(0, 0, 0), 3); // px=0, py=0
        assert_eq!(cache.pixel(0, 1, 0), 1); // px=1, py=0
        assert_eq!(cache.pixel(0, 2, 0), 0); // px=2, py=0
    }

    #[test]
    fn decode_dkong_sprites_known_pattern() {
        // 1 sprite. Plane 0 left (offset 0), plane 1 left (offset 0x1000).
        // Sprite code 0, row 0, left half.
        // Plane 0 byte: 0xFF -> all 8 left pixels have p0=1
        // Plane 1 byte: 0x00 -> all 8 left pixels have p1=0
        // pixel = 1 | 0 = 1 for all left pixels
        let mut rom = vec![0u8; 0x2000];
        rom[0] = 0xFF; // plane 0, code 0, row 0
        // plane 1 at 0x1000 stays 0

        let cache = decode_dkong_sprites(&rom, 0, 1);
        for px in 0..8 {
            assert_eq!(cache.pixel(0, px, 0), 1, "px={px}");
        }
        for px in 8..16 {
            assert_eq!(cache.pixel(0, px, 0), 0, "px={px}");
        }
    }

    // -- MCR tile/sprite decode tests --

    #[test]
    fn decode_mcr_tiles_known_pattern() {
        // 1 tile, 32 bytes total (16 bytes first half + 16 bytes second half).
        // MSB-first: px=0 reads bits 7,6 of each byte.
        // For px=0, py=0: byte_off=0, local_px=0
        //   hi_byte (second half, byte 0) = 0x80: bit7=1 -> p0=1, bit6=0 -> p1=0
        //   lo_byte (first half, byte 0) = 0xC0: bit7=1 -> p2=1, bit6=1 -> p3=1
        //   pixel = (1<<3)|(0<<2)|(1<<1)|1 = 0b1011 = 11
        let mut rom = vec![0u8; 32]; // 16 bytes per half
        rom[0] = 0xC0; // first half, tile 0, row 0: bits 7,6 set
        rom[16] = 0x80; // second half, tile 0, row 0: bit 7 set

        let cache = decode_mcr_tiles(&rom, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0b1011); // px=0: p0=1,p1=0,p2=1,p3=1
    }

    #[test]
    fn decode_mcr_tiles_all_planes_set() {
        // All bits set in both halves for px=0, py=0
        let mut rom = vec![0u8; 32];
        rom[0] = 0xFF; // first half: p2=1, p3=1 for all 4 pixels in this byte
        rom[16] = 0xFF; // second half: p0=1, p1=1 for all 4 pixels

        let cache = decode_mcr_tiles(&rom, 1);
        // px=0: MSB bits 7,6 all set -> pixel = 0x0F
        assert_eq!(cache.pixel(0, 0, 0), 0x0F);
        // px=1: MSB bits 5,4 all set -> pixel = 0x0F
        assert_eq!(cache.pixel(0, 1, 0), 0x0F);
    }

    #[test]
    fn decode_mcr_sprites_known_pattern() {
        // 1 sprite, 4 ROM quarters of 128 bytes each = 512 bytes total.
        // MSB-first: even px = high nibble, odd px = low nibble.
        // Byte value 0xF0: high nibble = 0xF (px=0), low nibble = 0x0 (px=1)
        let mut rom = vec![0u8; 512]; // 4 quarters of 128 bytes
        rom[0] = 0xF0; // ROM 0, sprite 0, row 0, group 0

        let cache = decode_mcr_sprites(&rom, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0x0F); // px=0: high nibble = 0xF
        assert_eq!(cache.pixel(0, 1, 0), 0x00); // px=1: low nibble = 0x0
    }

    #[test]
    fn decode_mcr_sprites_direct_nibble() {
        // Test that nibble value maps directly to pixel (no bit reversal).
        // Byte 0x10: high nibble = 0x1, low nibble = 0x0
        let mut rom = vec![0u8; 512];
        rom[0] = 0x10; // ROM 0, high nibble = 1

        let cache = decode_mcr_sprites(&rom, 1);
        assert_eq!(cache.pixel(0, 0, 0), 0x01); // direct: nibble 1 = pixel 1
    }

    #[test]
    fn decode_mcr_sprites_rom_distribution() {
        // Verify columns are distributed across ROMs correctly.
        // px=0,1 -> ROM 0; px=2,3 -> ROM 1; px=4,5 -> ROM 2; px=6,7 -> ROM 3
        // Then px=8,9 -> ROM 0 again (group 1), etc.
        // MSB-first: even px = high nibble, odd px = low nibble.
        let mut rom = vec![0u8; 512]; // 4 x 128
        rom[0] = 0x11; // ROM 0: high=1, low=1
        rom[128] = 0x22; // ROM 1: high=2, low=2
        rom[256] = 0x33; // ROM 2: high=3, low=3
        rom[384] = 0x44; // ROM 3: high=4, low=4

        let cache = decode_mcr_sprites(&rom, 1);
        // px=0: ROM0, even -> high nibble = 0x1
        assert_eq!(cache.pixel(0, 0, 0), 0x01);
        // px=1: ROM0, odd -> low nibble = 0x1
        assert_eq!(cache.pixel(0, 1, 0), 0x01);
        // px=2: ROM1, even -> high nibble = 0x2
        assert_eq!(cache.pixel(0, 2, 0), 0x02);
        // px=3: ROM1, odd -> low nibble = 0x2
        assert_eq!(cache.pixel(0, 3, 0), 0x02);
        // px=4: ROM2, even -> high nibble = 0x3
        assert_eq!(cache.pixel(0, 4, 0), 0x03);
        // px=6: ROM3, even -> high nibble = 0x4
        assert_eq!(cache.pixel(0, 6, 0), 0x04);
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
}
