/// Minimal 4x5 bitmap font for FPS overlay. Each glyph is 4 pixels wide, 5 rows tall.
/// Bits are MSB-left within each u8 (only top 4 bits used).
const GLYPHS: &[(&[u8; 5], u8)] = &[
    // '0'
    (&[0x60, 0x90, 0x90, 0x90, 0x60], b'0'),
    // '1'
    (&[0x20, 0x60, 0x20, 0x20, 0x70], b'1'),
    // '2'
    (&[0x60, 0x90, 0x20, 0x40, 0xF0], b'2'),
    // '3'
    (&[0x60, 0x90, 0x20, 0x90, 0x60], b'3'),
    // '4'
    (&[0x90, 0x90, 0xF0, 0x10, 0x10], b'4'),
    // '5'
    (&[0xF0, 0x80, 0xE0, 0x10, 0xE0], b'5'),
    // '6'
    (&[0x60, 0x80, 0xE0, 0x90, 0x60], b'6'),
    // '7'
    (&[0xF0, 0x10, 0x20, 0x40, 0x40], b'7'),
    // '8'
    (&[0x60, 0x90, 0x60, 0x90, 0x60], b'8'),
    // '9'
    (&[0x60, 0x90, 0x70, 0x10, 0x60], b'9'),
    // '.'
    (&[0x00, 0x00, 0x00, 0x00, 0x40], b'.'),
    // ' '
    (&[0x00, 0x00, 0x00, 0x00, 0x00], b' '),
];

const GLYPH_W: usize = 4;

fn glyph_for(ch: u8) -> &'static [u8; 5] {
    for &(data, c) in GLYPHS {
        if c == ch {
            return data;
        }
    }
    // fallback: space
    &[0x00, 0x00, 0x00, 0x00, 0x00]
}

/// Draw an FPS string (e.g. "60.1") onto an RGB24 framebuffer.
/// Renders at the top-left corner with 1px padding.
pub fn draw_fps(buffer: &mut [u8], width: usize, text: &str) {
    let x0: usize = 2;
    let y0: usize = 2;

    for (ci, ch) in text.bytes().enumerate() {
        let glyph = glyph_for(ch);
        let gx = x0 + ci * (GLYPH_W + 1);

        for (row, &bits) in glyph.iter().enumerate() {
            let py = y0 + row;
            for col in 0..GLYPH_W {
                if bits & (0x80 >> col) != 0 {
                    let px = gx + col;
                    let offset = (py * width + px) * 3;
                    if offset + 2 < buffer.len() {
                        buffer[offset] = 255;
                        buffer[offset + 1] = 255;
                        buffer[offset + 2] = 255;
                    }
                }
            }
        }
    }
}
