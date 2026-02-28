use phosphor_macros::Saveable;

/// Namco 06XX custom chip — bus arbiter and NMI timer.
///
/// Multiplexes access to up to 4 custom I/O chips (51XX, 53XX, etc.)
/// and generates periodic NMI to the controlling CPU based on a
/// programmable clock divider.
///
/// Control register (written at SEL=1):
///   bits 0-3: chip select (active high, one per custom chip)
///   bit 4:    R/W direction (1 = read from chips, 0 = write to chips)
///   bits 5-7: clock divider (0 = timer stopped, else divides by 1<<N)
#[derive(Saveable)]
#[save_version(1)]
pub struct Namco06 {
    control: u8,
    nmi_pending: bool,
    timer_counter: u32,
    timer_period: u32,
    timer_running: bool,
    read_stretch: bool,
    timer_state: bool,
    /// CPU cycles per 06XX base clock tick (typically 64 = CPU_CLK / 06XX_CLK).
    #[save_skip]
    base_divisor: u32,
}

impl Namco06 {
    pub fn new(base_divisor: u32) -> Self {
        Self {
            control: 0,
            nmi_pending: false,
            timer_counter: 0,
            timer_period: 0,
            timer_running: false,
            read_stretch: false,
            timer_state: false,
            base_divisor,
        }
    }

    /// Read the control register.
    pub fn ctrl_read(&self) -> u8 {
        self.control
    }

    /// Write the control register. Starts or stops the NMI timer based on
    /// the clock divider bits (5-7).
    pub fn ctrl_write(&mut self, data: u8) {
        self.control = data;
        let num_shifts = (data >> 5) & 7;

        if num_shifts == 0 {
            // Divider zero: stop timer, clear NMI and chip selects.
            self.timer_running = false;
            self.nmi_pending = false;
            self.timer_state = false;
        } else {
            // Compute timer half-period in CPU cycles.
            // Full NMI period = base_divisor * (1 << num_shifts).
            // Half-period for the two-phase timer = half of that.
            let half_period = (self.base_divisor * (1 << num_shifts)) / 2;
            self.timer_period = half_period;
            self.timer_counter = half_period;
            self.timer_running = true;

            if data & 0x10 != 0 {
                // Read mode: suppress the first NMI pulse.
                self.nmi_pending = false;
                self.read_stretch = true;
            } else {
                self.read_stretch = false;
            }
        }
    }

    /// Returns true if chip N (0-3) is selected.
    pub fn chip_select(&self, n: u8) -> bool {
        self.control & (1 << n) != 0
    }

    /// Returns true if the control register is in read mode (bit 4 set).
    pub fn is_read_mode(&self) -> bool {
        self.control & 0x10 != 0
    }

    /// Advance the timer by one CPU cycle. Call every CPU cycle.
    pub fn tick(&mut self) {
        if !self.timer_running {
            return;
        }

        self.timer_counter = self.timer_counter.saturating_sub(1);
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            self.timer_state = !self.timer_state;

            // NMI fires on the falling edge (timer_state becomes true),
            // unless suppressed by read_stretch.
            if self.timer_state && !self.read_stretch {
                self.nmi_pending = true;
            }
            self.read_stretch = false;
        }
    }

    /// Returns and clears the NMI pending flag.
    pub fn take_nmi(&mut self) -> bool {
        let pending = self.nmi_pending;
        self.nmi_pending = false;
        pending
    }

    pub fn reset(&mut self) {
        self.control = 0;
        self.nmi_pending = false;
        self.timer_counter = 0;
        self.timer_period = 0;
        self.timer_running = false;
        self.read_stretch = false;
        self.timer_state = false;
    }
}

impl super::Device for Namco06 {
    fn name(&self) -> &'static str {
        "Namco 06XX"
    }
    fn reset(&mut self) {
        self.reset();
    }
}

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for Namco06 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "CTRL",
                value: self.control as u64,
                width: 8,
            },
            DebugRegister {
                name: "NMI",
                value: self.nmi_pending as u64,
                width: 1,
            },
            DebugRegister {
                name: "TIMER",
                value: self.timer_counter as u64,
                width: 16,
            },
        ]
    }
}
