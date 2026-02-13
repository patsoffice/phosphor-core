/// Williams Special Chip 1 (SC1) — Blitter / DMA engine
///
/// Hardware block copy/fill engine used in Williams 2nd-generation arcade boards
/// (Joust, Robotron 2084, Bubbles, Sinistar, etc.). Operates on video RAM via DMA,
/// halting the CPU during transfers.
///
/// # Register map (offsets 0-7)
///
/// | Offset | Name        | Description                                           |
/// |--------|-------------|-------------------------------------------------------|
/// | 0      | mask        | Bit plane mask — controls which dest bits are modified|
/// | 1      | solid_color | Source byte for solid fill mode                       |
/// | 2      | src_hi      | Source address high byte                              |
/// | 3      | src_lo      | Source address low byte                               |
/// | 4      | dst_hi      | Destination address high byte                         |
/// | 5      | dst_lo      | Destination address low byte                          |
/// | 6      | width       | Width in bytes minus 1 (0 = 1 byte)                   |
/// | 7      | height      | Height in rows minus 1 — writing triggers the blit    |
///
/// # Control flags (set separately via `set_control`)
///
/// | Bit | Flag            | Description                                       |
/// |-----|-----------------|---------------------------------------------------|
/// | 0   | (speed)         | Ignored — always 1 byte/cycle                     |
/// | 1   | foreground_only | Skip writing when source byte is 0x00             |
/// | 2   | solid           | Use `solid_color` instead of reading from source  |
/// | 3   | shift           | 4-bit right shift with shift register             |
///
/// # DMA integration
///
/// After triggering, `is_active()` returns `true`. The system should halt the
/// CPU by returning `true` from `Bus::is_halted_for(Cpu(0))` while the blitter
/// is active. Each call to `do_dma_cycle()` transfers one byte. When the blit
/// completes, `is_active()` returns `false` and the CPU resumes.
pub struct WilliamsBlitter {
    // Registers (offsets 0-7)
    mask: u8,
    solid_color: u8,
    src_addr: u16,
    dst_addr: u16,
    width: u8,
    height: u8,

    // Control flags (set via separate address)
    control: u8,

    // Execution state
    active: bool,
    cur_src: u16,
    cur_dst: u16,
    col: u16, // u16 to avoid overflow when width=255
    row: u16, // u16 to avoid overflow when height=255
    dst_row_start: u16,
    shift_reg: u8,
}

// Control flag bit positions
const FLAG_FOREGROUND_ONLY: u8 = 0x02; // Bit 1: skip zero source bytes
const FLAG_SOLID: u8 = 0x04; // Bit 2: use solid_color instead of reading source
const FLAG_SHIFT: u8 = 0x08; // Bit 3: 4-bit right shift

impl WilliamsBlitter {
    pub fn new() -> Self {
        Self {
            mask: 0,
            solid_color: 0,
            src_addr: 0,
            dst_addr: 0,
            width: 0,
            height: 0,
            control: 0,
            active: false,
            cur_src: 0,
            cur_dst: 0,
            col: 0,
            row: 0,
            dst_row_start: 0,
            shift_reg: 0,
        }
    }

    /// Write to a blitter data register (offset 0-7).
    /// Writing to offset 7 (height) triggers the blit operation.
    pub fn write_register(&mut self, offset: u8, data: u8) {
        match offset {
            0 => self.mask = data,
            1 => self.solid_color = data,
            2 => self.src_addr = (self.src_addr & 0x00FF) | ((data as u16) << 8),
            3 => self.src_addr = (self.src_addr & 0xFF00) | (data as u16),
            4 => self.dst_addr = (self.dst_addr & 0x00FF) | ((data as u16) << 8),
            5 => self.dst_addr = (self.dst_addr & 0xFF00) | (data as u16),
            6 => self.width = data,
            7 => {
                self.height = data;
                self.start_blit();
            }
            _ => {}
        }
    }

    /// Read a blitter register (offset 0-7).
    pub fn read_register(&self, offset: u8) -> u8 {
        match offset {
            0 => self.mask,
            1 => self.solid_color,
            2 => (self.src_addr >> 8) as u8,
            3 => self.src_addr as u8,
            4 => (self.dst_addr >> 8) as u8,
            5 => self.dst_addr as u8,
            6 => self.width,
            7 => self.height,
            _ => 0,
        }
    }

    /// Set the blitter control flags. On Williams hardware, this is written
    /// to a separate address from the data registers. Must be configured
    /// before triggering the blit (writing register 7).
    pub fn set_control(&mut self, flags: u8) {
        self.control = flags;
    }

    /// Returns true if the blitter is currently executing a DMA transfer.
    /// When true, the CPU should be halted (TSC asserted).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Execute one DMA cycle. Reads/writes directly to video RAM.
    /// Call this once per clock cycle while `is_active()` returns true.
    ///
    /// The blitter accesses video RAM directly rather than going through
    /// the Bus trait. This is physically accurate (the blitter has its own
    /// address bus connection to video RAM) and avoids borrow conflicts.
    pub fn do_dma_cycle(&mut self, video_ram: &mut [u8]) {
        if !self.active {
            return;
        }

        // Step 1: Get source byte
        let raw_src = if (self.control & FLAG_SOLID) != 0 {
            self.solid_color
        } else {
            video_ram.get(self.cur_src as usize).copied().unwrap_or(0)
        };

        // Step 2: Apply shift if enabled
        let src_byte = if (self.control & FLAG_SHIFT) != 0 {
            let shifted = ((self.shift_reg as u16) << 8 | (raw_src as u16)) >> 4;
            self.shift_reg = raw_src;
            (shifted & 0xFF) as u8
        } else {
            raw_src
        };

        // Step 3: Get destination byte for read-modify-write
        let dst_byte = video_ram.get(self.cur_dst as usize).copied().unwrap_or(0);

        // Step 4: Check foreground-only mode (transparency)
        let skip = (self.control & FLAG_FOREGROUND_ONLY) != 0 && src_byte == 0x00;

        // Step 5: Write result through mask
        if !skip && let Some(dst) = video_ram.get_mut(self.cur_dst as usize) {
            *dst = (src_byte & self.mask) | (dst_byte & !self.mask);
        }

        // Step 6: Advance source address (skip in solid mode)
        if (self.control & FLAG_SOLID) == 0 {
            self.cur_src = self.cur_src.wrapping_add(1);
        }

        // Step 7: Advance column/row counters
        self.col += 1;

        if self.col > self.width as u16 {
            // End of row
            self.col = 0;
            self.row += 1;

            if self.row > self.height as u16 {
                // Blit complete
                self.active = false;
                return;
            }

            // Destination: stride of 256 bytes between rows (column-major video RAM)
            self.dst_row_start = self.dst_row_start.wrapping_add(256);
            self.cur_dst = self.dst_row_start;
        } else {
            self.cur_dst = self.cur_dst.wrapping_add(1);
        }
    }

    fn start_blit(&mut self) {
        self.active = true;
        self.cur_src = self.src_addr;
        self.cur_dst = self.dst_addr;
        self.dst_row_start = self.dst_addr;
        self.col = 0;
        self.row = 0;
        self.shift_reg = 0;
    }
}

impl Default for WilliamsBlitter {
    fn default() -> Self {
        Self::new()
    }
}
