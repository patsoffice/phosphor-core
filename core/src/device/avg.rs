//! Atari Analog Vector Generator (AVG) — Tempest variant
//!
//! A state-machine coprocessor that reads byte-addressed instructions from
//! shared vector RAM/ROM and generates a display list of colored line segments
//! for rendering on a color vector CRT.
//!
//! Used in Tempest (1981). Other AVG variants (Battle Zone, Star Wars, Major
//! Havoc, Quantum) differ in color decoding and coordinate handling.
//!
//! # Architecture
//!
//! The AVG reads 4 bytes per instruction from a contiguous vector memory space.
//! The real hardware uses a 256×4-bit PROM state machine that sequences through
//! handlers 0–3 (latching DVY, opcode, DVX, intensity) then dispatches to
//! handlers 4–7 (strobe0–strobe3) based on the 3-bit opcode.
//!
//! This implementation decodes instructions directly at the word level for
//! clarity, while matching the hardware's handler behavior exactly.
//!
//! # Byte addressing
//!
//! The AVG byte-addresses vector memory with an XOR-1 swap, reading the high
//! byte of each 16-bit word first (at even PC), then the low byte (at odd PC).
//! The AVG addresses bytes as `(pc ^ 1)`, swapping bytes within each word.
//!
//! # Instruction sizes
//!
//! - Op 0 (VCTR): 4-byte instruction (two 16-bit words)
//! - Ops 1–7: 2-byte instructions (one 16-bit word)
//!
//! The PROM state machine determines handler sequencing per opcode.
//! SVEC (op 2) packs DVX and int_latch into the low byte of its
//! single word (handler 3 reads it), with 4-bit DVX/DVY precision.
//!
//! # Tempest-specific behavior
//!
//! - Color RAM: 16 entries, looked up by color index in strobe3
//! - Color vs intensity select: bit 11 of DVY in strobe2
//! - Coordinate rotation (ROT270): output swaps X/Y axes
//! - Continuous loop: jump to address 0 triggers a frame flush
//!
//! # Reference
//!
//! - Atari avgdvg hardware (avg_device, avg_tempest_device)
//! - Jed Margolin, "The Secret Life of Vector Generators"

use super::dvg::VectorLine;
use crate::core::debug::{DebugRegister, Debuggable};
use crate::core::save_state::{SaveError, Saveable};

/// Atari AVG (Tempest variant).
///
/// The AVG runs continuously (not halt-based like DVG). Each frame is
/// delineated by a jump to address 0, which flushes the accumulated
/// display list. The caller triggers execution via [`Avg::go`] + [`Avg::execute`].
pub struct Avg {
    /// Program counter (byte address into vector memory).
    pc: u16,
    /// 4-entry return address stack (byte addresses).
    stack: [u16; 4],
    /// Stack pointer (only bits 1:0 used).
    sp: u8,

    /// Current beam X position (fixed-point, pixel << 16).
    xpos: i32,
    /// Current beam Y position (fixed-point).
    ypos: i32,

    /// Previous beam position for line segment generation (fixed-point).
    prev_x: i32,
    prev_y: i32,
    has_prev: bool,

    /// Analog scale factor (8-bit).
    scale: u8,
    /// Binary scale factor (3-bit).
    bin_scale: u8,
    /// Current color index (4-bit).
    color: u8,
    /// Current intensity (4-bit).
    intensity: u8,

    /// Center coordinates in fixed-point.
    xcenter: i32,
    ycenter: i32,

    /// DAC sign XOR values (0x200 for standard AVG).
    xdac_xor: u16,
    ydac_xor: u16,

    /// Axis flipping (set via $4000 write on Tempest).
    flip_x: bool,
    flip_y: bool,

    /// True when the AVG has halted.
    halted: bool,

    /// Accumulated display list for the current frame.
    display_list: Vec<VectorLine>,
}

impl Avg {
    /// Create a new AVG with beam center derived from visible area dimensions.
    ///
    /// Beam center: `xcenter = (visible_width / 2) << 16`,
    ///              `ycenter = (visible_height / 2) << 16`.
    /// For Tempest (visible area 0..580 x 0..570): xcenter=290<<16, ycenter=285<<16.
    pub fn new(visible_width: i32, visible_height: i32) -> Self {
        let xcenter = (visible_width / 2) << 16;
        let ycenter = (visible_height / 2) << 16;
        Self {
            pc: 0,
            stack: [0; 4],
            sp: 0,
            xpos: xcenter,
            ypos: ycenter,
            prev_x: xcenter,
            prev_y: ycenter,
            has_prev: false,
            scale: 0,
            bin_scale: 0,
            color: 0,
            intensity: 0,
            xcenter,
            ycenter,
            xdac_xor: 0x200,
            ydac_xor: 0x200,
            flip_x: false,
            flip_y: false,
            halted: true,
            display_list: Vec::with_capacity(2048),
        }
    }

    /// Trigger AVG execution (CPU writes to AVG GO register).
    pub fn go(&mut self) {
        self.pc = 0;
        self.sp = 0;
        self.halted = false;
    }

    /// Returns true if the AVG has halted.
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Debug: return (scale, bin_scale, color, intensity).
    pub fn debug_state(&self) -> (u8, u8, u8, u8) {
        (self.scale, self.bin_scale, self.color, self.intensity)
    }

    /// Set axis flipping (controlled by hardware register).
    pub fn set_flip(&mut self, flip_x: bool, flip_y: bool) {
        self.flip_x = flip_x;
        self.flip_y = flip_y;
    }

    /// Reset to power-on state.
    pub fn reset(&mut self) {
        self.pc = 0;
        self.sp = 0;
        self.stack = [0; 4];
        self.scale = 0;
        self.bin_scale = 0;
        self.color = 0;
        self.intensity = 0;
        self.halted = true;
        self.has_prev = false;
        self.xpos = self.xcenter;
        self.ypos = self.ycenter;
        self.prev_x = self.xcenter;
        self.prev_y = self.ycenter;
        self.display_list.clear();
    }

    /// Execute AVG instructions until halt or frame boundary (jump to address 0).
    ///
    /// `vmem` is the combined vector RAM + ROM (8 KB for Tempest).
    /// `color_ram` is the 16-entry color RAM for Tempest color lookup.
    ///
    /// Returns true if a frame was completed.
    pub fn execute(&mut self, vmem: &[u8], color_ram: &[u8; 16]) -> bool {
        let mut instructions = 0u32;
        const MAX_INSTRUCTIONS: u32 = 50_000;

        while !self.halted && instructions < MAX_INSTRUCTIONS {
            // --- Decode ---
            // The AVG PROM state machine reads bytes in handler order 1,0
            // (high byte first via XOR-1), then 3,2 for 4-byte instructions.
            // Only VCTR (op=0) is 4-byte; all others are 2-byte.
            let hi0 = Self::read_byte(vmem, self.pc);
            let lo0 = Self::read_byte(vmem, self.pc.wrapping_add(1));
            let dvy12 = (hi0 >> 4) & 1;
            let op = hi0 >> 5;

            let (dvy, dvx, int_latch) = if op == 0 {
                // VCTR: 4-byte instruction (two 16-bit words)
                // Word 0 (handlers 1,0): DVY
                // Word 1 (handlers 3,2): int_latch + DVX
                let dvy = (u16::from(dvy12) << 12) | (u16::from(hi0 & 0xF) << 8) | u16::from(lo0);
                let hi1 = Self::read_byte(vmem, self.pc.wrapping_add(2));
                let lo1 = Self::read_byte(vmem, self.pc.wrapping_add(3));
                self.pc = self.pc.wrapping_add(4);
                let il = hi1 >> 4;
                let dx = (u16::from(il & 1) << 12) | (u16::from(hi1 & 0xF) << 8) | u16::from(lo1);
                (dvy, dx, il)
            } else if op == 2 {
                // SVEC: 2-byte instruction (one 16-bit word)
                // High byte (handler 1): opcode + dvy12 + dvy[11:8]
                // Low byte (handler 3): int_latch[3:0] + dvx[11:8]
                // DVY and DVX lower 8 bits are zero (4-bit precision).
                let dvy = (u16::from(dvy12) << 12) | (u16::from(hi0 & 0xF) << 8);
                let il = lo0 >> 4;
                let dx = (u16::from(il & 1) << 12) | (u16::from(lo0 & 0xF) << 8);
                self.pc = self.pc.wrapping_add(2);
                (dvy, dx, il)
            } else {
                // All other ops: 2-byte instruction
                // DVY from high byte only (handler 1); lo0 used by handler 0
                // for dvy lower bits in STAT/HALT/CNTR/JSR/RTS/JMP.
                let dvy = (u16::from(dvy12) << 12) | (u16::from(hi0 & 0xF) << 8) | u16::from(lo0);
                self.pc = self.pc.wrapping_add(2);
                (dvy, 0u16, 0u8)
            };

            // --- Execute ---
            match op {
                0 | 2 => {
                    let is_short = op == 2;
                    let (norm_dvx, norm_dvy, timer) = self.normalize(dvx, dvy, is_short);
                    let timer = self.apply_bin_scale(timer, is_short);
                    self.draw_vector(norm_dvx, norm_dvy, timer, is_short, int_latch, color_ram);
                }
                1 => self.halted = true,
                3 => {
                    if dvy12 != 0 {
                        self.scale = (dvy & 0xFF) as u8;
                        self.bin_scale = ((dvy >> 8) & 7) as u8;
                    } else if dvy & 0x800 != 0 {
                        self.color = (dvy & 0xF) as u8;
                    } else {
                        self.intensity = ((dvy >> 4) & 0xF) as u8;
                    }
                }
                4 => {
                    self.xpos = self.xcenter;
                    self.ypos = self.ycenter;
                    self.add_point(self.xpos, self.ypos, 0, [0, 0, 0]);
                }
                5 => {
                    self.stack[(self.sp & 3) as usize] = self.pc;
                    self.sp = self.sp.wrapping_add(1) & 0xF;
                    self.pc = dvy << 1;
                    if dvy == 0 {
                        self.halted = true;
                        return true;
                    }
                }
                6 => {
                    self.sp = self.sp.wrapping_sub(1) & 0xF;
                    self.pc = self.stack[(self.sp & 3) as usize];
                }
                7 => {
                    self.pc = dvy << 1;
                    if dvy == 0 {
                        self.halted = true;
                        return true;
                    }
                }
                _ => {}
            }

            instructions += 1;
        }
        false
    }

    /// Drain the display list, returning ownership to the caller.
    pub fn take_display_list(&mut self) -> Vec<VectorLine> {
        self.has_prev = false;
        std::mem::take(&mut self.display_list)
    }

    // -----------------------------------------------------------------------
    // Instruction decode and execute
    // -----------------------------------------------------------------------

    /// Read one byte from vector memory with AVG XOR-1 byte swap.
    ///
    /// The AVG addresses bytes with `addr ^ 1`, which swaps the two bytes
    /// within each 16-bit word. This means reading at even PC gives the
    /// high byte, and odd PC gives the low byte.
    fn read_byte(vmem: &[u8], addr: u16) -> u8 {
        let idx = (addr as usize) ^ 1;
        if idx < vmem.len() { vmem[idx] } else { 0 }
    }

    /// Normalize DVX/DVY (strobe0) — shift both axes together until EITHER
    /// is normalized (sign bit differs from MSB).
    fn normalize(&self, mut dvx: u16, mut dvy: u16, is_short: bool) -> (u16, u16, u16) {
        let mut timer: u16 = 0;
        let op1_bit: u16 = if is_short { 0x80 } else { 0 };

        // Continue while BOTH axes need normalization (AND condition).
        // Stop when EITHER axis is normalized or after 16 iterations.
        let mut i = 0;
        while (((dvy ^ (dvy << 1)) & 0x1000) == 0)
            && (((dvx ^ (dvx << 1)) & 0x1000) == 0)
            && (i < 16)
        {
            dvy = (dvy & 0x1000) | ((dvy << 1) & 0x1FFF);
            dvx = (dvx & 0x1000) | ((dvx << 1) & 0x1FFF);
            timer >>= 1;
            timer |= 0x4000 | op1_bit;
            i += 1;
        }

        // SVEC: mask timer to 8 bits
        if is_short {
            timer &= 0xFF;
        }

        (dvx, dvy, timer)
    }

    /// Apply binary scale to the timer (strobe1).
    fn apply_bin_scale(&self, mut timer: u16, is_short: bool) -> u16 {
        let op1_bit: u16 = if is_short { 0x80 } else { 0 };

        for _ in 0..self.bin_scale {
            timer >>= 1;
            timer |= 0x4000 | op1_bit;
        }

        // SVEC: mask timer to 8 bits again after bin_scale
        if is_short {
            timer &= 0xFF;
        }

        timer
    }

    /// Draw a vector from the current beam position using normalized DVX/DVY.
    /// Matches MAME's `avg_common_strobe3` + `tempest_strobe3`.
    fn draw_vector(
        &mut self,
        dvx: u16,
        dvy: u16,
        timer: u16,
        is_short: bool,
        int_latch: u8,
        color_ram: &[u8; 16],
    ) {
        // SVEC uses 8-bit timer, VCTR uses 16-bit timer
        let cycles: i32 = if is_short {
            0x100_i32 - i32::from(timer & 0xFF)
        } else {
            0x8000_i32 - i32::from(timer)
        };

        // Scale factor: complement of 8-bit scale
        let scale_factor: i32 = i32::from(self.scale) ^ 0xFF;

        // DAC conversion: upper 10 bits of 13-bit value, XOR for sign, center at 0
        let dx = ((i32::from(dvx >> 3) ^ i32::from(self.xdac_xor)) - 0x200)
            .wrapping_mul(cycles)
            .wrapping_mul(scale_factor)
            >> 4;
        let dy = ((i32::from(dvy >> 3) ^ i32::from(self.ydac_xor)) - 0x200)
            .wrapping_mul(cycles)
            .wrapping_mul(scale_factor)
            >> 4;
        self.xpos = self.xpos.wrapping_add(dx);
        self.ypos = self.ypos.wrapping_sub(dy);

        // Tempest color RAM lookup
        let data = color_ram[(self.color & 0xF) as usize];
        let bit3 = (!data >> 3) & 1;
        let bit2 = (!data >> 2) & 1;
        let bit1 = (!data >> 1) & 1;
        let bit0 = !data & 1;
        let r = bit1
            .wrapping_mul(0xF3)
            .wrapping_add(bit0.wrapping_mul(0x0C));
        let g = bit3.wrapping_mul(0xF3);
        let b = bit2.wrapping_mul(0xF3);

        // Effective intensity: when int_latch bits 3:1 == 001 (DATEA signal), use stored intensity
        // from STAT register; otherwise use int_latch bits 3:1 as direct intensity.
        let eff_intensity = if (int_latch >> 1) == 1 {
            self.intensity
        } else {
            int_latch & 0xE
        };

        // Apply flipping
        let mut x = self.xpos;
        let mut y = self.ypos;
        if self.flip_x {
            x += (self.xcenter - x) << 1;
        }
        if self.flip_y {
            y += (self.ycenter - y) << 1;
        }

        self.add_point(x, y, eff_intensity, [r, g, b]);
    }

    /// Add a point to the display list, creating a line from the previous point.
    ///
    /// Coordinates are stored unclamped — the rasterizer handles clipping.
    fn add_point(&mut self, x: i32, y: i32, intensity: u8, rgb: [u8; 3]) {
        // Convert from fixed-point to pixel coordinates (no clamping).
        let px = x >> 16;
        let py = y >> 16;

        if self.has_prev {
            let prev_px = self.prev_x >> 16;
            let prev_py = self.prev_y >> 16;

            if intensity == 0 && prev_px == px && prev_py == py {
                self.prev_x = x;
                self.prev_y = y;
                return;
            }

            self.display_list.push(VectorLine {
                x0: prev_px,
                y0: prev_py,
                x1: px,
                y1: py,
                intensity,
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
            });
        } else {
            self.display_list.push(VectorLine {
                x0: px,
                y0: py,
                x1: px,
                y1: py,
                intensity: 0,
                r: 0,
                g: 0,
                b: 0,
            });
        }

        self.prev_x = x;
        self.prev_y = y;
        self.has_prev = true;
    }
}

impl Debuggable for Avg {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "PC",
                value: self.pc as u64,
                width: 16,
            },
            DebugRegister {
                name: "X",
                value: (self.xpos >> 16) as u64,
                width: 16,
            },
            DebugRegister {
                name: "Y",
                value: (self.ypos >> 16) as u64,
                width: 16,
            },
            DebugRegister {
                name: "SCALE",
                value: self.scale as u64,
                width: 8,
            },
            DebugRegister {
                name: "COLOR",
                value: self.color as u64,
                width: 8,
            },
            DebugRegister {
                name: "INTEN",
                value: self.intensity as u64,
                width: 8,
            },
            DebugRegister {
                name: "HALT",
                value: self.halted as u64,
                width: 8,
            },
        ]
    }
}

impl Saveable for Avg {
    fn save_state(&self, w: &mut crate::core::save_state::StateWriter) {
        w.write_u16_le(self.pc);
        for &s in &self.stack {
            w.write_u16_le(s);
        }
        w.write_u8(self.sp);
        w.write_i32_le(self.xpos);
        w.write_i32_le(self.ypos);
        w.write_u8(self.scale);
        w.write_u8(self.bin_scale);
        w.write_u8(self.color);
        w.write_u8(self.intensity);
        w.write_bool(self.flip_x);
        w.write_bool(self.flip_y);
        w.write_bool(self.halted);
    }

    fn load_state(
        &mut self,
        r: &mut crate::core::save_state::StateReader,
    ) -> Result<(), SaveError> {
        self.pc = r.read_u16_le()?;
        for s in &mut self.stack {
            *s = r.read_u16_le()?;
        }
        self.sp = r.read_u8()?;
        self.xpos = r.read_i32_le()?;
        self.ypos = r.read_i32_le()?;
        self.scale = r.read_u8()?;
        self.bin_scale = r.read_u8()?;
        self.color = r.read_u8()?;
        self.intensity = r.read_u8()?;
        self.flip_x = r.read_bool()?;
        self.flip_y = r.read_bool()?;
        self.halted = r.read_bool()?;
        self.has_prev = false;
        self.display_list.clear();
        Ok(())
    }
}

impl super::Device for Avg {
    fn name(&self) -> &'static str {
        "AVG"
    }
    fn reset(&mut self) {
        self.reset();
    }
}

impl Default for Avg {
    fn default() -> Self {
        Self::new(1024, 1024)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build vector memory from 16-bit words stored as little-endian byte pairs.
    ///
    /// Each pair of bytes represents one 16-bit word: `[low_byte, high_byte]`.
    /// The bytes are stored directly at their physical addresses (matching how
    /// the 6502 CPU writes to vector RAM). The AVG's `read_byte` handles the
    /// XOR-1 byte swap to read high byte first, then low byte.
    fn build_vmem(bytes: &[u8]) -> Vec<u8> {
        let mut vmem = vec![0u8; 8192]; // 8KB
        for (i, &b) in bytes.iter().enumerate() {
            vmem[i] = b;
        }
        vmem
    }

    /// Helper: encode a 16-bit AVG word as [low_byte, high_byte] for build_vmem.
    fn word(val: u16) -> [u8; 2] {
        [(val & 0xFF) as u8, (val >> 8) as u8]
    }

    fn default_color_ram() -> [u8; 16] {
        // All white (inverted bits → r=0xFF, g=0xF3, b=0xF3)
        [0x00; 16]
    }

    #[test]
    fn new_starts_halted() {
        let avg = Avg::new(1024, 1024);
        assert!(avg.is_halted());
    }

    #[test]
    fn go_clears_halt() {
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        assert!(!avg.is_halted());
        assert_eq!(avg.pc, 0);
    }

    #[test]
    fn halt_instruction() {
        // HALT: op=1 → high byte = 0b001_0_0000 = 0x20
        // 2-byte instruction: only word 0 needed.
        let w0 = word(0x2000); // HALT
        let vmem = build_vmem(&[w0[0], w0[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        avg.execute(&vmem, &color_ram);
        assert!(avg.is_halted());
    }

    #[test]
    fn reset_clears_state() {
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        avg.color = 5;
        avg.intensity = 12;
        avg.scale = 0x80;
        avg.reset();
        assert!(avg.is_halted());
        assert_eq!(avg.color, 0);
        assert_eq!(avg.intensity, 0);
        assert_eq!(avg.scale, 0);
        assert!(avg.display_list.is_empty());
    }

    #[test]
    fn frame_boundary_on_jsr_to_zero() {
        // JSR to address 0 = frame boundary.
        // JSR: op=5 → high byte = 0b101_0_0000 = 0xA0, dvy=0 → target=0
        // This is a 2-byte instruction.
        let w0 = word(0xA000); // JSR, dvy=0
        let vmem = build_vmem(&[w0[0], w0[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        let frame = avg.execute(&vmem, &color_ram);
        assert!(frame, "expected frame boundary on JSR to address 0");
    }

    #[test]
    fn frame_boundary_on_jmp_to_zero() {
        // JMP to address 0 = frame boundary.
        // JMP: op=7 → high byte = 0b111_0_0000 = 0xE0, dvy=0 → target=0
        let w0 = word(0xE000); // JMP, dvy=0
        let vmem = build_vmem(&[w0[0], w0[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        let frame = avg.execute(&vmem, &color_ram);
        assert!(frame, "expected frame boundary on JMP to address 0");
    }

    #[test]
    fn two_byte_instruction_advances_pc_by_2() {
        // CNTR (op=4) is a 2-byte instruction. After it, PC should be 2.
        // Then a HALT at byte offset 2 should be reached.
        // CNTR: op=4 → high byte = 0b100_0_0000 = 0x80
        // HALT: op=1 → 0x2000 (also 2-byte)
        let w0 = word(0x8000); // CNTR (2 bytes)
        let w1 = word(0x2000); // HALT (2 bytes)
        let vmem = build_vmem(&[w0[0], w0[1], w1[0], w1[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        avg.execute(&vmem, &color_ram);
        assert!(
            avg.is_halted(),
            "CNTR should advance PC by 2, reaching HALT at offset 2"
        );
    }

    #[test]
    fn stat_advances_pc_by_2() {
        // STAT (op=3) is a 2-byte instruction. After it, PC should be 2.
        // STAT with dvy12=0, bit11=0: sets intensity.
        // STAT: op=3 → high byte = 0b011_0_0000 = 0x60, dvy12=0
        // Set intensity to 0xA: DVY bits [7:4] = 0xA → low byte = 0xA0
        let w0 = word(0x6000 | 0x00A0); // STAT: set intensity to 0xA
        let w1 = word(0x2000); // HALT at offset 2
        let vmem = build_vmem(&[w0[0], w0[1], w1[0], w1[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        avg.execute(&vmem, &color_ram);
        assert!(avg.is_halted());
        assert_eq!(avg.intensity, 0xA);
    }

    #[test]
    fn color_ram_decode() {
        // Verify color RAM decode matches Tempest hardware.
        // color_ram[0] = 0x05, ~0x05 = 0xFA
        // bit0 = 0xFA & 1 = 0
        // bit1 = (0xFA >> 1) & 1 = 1
        // bit2 = (0xFA >> 2) & 1 = 0
        // bit3 = (0xFA >> 3) & 1 = 1
        // r = 1 * 0xF3 + 0 * 0x0C = 0xF3
        // g = 1 * 0xF3 = 0xF3
        // b = 0 * 0xF3 = 0
        let data: u8 = 0x05;
        let bit3 = (!data >> 3) & 1;
        let bit2 = (!data >> 2) & 1;
        let bit1 = (!data >> 1) & 1;
        let bit0 = !data & 1;
        let r = bit1
            .wrapping_mul(0xF3)
            .wrapping_add(bit0.wrapping_mul(0x0C));
        let g = bit3.wrapping_mul(0xF3);
        let b = bit2.wrapping_mul(0xF3);
        assert_eq!(r, 0xF3);
        assert_eq!(g, 0xF3);
        assert_eq!(b, 0);
    }

    #[test]
    fn vctr_then_halt_produces_display_list() {
        // VCTR (op=0) draws a vector, then HALT stops. Display list should
        // contain the drawn vector even though execute returns false.
        //
        // VCTR word 0: op=0, dvy12=0, DVY = 0x200 → 0x0200
        //   high byte = 0b000_0_0010 = 0x02, low byte = 0x00
        //   word = 0x0200
        // VCTR word 1: int_latch=0x8 (intensity=8), DVX = 0x200
        //   high byte = 0b1000_0010 = 0x82, low byte = 0x00
        //   word = 0x8200
        // HALT: 0x2000 (2-byte instruction)
        let vmem = build_vmem(&[
            0x00, 0x02, 0x00, 0x82, // VCTR: DVY=0x200, DVX=0x200, intensity=8
            0x00, 0x20, // HALT
        ]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        let frame = avg.execute(&vmem, &color_ram);
        assert!(!frame, "HALT should not signal frame boundary");
        assert!(avg.is_halted(), "AVG should be halted after HALT");

        let display_list = avg.take_display_list();
        assert!(
            !display_list.is_empty(),
            "display list should contain vectors drawn before HALT"
        );
    }

    #[test]
    fn stat_sets_scale() {
        // STAT with dvy12=1: sets scale and bin_scale.
        // STAT is a 2-byte instruction.
        // op=3, dvy12=1 → high byte bits: 011_1_YYYY
        // scale = DVY & 0xFF = low byte
        // bin_scale = (DVY >> 8) & 7 = high byte bits 2:0
        //
        // Set scale=0x80, bin_scale=3:
        //   DVY = (3 << 8) | 0x80 = 0x0380
        //   dvy12=1 → bit 4 of high byte set
        //   high byte = 0b011_1_0011 = 0x73
        //   low byte = 0x80
        //   word = 0x7380
        let w0 = word(0x7380); // STAT: dvy12=1, scale=0x80, bin_scale=3
        let w1 = word(0x2000); // HALT at offset 2
        let vmem = build_vmem(&[w0[0], w0[1], w1[0], w1[1]]);
        let color_ram = default_color_ram();
        let mut avg = Avg::new(1024, 1024);
        avg.go();
        avg.execute(&vmem, &color_ram);
        assert!(avg.is_halted());
        assert_eq!(avg.scale, 0x80);
        assert_eq!(avg.bin_scale, 3);
    }
}
