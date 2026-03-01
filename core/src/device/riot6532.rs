//! MOS 6532 RIOT (RAM-I/O-Timer) device.
//!
//! 128 bytes of static RAM, two 8-bit bidirectional I/O ports with data
//! direction registers, a programmable interval timer with 4 prescaler
//! options, and PA7 edge detection with interrupt generation.

use phosphor_macros::Saveable;

/// Timer prescaler shift values for ÷1, ÷8, ÷64, ÷1024.
const PRESCALE_SHIFT: [u8; 4] = [0, 3, 6, 10];

#[derive(Saveable)]
#[save_version(1)]
pub struct Riot6532 {
    // 128 bytes of internal RAM
    ram: [u8; 128],

    // Port A
    pa_out: u8,
    pa_ddr: u8,
    pa_in: u8,

    // Port B
    pb_out: u8,
    pb_ddr: u8,
    pb_in: u8,

    // Timer
    timer: u8,
    prescale_shift: u8,
    prescale_counter: u16,
    timer_running: bool, // false after underflow (counts at ÷1)

    // Interrupt state
    ie_timer: bool,
    irq_timer: bool,
    ie_edge: bool,
    irq_edge: bool,

    // PA7 edge detection
    pa7_dir: bool,  // true = positive edge, false = negative edge
    pa7_prev: bool, // previous PA7 state
}

impl Default for Riot6532 {
    fn default() -> Self {
        Self::new()
    }
}

impl Riot6532 {
    pub fn new() -> Self {
        Self {
            ram: [0; 128],
            pa_out: 0,
            pa_ddr: 0,
            pa_in: 0xFF,
            pb_out: 0,
            pb_ddr: 0,
            pb_in: 0xFF,
            timer: 0xFF,
            prescale_shift: 10, // ÷1024 default
            prescale_counter: 0,
            timer_running: false,
            ie_timer: false,
            irq_timer: false,
            ie_edge: false,
            irq_edge: false,
            pa7_dir: false,
            pa7_prev: false,
        }
    }

    /// Read from the RIOT RAM space (offset 0x00-0x7F).
    pub fn read_ram(&self, offset: u8) -> u8 {
        self.ram[(offset & 0x7F) as usize]
    }

    /// Write to the RIOT RAM space (offset 0x00-0x7F).
    pub fn write_ram(&mut self, offset: u8, data: u8) {
        self.ram[(offset & 0x7F) as usize] = data;
    }

    /// Read from the RIOT I/O register space (offset 0x00-0x1F).
    pub fn read_io(&mut self, offset: u8) -> u8 {
        let offset = offset & 0x1F;

        // Bit 2 (A2) distinguishes port registers from timer/IRQ
        if offset & 0x04 == 0 {
            // Port registers: A2=0
            match offset & 0x03 {
                0 => {
                    // Port A data: output bits from pa_out, input bits from pa_in
                    (self.pa_out & self.pa_ddr) | (self.pa_in & !self.pa_ddr)
                }
                1 => self.pa_ddr,
                2 => {
                    // Port B data
                    (self.pb_out & self.pb_ddr) | (self.pb_in & !self.pb_ddr)
                }
                3 => self.pb_ddr,
                _ => unreachable!(),
            }
        } else {
            // Timer / IRQ flags: A2=1
            // Bit 0 (A0) distinguishes timer read from IRQ flags read
            if offset & 0x01 == 0 {
                // Read timer (even offsets: 0x04, 0x06, 0x0C, 0x0E, 0x14, 0x16, 0x1C, 0x1E)
                // Bit 3 (A3): 0=disable timer IRQ, 1=enable timer IRQ
                self.ie_timer = offset & 0x08 != 0;
                self.irq_timer = false;
                self.timer
            } else {
                // Read IRQ flags (odd offsets: 0x05, 0x07, 0x0D, 0x0F, 0x15, 0x17, 0x1D, 0x1F)
                let flags =
                    if self.irq_timer { 0x80 } else { 0 } | if self.irq_edge { 0x40 } else { 0 };
                // Reading clears the edge flag
                self.irq_edge = false;
                flags
            }
        }
    }

    /// Write to the RIOT I/O register space (offset 0x00-0x1F).
    pub fn write_io(&mut self, offset: u8, data: u8) {
        let offset = offset & 0x1F;

        // Bit 2 (A2) distinguishes port registers from timer/edge
        if offset & 0x04 == 0 {
            // Port registers: A2=0
            match offset & 0x03 {
                0 => {
                    self.pa_out = data;
                    self.update_pa7();
                }
                1 => {
                    self.pa_ddr = data;
                    self.update_pa7();
                }
                2 => self.pb_out = data,
                3 => self.pb_ddr = data,
                _ => unreachable!(),
            }
        } else if offset & 0x10 == 0 {
            // Edge detect config: A4=0, A2=1 (offsets 0x04-0x07, 0x0C-0x0F)
            // A0 = edge direction (0=negative, 1=positive)
            // A1 = interrupt enable
            self.pa7_dir = offset & 0x01 != 0;
            self.ie_edge = offset & 0x02 != 0;
        } else {
            // Timer write: A4=1, A2=1 (offsets 0x14-0x17, 0x1C-0x1F)
            // A0-A1 = prescaler select
            // A3 = interrupt enable
            self.prescale_shift = PRESCALE_SHIFT[(offset & 0x03) as usize];
            self.ie_timer = offset & 0x08 != 0;
            self.timer = data;
            self.prescale_counter = 0;
            self.timer_running = true;
            self.irq_timer = false;
        }
    }

    /// Advance the timer by one clock tick. Call at the RIOT's clock rate.
    pub fn tick(&mut self) {
        if self.timer_running {
            // Prescaled countdown
            self.prescale_counter += 1;
            if self.prescale_counter >= (1u16 << self.prescale_shift) {
                self.prescale_counter = 0;
                if self.timer == 0 {
                    // Underflow: set IRQ, switch to ÷1 mode
                    self.irq_timer = true;
                    self.timer_running = false;
                    self.timer = 0xFF;
                } else {
                    self.timer -= 1;
                }
            }
        } else {
            // After underflow: count down at ÷1 rate (spinning)
            self.timer = self.timer.wrapping_sub(1);
        }
    }

    /// Set external input on Port A (bits driven by external hardware).
    pub fn set_pa_input(&mut self, data: u8) {
        self.pa_in = data;
        self.update_pa7();
    }

    /// Set external input on Port A with mask (only update bits where mask=1).
    pub fn set_pa_input_masked(&mut self, data: u8, mask: u8) {
        self.pa_in = (self.pa_in & !mask) | (data & mask);
        self.update_pa7();
    }

    /// Set external input on Port B.
    pub fn set_pb_input(&mut self, data: u8) {
        self.pb_in = data;
    }

    /// Set external input on Port B with mask (only update bits where mask=1).
    pub fn set_pb_input_masked(&mut self, data: u8, mask: u8) {
        self.pb_in = (self.pb_in & !mask) | (data & mask);
    }

    /// Read the current Port A output pin state.
    pub fn pa_output(&self) -> u8 {
        (self.pa_out & self.pa_ddr) | (self.pa_in & !self.pa_ddr)
    }

    /// Read the current Port B output pin state.
    pub fn pb_output(&self) -> u8 {
        (self.pb_out & self.pb_ddr) | (self.pb_in & !self.pb_ddr)
    }

    /// Check if the IRQ output is asserted.
    pub fn irq_active(&self) -> bool {
        (self.ie_timer && self.irq_timer) || (self.ie_edge && self.irq_edge)
    }

    /// Update PA7 edge detection after any change to PA.
    fn update_pa7(&mut self) {
        let pa_data = (self.pa_out & self.pa_ddr) | (self.pa_in & !self.pa_ddr);
        let pa7 = pa_data & 0x80 != 0;

        // Detect edge: state changed AND matches configured direction
        if pa7 != self.pa7_prev && pa7 == self.pa7_dir {
            self.irq_edge = true;
        }
        self.pa7_prev = pa7;
    }

    pub fn reset(&mut self) {
        self.pa_out = 0;
        self.pa_ddr = 0;
        self.pb_out = 0;
        self.pb_ddr = 0;
        self.timer = 0xFF;
        self.prescale_shift = 10;
        self.prescale_counter = 0;
        self.timer_running = false;
        self.ie_timer = false;
        self.irq_timer = false;
        self.ie_edge = false;
        self.irq_edge = false;
        self.pa7_dir = false;
        self.pa7_prev = false;
        // RAM is not cleared on reset
    }
}

impl super::Device for Riot6532 {
    fn name(&self) -> &'static str {
        "6532 RIOT"
    }
    fn reset(&mut self) {
        self.reset();
    }
    fn tick(&mut self) {
        self.tick();
    }
}

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for Riot6532 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "PA_OUT",
                value: self.pa_out as u64,
                width: 8,
            },
            DebugRegister {
                name: "PA_DDR",
                value: self.pa_ddr as u64,
                width: 8,
            },
            DebugRegister {
                name: "PA_IN",
                value: self.pa_in as u64,
                width: 8,
            },
            DebugRegister {
                name: "PB_OUT",
                value: self.pb_out as u64,
                width: 8,
            },
            DebugRegister {
                name: "PB_DDR",
                value: self.pb_ddr as u64,
                width: 8,
            },
            DebugRegister {
                name: "PB_IN",
                value: self.pb_in as u64,
                width: 8,
            },
            DebugRegister {
                name: "TIMER",
                value: self.timer as u64,
                width: 8,
            },
            DebugRegister {
                name: "PSC_SHIFT",
                value: self.prescale_shift as u64,
                width: 8,
            },
            DebugRegister {
                name: "IRQ",
                value: u64::from(self.irq_active()),
                width: 1,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_read_write() {
        let mut riot = Riot6532::new();
        riot.write_ram(0x00, 0xAB);
        riot.write_ram(0x7F, 0xCD);
        assert_eq!(riot.read_ram(0x00), 0xAB);
        assert_eq!(riot.read_ram(0x7F), 0xCD);
        // Mirroring: bit 7 is masked
        assert_eq!(riot.read_ram(0x80), 0xAB);
    }

    #[test]
    fn port_a_ddr() {
        let mut riot = Riot6532::new();
        // Set PA0-3 as output, PA4-7 as input
        riot.write_io(0x01, 0x0F); // DDR
        riot.write_io(0x00, 0xAB); // Output data
        riot.set_pa_input(0xF0); // External input on PA4-7

        // Reading PA: output bits from pa_out, input bits from pa_in
        let val = riot.read_io(0x00);
        assert_eq!(val, (0xAB & 0x0F) | (0xF0 & 0xF0));
    }

    #[test]
    fn port_b_ddr() {
        let mut riot = Riot6532::new();
        riot.write_io(0x03, 0xFF); // All output
        riot.write_io(0x02, 0x42); // Output data
        assert_eq!(riot.read_io(0x02), 0x42);
    }

    #[test]
    fn timer_div1() {
        let mut riot = Riot6532::new();
        // Write timer with ÷1 prescaler (offset 0x14, A0-A1=00)
        riot.write_io(0x14, 3); // Count down from 3
        assert_eq!(riot.timer, 3);
        assert!(riot.timer_running);

        riot.tick(); // 3→2
        assert_eq!(riot.timer, 2);
        riot.tick(); // 2→1
        assert_eq!(riot.timer, 1);
        riot.tick(); // 1→0
        assert_eq!(riot.timer, 0);
        assert!(!riot.irq_timer);
        riot.tick(); // 0→underflow
        assert!(riot.irq_timer);
        assert!(!riot.timer_running);
        assert_eq!(riot.timer, 0xFF);
    }

    #[test]
    fn timer_div8() {
        let mut riot = Riot6532::new();
        // Write timer with ÷8 prescaler (offset 0x15, A0-A1=01)
        riot.write_io(0x15, 1); // Count down from 1

        // Should take 8 ticks to decrement once (1→0)
        for _ in 0..8 {
            assert!(!riot.irq_timer);
            riot.tick();
        }
        assert_eq!(riot.timer, 0);

        // 8 more ticks for underflow (0→underflow)
        for _ in 0..8 {
            assert!(!riot.irq_timer);
            riot.tick();
        }
        assert!(riot.irq_timer);
    }

    #[test]
    fn timer_irq_enable() {
        let mut riot = Riot6532::new();
        // Write timer with IRQ enabled (offset 0x1C, A3=1)
        riot.write_io(0x1C, 0); // Immediate underflow on first tick
        assert!(riot.ie_timer);
        riot.tick();
        assert!(riot.irq_timer);
        assert!(riot.irq_active());

        // Read timer to clear IRQ (offset 0x04, disable IE)
        riot.read_io(0x04);
        assert!(!riot.irq_timer);
        assert!(!riot.ie_timer);
        assert!(!riot.irq_active());
    }

    #[test]
    fn irq_flags_read() {
        let mut riot = Riot6532::new();
        // Trigger timer IRQ
        riot.write_io(0x14, 0);
        riot.tick();
        assert!(riot.irq_timer);

        // Read IRQ flags (offset 0x05)
        let flags = riot.read_io(0x05);
        assert_eq!(flags & 0x80, 0x80); // Timer flag
        assert_eq!(flags & 0x40, 0x00); // No edge flag
    }

    #[test]
    fn pa7_positive_edge() {
        let mut riot = Riot6532::new();
        // Configure positive edge detection with IRQ (write to 0x07: A0=1, A1=1)
        riot.write_io(0x07, 0);
        assert!(riot.pa7_dir); // positive edge
        assert!(riot.ie_edge);

        // PA7 starts low (pa_in default 0xFF but DDR=0 means all input)
        // Actually pa_in=0xFF means PA7 is high. Let's set it low first.
        riot.set_pa_input(0x00); // PA7 = 0
        assert!(!riot.irq_edge); // no edge yet (went low, we want positive)

        // Now set PA7 high → positive edge
        riot.set_pa_input(0x80);
        assert!(riot.irq_edge);
        assert!(riot.irq_active());

        // Read IRQ flags clears edge flag
        riot.read_io(0x05);
        assert!(!riot.irq_edge);
    }

    #[test]
    fn pa7_negative_edge() {
        let mut riot = Riot6532::new();
        // Configure negative edge detection (write to 0x06: A0=0, A1=1)
        riot.write_io(0x06, 0);
        assert!(!riot.pa7_dir); // negative edge
        assert!(riot.ie_edge);

        // Start with PA7 high
        riot.set_pa_input(0x80);
        assert!(!riot.irq_edge);

        // PA7 goes low → negative edge
        riot.set_pa_input(0x00);
        assert!(riot.irq_edge);
        assert!(riot.irq_active());
    }

    #[test]
    fn masked_pa_input() {
        let mut riot = Riot6532::new();
        riot.set_pa_input(0x00);
        // Update only bits 0-5 and 7, leave bit 6 unchanged
        riot.set_pa_input_masked(0xBF, 0xBF);
        assert_eq!(riot.pa_in, 0xBF & 0xBF); // bit 6 stays 0
    }

    #[test]
    fn reset_preserves_ram() {
        let mut riot = Riot6532::new();
        riot.write_ram(0x10, 0x42);
        riot.reset();
        assert_eq!(riot.read_ram(0x10), 0x42); // RAM preserved
        assert_eq!(riot.timer, 0xFF); // Timer reset
        assert!(!riot.irq_timer);
    }

    #[test]
    fn timer_spinning_after_underflow() {
        let mut riot = Riot6532::new();
        riot.write_io(0x14, 0); // ÷1, start at 0
        riot.tick(); // underflow → 0xFF
        assert!(!riot.timer_running);

        // Spinning: counts down at ÷1 regardless of original prescaler
        riot.tick();
        assert_eq!(riot.timer, 0xFE);
        riot.tick();
        assert_eq!(riot.timer, 0xFD);
    }
}
