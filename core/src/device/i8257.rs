use crate::core::{Bus, BusMaster};

/// Intel 8257 Programmable DMA Controller
///
/// 4-channel DMA controller for transferring data between memory and
/// peripherals without CPU intervention. Used in systems like Donkey Kong
/// for sprite DMA.
///
/// # Register map (9 I/O ports, offsets 0x0–0x8)
///
/// | Offset | Read                 | Write                |
/// |--------|----------------------|----------------------|
/// | 0      | Ch0 Address          | Ch0 Address          |
/// | 1      | Ch0 Terminal Count   | Ch0 Terminal Count   |
/// | 2      | Ch1 Address          | Ch1 Address          |
/// | 3      | Ch1 Count            | Ch1 Count            |
/// | 4      | Ch2 Address          | Ch2 Address          |
/// | 5      | Ch2 Count            | Ch2 Count            |
/// | 6      | Ch3 Address          | Ch3 Address          |
/// | 7      | Ch3 Count            | Ch3 Count            |
/// | 8      | Status Register      | Mode Register        |
///
/// Address and count registers are 16-bit, accessed as two consecutive
/// 8-bit operations via a shared first/last flip-flop (LSB first, then MSB).
///
/// Count register bits 15:14 encode the transfer mode:
/// - `00` = Verify (no actual transfer)
/// - `01` = Write (peripheral → memory)
/// - `10` = Read (memory → peripheral)
/// - `11` = Illegal
///
/// # Mode register bits (write-only, offset 0x8)
///
/// | Bit | Name              | Description                                |
/// |-----|-------------------|--------------------------------------------|
/// | 0-3 | Channel enable    | Set bit N to enable channel N              |
/// | 4   | Rotating priority | 0 = fixed (ch0 highest), 1 = rotating     |
/// | 5   | TC Stop           | Auto-disable channel on terminal count     |
/// | 6   | Auto-load         | Ch2 reloads from Ch3 base regs on TC      |
/// | 7   | Reserved          | Always 0                                   |
///
/// # Status register bits (read-only, offset 0x8)
///
/// | Bit | Description                              |
/// |-----|------------------------------------------|
/// | 0-3 | Terminal count reached on channels 0-3   |
/// | 4   | Update flag (auto-load occurred)          |
/// | 5-7 | Always 0                                 |
///
/// Writing the mode register resets the flip-flop and clears TC/update flags.
///
/// # DMA integration
///
/// Peripherals assert DREQ via `set_dreq()`. When any enabled channel has
/// DREQ active, `hrq()` returns true — the board should halt the CPU.
/// Each call to `do_dma_cycle()` transfers one byte and returns a
/// `DmaTransfer` describing what happened (channel, data, direction, TC).
pub struct I8257 {
    channels: [DmaChannel; 4],
    flip_flop: bool, // false = LSB, true = MSB
    mode: u8,
    tc_flags: u8,
    update_flag: bool,
    dreq: [bool; 4],
    last_serviced: usize,
}

#[derive(Clone, Copy, Default)]
struct DmaChannel {
    address: u16,
    count: u16, // bits 15:14 = transfer mode, bits 13:0 = byte count
    base_address: u16,
    base_count: u16,
}

/// Result of a single DMA transfer cycle.
#[derive(Debug, Clone, Copy)]
pub struct DmaTransfer {
    /// Which channel performed the transfer (0-3).
    pub channel: usize,
    /// The byte that was transferred.
    pub data: u8,
    /// Direction of transfer.
    pub direction: DmaDirection,
    /// True if terminal count was reached on this cycle.
    pub tc: bool,
}

/// DMA transfer direction, encoded in count register bits 15:14.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DmaDirection {
    /// No actual data transfer; address/count still update.
    Verify,
    /// Peripheral → memory (8257 writes to memory).
    Write,
    /// Memory → peripheral (8257 reads from memory).
    Read,
}

// Mode register bit masks
const MODE_CH_ENABLE_MASK: u8 = 0x0F;
const MODE_ROTATING_PRIORITY: u8 = 0x10;
const MODE_TC_STOP: u8 = 0x20;
const MODE_AUTO_LOAD: u8 = 0x40;

// Transfer mode encoding in count register bits 15:14
const XFER_VERIFY: u16 = 0b00;
const XFER_WRITE: u16 = 0b01;
const XFER_READ: u16 = 0b10;

impl I8257 {
    pub fn new() -> Self {
        Self {
            channels: [DmaChannel::default(); 4],
            flip_flop: false,
            mode: 0,
            tc_flags: 0,
            update_flag: false,
            dreq: [false; 4],
            last_serviced: 0,
        }
    }

    /// Read a register (offset 0-8).
    ///
    /// Offsets 0-7 read channel address/count registers via the flip-flop.
    /// Offset 8 reads the status register. Reading status does NOT clear
    /// TC flags (only a mode register write does).
    pub fn read(&mut self, offset: u8) -> u8 {
        match offset {
            0..=7 => {
                let ch = (offset / 2) as usize;
                let is_count = (offset & 1) != 0;
                let reg = if is_count {
                    self.channels[ch].count
                } else {
                    self.channels[ch].address
                };
                let byte = if self.flip_flop {
                    (reg >> 8) as u8
                } else {
                    reg as u8
                };
                self.flip_flop = !self.flip_flop;
                byte
            }
            8 => {
                let mut status = self.tc_flags & 0x0F;
                if self.update_flag {
                    status |= 0x10;
                }
                status
            }
            _ => 0xFF,
        }
    }

    /// Write a register (offset 0-8).
    ///
    /// Offsets 0-7 write channel address/count registers via the flip-flop.
    /// Offset 8 writes the mode register, resets the flip-flop, and clears
    /// TC flags and the update flag.
    pub fn write(&mut self, offset: u8, data: u8) {
        match offset {
            0..=7 => {
                let ch = (offset / 2) as usize;
                let is_count = (offset & 1) != 0;
                let reg = if is_count {
                    &mut self.channels[ch].count
                } else {
                    &mut self.channels[ch].address
                };
                if self.flip_flop {
                    *reg = (*reg & 0x00FF) | ((data as u16) << 8);
                } else {
                    *reg = (*reg & 0xFF00) | (data as u16);
                }
                self.flip_flop = !self.flip_flop;

                // Latch base values for auto-load
                self.channels[ch].base_address = self.channels[ch].address;
                self.channels[ch].base_count = self.channels[ch].count;
            }
            8 => {
                self.mode = data;
                self.flip_flop = false;
                self.tc_flags = 0;
                self.update_flag = false;
            }
            _ => {}
        }
    }

    /// Read the current address register for a channel.
    pub fn channel_address(&self, channel: usize) -> u16 {
        self.channels[channel].address
    }

    /// Read the current count register for a channel (includes mode bits 15:14).
    pub fn channel_count(&self, channel: usize) -> u16 {
        self.channels[channel].count
    }

    /// Set the DREQ (DMA request) input for a channel.
    pub fn set_dreq(&mut self, channel: usize, active: bool) {
        if channel < 4 {
            self.dreq[channel] = active;
        }
    }

    /// Returns true if the DMA controller is requesting the bus (HRQ).
    ///
    /// True when any enabled channel has an active DREQ.
    pub fn hrq(&self) -> bool {
        let enable_mask = self.mode & MODE_CH_ENABLE_MASK;
        for ch in 0..4 {
            if self.dreq[ch] && (enable_mask & (1 << ch)) != 0 {
                return true;
            }
        }
        false
    }

    /// Execute one DMA transfer cycle through the system bus.
    ///
    /// For **Read** transfers (memory → peripheral), reads a byte from memory
    /// at the channel's address and returns it in `DmaTransfer::data`.
    ///
    /// For **Write** transfers (peripheral → memory), writes `dack_data` to
    /// memory at the channel's address.
    ///
    /// For **Verify** transfers, no bus access occurs but address/count still
    /// update.
    ///
    /// Returns `None` if no enabled channel has an active DREQ.
    pub fn do_dma_cycle(
        &mut self,
        bus: &mut dyn Bus<Address = u16, Data = u8>,
        dack_data: u8,
    ) -> Option<DmaTransfer> {
        let ch = self.select_channel()?;
        let channel = &mut self.channels[ch];
        let mode_bits = channel.count >> 14;

        let (data, direction) = match mode_bits {
            XFER_READ => {
                let byte = bus.read(BusMaster::Dma, channel.address);
                (byte, DmaDirection::Read)
            }
            XFER_WRITE => {
                bus.write(BusMaster::Dma, channel.address, dack_data);
                (dack_data, DmaDirection::Write)
            }
            XFER_VERIFY => (0, DmaDirection::Verify),
            _ => (0, DmaDirection::Verify), // Illegal mode treated as verify
        };

        // Increment address
        channel.address = channel.address.wrapping_add(1);

        // Check terminal count (14-bit count field reaches 0)
        let count = channel.count & 0x3FFF;
        let tc = count == 0;

        if tc {
            self.tc_flags |= 1 << ch;
            self.handle_terminal_count(ch);
        } else {
            // Decrement 14-bit count, preserving mode bits
            channel.count = (channel.count & 0xC000) | (count - 1);
        }

        // Update rotating priority
        if (self.mode & MODE_ROTATING_PRIORITY) != 0 {
            self.last_serviced = ch;
        }

        Some(DmaTransfer {
            channel: ch,
            data,
            direction,
            tc,
        })
    }

    /// Select the highest-priority enabled channel with active DREQ.
    fn select_channel(&self) -> Option<usize> {
        let enable_mask = self.mode & MODE_CH_ENABLE_MASK;

        if (self.mode & MODE_ROTATING_PRIORITY) != 0 {
            // Rotating priority: start after the last serviced channel
            for i in 1..=4 {
                let ch = (self.last_serviced + i) % 4;
                if self.dreq[ch] && (enable_mask & (1 << ch)) != 0 {
                    return Some(ch);
                }
            }
        } else {
            // Fixed priority: channel 0 is highest
            for ch in 0..4 {
                if self.dreq[ch] && (enable_mask & (1 << ch)) != 0 {
                    return Some(ch);
                }
            }
        }
        None
    }

    /// Handle terminal count for a channel: TC Stop and auto-load.
    fn handle_terminal_count(&mut self, ch: usize) {
        if (self.mode & MODE_TC_STOP) != 0 {
            // Disable this channel
            self.mode &= !(1 << ch);
        }

        if ch == 2 && (self.mode & MODE_AUTO_LOAD) != 0 {
            // Reload channel 2 from channel 3's base registers
            self.channels[2].address = self.channels[3].base_address;
            self.channels[2].count = self.channels[3].base_count;
            self.update_flag = true;

            // Re-enable channel 2 if TC Stop disabled it
            if (self.mode & MODE_TC_STOP) != 0 {
                self.mode |= 1 << 2;
            }
        }
    }
}

impl Default for I8257 {
    fn default() -> Self {
        Self::new()
    }
}
