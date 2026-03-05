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
    /// Countdown for chip_select assertion delay. When the timer toggles to
    /// the active phase, chip_select waits this many CPU cycles before
    /// asserting. Compensates for MAME's timeslice scheduling which naturally
    /// gives the Z80 time to process its NMI and write data before the MCU
    /// processes its IRQ.
    chip_select_delay: u32,
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
            chip_select_delay: 0,
            base_divisor,
        }
    }

    /// Read the control register.
    pub fn ctrl_read(&self) -> u8 {
        self.control
    }

    /// Write the control register. Starts or stops the NMI timer based on
    /// the clock divider bits (5-7). `cpu_clock` is the current CPU cycle
    /// count, used to align the initial delay to the next 06XX base clock tick.
    pub fn ctrl_write(&mut self, data: u8, cpu_clock: u64) {
        self.control = data;
        let num_shifts = (data >> 5) & 7;

        if num_shifts == 0 {
            // Divider zero: stop timer. Reset timer_state and clear NMI,
            // matching MAME's ctrl_w_sync which resets m_timer_state=false
            // and calls set_nmi(CLEAR_LINE).
            self.timer_running = false;
            self.timer_state = false;
            self.nmi_pending = false;
        } else {
            // Compute timer half-period in CPU cycles.
            // MAME: attotime::from_hz(clock() / divisor) / 2
            // = (1 << num_shifts) * base_divisor / 2 CPU cycles per toggle.
            let half_period = (self.base_divisor * (1 << num_shifts)) / 2;
            self.timer_period = half_period;

            // Initial delay: align to the next 06XX base clock tick.
            // MAME: from_ticks(total_ticks + 1, clock()) - now
            // This always advances to the NEXT clock edge (1-64 CPU cycles).
            let base = self.base_divisor as u64;
            let initial_delay = (base - (cpu_clock % base)) as u32;
            self.timer_counter = initial_delay;
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
    ///
    /// The NMI output is a **level signal** that follows the two-phase timer,
    /// matching MAME's `nmi_generate` callback which calls `set_nmi(ASSERT)`
    /// on the falling edge (timer_state true) and `set_nmi(CLEAR)` on the
    /// rising edge. The board code uses the Z80's edge detector to convert
    /// this level into discrete NMI events.
    pub fn tick(&mut self) {
        if !self.timer_running {
            return;
        }

        // Count down chip_select delay from previous toggle
        self.chip_select_delay = self.chip_select_delay.saturating_sub(1);

        self.timer_counter = self.timer_counter.saturating_sub(1);
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            self.timer_state = !self.timer_state;

            // Drive NMI output level on every toggle:
            //   falling edge (timer_state true):  ASSERT (unless read_stretch)
            //   rising edge  (timer_state false): CLEAR
            // This matches MAME's nmi_generate exactly.
            self.nmi_pending = self.timer_state && !self.read_stretch;
            self.read_stretch = false;

            // Delay chip_select assertion by one 06XX tick after NMI fires.
            // In MAME, both signals fire simultaneously but the timeslice
            // scheduler gives the Z80 priority, so it processes NMI and writes
            // command data before the MCU processes its IRQ. In our per-cycle
            // model, the MCU wins the race. This delay gives the Z80 one
            // 06XX tick (~64 CPU cycles) of head start.
            if self.timer_state {
                self.chip_select_delay = self.base_divisor;
            }
        }
    }

    /// Returns the current NMI output level (true = asserted).
    ///
    /// This is a level signal, not consumed on read — the Z80's rising-edge
    /// detector handles edge detection. The board code should only propagate
    /// this level to the CPU when it is not halted, matching MAME's
    /// `set_nmi()` which skips suspended CPUs.
    pub fn nmi_output(&self) -> bool {
        self.nmi_pending
    }

    /// Returns true if chip N is selected AND timer is in the active phase
    /// AND the chip_select propagation delay has elapsed.
    pub fn chip_select_active(&self, n: u8) -> bool {
        self.control & (1 << n) != 0 && self.timer_state && self.chip_select_delay == 0
    }

    // Debug accessors
    pub fn timer_running(&self) -> bool {
        self.timer_running
    }
    pub fn timer_counter(&self) -> u32 {
        self.timer_counter
    }
    pub fn timer_period(&self) -> u32 {
        self.timer_period
    }
    pub fn timer_state(&self) -> bool {
        self.timer_state
    }
    pub fn read_stretch(&self) -> bool {
        self.read_stretch
    }

    pub fn reset(&mut self) {
        self.control = 0;
        self.nmi_pending = false;
        self.timer_counter = 0;
        self.timer_period = 0;
        self.timer_running = false;
        self.read_stretch = false;
        self.timer_state = false;
        self.chip_select_delay = 0;
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
