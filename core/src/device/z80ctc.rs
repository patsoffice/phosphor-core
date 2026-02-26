//! Z80 CTC (Counter/Timer Circuit) — 4-channel counter/timer with IM2 interrupts.
//!
//! Used by MCR I/II/III and many other Z80 arcade and computer systems. Each
//! channel operates independently in either timer mode (prescaled CPU clock)
//! or counter mode (external CLK/TRG input). Generates IM2 vectored interrupts
//! with built-in priority (channel 0 highest, channel 3 lowest).
//!
//! # Channel write protocol
//!
//! | Condition | Interpretation |
//! |-----------|----------------|
//! | `waiting_for_tc` is set | Time constant value (1–255, 0 = 256) |
//! | Bit 0 = 1 | Control word |
//! | Bit 0 = 0, channel 0 | Interrupt vector base |
//! | Bit 0 = 0, channel 1–3 | Ignored |
//!
//! # Control word bits
//!
//! | Bit | Name | Description |
//! |-----|------|-------------|
//! | 7 | INT_EN | Interrupt enable |
//! | 6 | MODE | 0 = timer, 1 = counter |
//! | 5 | PRESCALER | Timer mode: 0 = ÷16, 1 = ÷256 |
//! | 4 | EDGE | CLK/TRG edge: 0 = falling, 1 = rising |
//! | 3 | CLK_TRG | Timer mode: 0 = auto-start, 1 = wait for CLK/TRG |
//! | 2 | TC_FOLLOWS | Next write is the time constant |
//! | 1 | RESET | Software reset (stops channel) |
//! | 0 | CONTROL | Always 1 for control words |

// Control word bit masks
const CONTROL_WORD: u8 = 0x01;
const RESET: u8 = 0x02;
const TC_FOLLOWS: u8 = 0x04;
const CLK_TRIGGER: u8 = 0x08; // Timer: 0=auto, 1=wait for CLK/TRG
const EDGE_RISING: u8 = 0x10;
const PRESCALER_256: u8 = 0x20;
const COUNTER_MODE: u8 = 0x40;
const INTERRUPT_EN: u8 = 0x80;

/// Z80 CTC — 4-channel Counter/Timer Circuit.
pub struct Z80Ctc {
    channels: [CtcChannel; 4],
    vector_base: u8,
}

#[derive(Default)]
struct CtcChannel {
    control: u8,
    time_constant: u16, // 1–256 (TC byte 0 maps to 256)
    down_counter: u16,
    prescaler_count: u16,
    waiting_for_tc: bool,
    running: bool,             // Timer is actively counting (timer mode only)
    waiting_for_trigger: bool, // Timer loaded, waiting for CLK/TRG edge
    trigger_state: bool,       // Previous CLK/TRG input for edge detection
    zc_pulse: bool,            // ZC/TO output pulse (one tick on zero-count)
    interrupt_pending: bool,
}

impl Z80Ctc {
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            vector_base: 0,
        }
    }

    /// Read the current down counter value for a channel.
    pub fn read(&self, channel: u8) -> u8 {
        self.channels[channel as usize & 3].down_counter as u8
    }

    /// Write to a CTC channel. Interprets the byte as a time constant,
    /// control word, or vector base depending on channel state.
    pub fn write(&mut self, channel: u8, data: u8) {
        let ch_idx = channel as usize & 3;

        // If waiting for time constant, this byte is the TC value
        if self.channels[ch_idx].waiting_for_tc {
            self.load_time_constant(ch_idx, data);
            return;
        }

        // Bit 0 = 1: control word
        if data & CONTROL_WORD != 0 {
            self.write_control(ch_idx, data);
            return;
        }

        // Bit 0 = 0 on channel 0: interrupt vector base (low 3 bits ignored)
        if ch_idx == 0 {
            self.vector_base = data & 0xF8;
        }
    }

    /// Advance all timer-mode channels by one CPU clock cycle.
    ///
    /// After calling tick(), check `zc_output()` to detect zero-count pulses
    /// for cascading (e.g. channel 0 ZC → channel 1 trigger).
    pub fn tick(&mut self) {
        // Clear previous ZC pulses
        for ch in &mut self.channels {
            ch.zc_pulse = false;
        }

        for ch in &mut self.channels {
            // Only process timer-mode channels that are actively running
            if ch.control & COUNTER_MODE != 0 || !ch.running {
                continue;
            }

            ch.prescaler_count -= 1;
            if ch.prescaler_count == 0 {
                ch.prescaler_count = if ch.control & PRESCALER_256 != 0 {
                    256
                } else {
                    16
                };

                ch.down_counter -= 1;
                if ch.down_counter == 0 {
                    ch.down_counter = ch.time_constant;
                    if ch.control & INTERRUPT_EN != 0 {
                        ch.interrupt_pending = true;
                    }
                    ch.zc_pulse = true;
                }
            }
        }
    }

    /// Apply an external trigger signal to a channel's CLK/TRG input.
    ///
    /// In counter mode, detected edges decrement the counter.
    /// In timer mode (CLK/TRG trigger), the first detected edge starts the timer.
    pub fn trigger(&mut self, channel: u8, state: bool) {
        let ch_idx = channel as usize & 3;
        let ch = &mut self.channels[ch_idx];

        let prev = ch.trigger_state;
        ch.trigger_state = state;

        // Edge detection
        let edge = if ch.control & EDGE_RISING != 0 {
            !prev && state // Rising edge
        } else {
            prev && !state // Falling edge
        };

        if !edge {
            return;
        }

        if ch.control & COUNTER_MODE != 0 {
            // Counter mode: edge decrements counter
            if ch.waiting_for_tc || ch.down_counter == 0 {
                return;
            }
            ch.down_counter -= 1;
            if ch.down_counter == 0 {
                ch.down_counter = ch.time_constant;
                if ch.control & INTERRUPT_EN != 0 {
                    ch.interrupt_pending = true;
                }
                ch.zc_pulse = true;
            }
        } else if ch.waiting_for_trigger {
            // Timer mode: edge starts the timer
            ch.waiting_for_trigger = false;
            ch.running = true;
            ch.prescaler_count = if ch.control & PRESCALER_256 != 0 {
                256
            } else {
                16
            };
        }
    }

    /// Check if any channel has a pending interrupt.
    pub fn interrupt_pending(&self) -> bool {
        self.channels.iter().any(|ch| ch.interrupt_pending)
    }

    /// Return the IM2 vector for the highest-priority pending interrupt.
    ///
    /// Channel 0 has highest priority, channel 3 lowest.
    /// Vector = base | (channel × 2).
    pub fn interrupt_vector(&self) -> u8 {
        for (i, ch) in self.channels.iter().enumerate() {
            if ch.interrupt_pending {
                return self.vector_base | (i as u8 * 2);
            }
        }
        self.vector_base
    }

    /// Acknowledge the highest-priority pending interrupt (clears pending flag).
    pub fn acknowledge_interrupt(&mut self) {
        for ch in &mut self.channels {
            if ch.interrupt_pending {
                ch.interrupt_pending = false;
                return;
            }
        }
    }

    /// Read the ZC/TO output pulse state for a channel.
    ///
    /// Returns true for one tick when the channel's counter reaches zero.
    /// Use this for cascading: feed the pulse to another channel's `trigger()`.
    pub fn zc_output(&self, channel: u8) -> bool {
        self.channels[channel as usize & 3].zc_pulse
    }

    /// Reset all channels and the vector base.
    pub fn reset(&mut self) {
        for ch in &mut self.channels {
            *ch = CtcChannel::default();
        }
        self.vector_base = 0;
    }

    // -- Private methods -------------------------------------------------------

    fn write_control(&mut self, ch_idx: usize, data: u8) {
        let ch = &mut self.channels[ch_idx];

        // Handle mode change from timer to counter without reset (MAME edge case)
        if ch.control & COUNTER_MODE == 0 && data & COUNTER_MODE != 0 && data & RESET == 0 {
            ch.running = false;
        }

        // Software reset: stops timer but does NOT clear interrupt pending
        if data & RESET != 0 {
            ch.running = false;
            ch.waiting_for_trigger = false;
            ch.zc_pulse = false;
        }

        // Disabling interrupts clears any pending INT
        if data & INTERRUPT_EN == 0 && ch.interrupt_pending {
            ch.interrupt_pending = false;
        }

        ch.control = data;

        if data & TC_FOLLOWS != 0 {
            ch.waiting_for_tc = true;
        }
    }

    fn load_time_constant(&mut self, ch_idx: usize, data: u8) {
        let ch = &mut self.channels[ch_idx];
        ch.time_constant = if data == 0 { 256 } else { data as u16 };
        ch.down_counter = ch.time_constant;
        ch.waiting_for_tc = false;

        // Loading TC auto-clears the RESET flag (per MAME)
        ch.control &= !RESET;

        if ch.control & COUNTER_MODE != 0 {
            // Counter mode: ready to count on trigger edges
            ch.running = false;
            ch.waiting_for_trigger = false;
        } else if ch.control & CLK_TRIGGER != 0 {
            // Timer mode, CLK/TRG trigger: wait for external edge
            ch.running = false;
            ch.waiting_for_trigger = true;
        } else {
            // Timer mode, auto-trigger: start immediately
            ch.prescaler_count = if ch.control & PRESCALER_256 != 0 {
                256
            } else {
                16
            };
            ch.running = true;
            ch.waiting_for_trigger = false;
        }
    }
}

impl Default for Z80Ctc {
    fn default() -> Self {
        Self::new()
    }
}

// -- Debug support -----------------------------------------------------------

use crate::core::debug::{DebugRegister, Debuggable};

impl Debuggable for Z80Ctc {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "VECTOR",
                value: self.vector_base as u64,
                width: 8,
            },
            DebugRegister {
                name: "CTR0",
                value: self.channels[0].down_counter as u64,
                width: 16,
            },
            DebugRegister {
                name: "CTL0",
                value: self.channels[0].control as u64,
                width: 8,
            },
            DebugRegister {
                name: "CTR1",
                value: self.channels[1].down_counter as u64,
                width: 16,
            },
            DebugRegister {
                name: "CTL1",
                value: self.channels[1].control as u64,
                width: 8,
            },
            DebugRegister {
                name: "CTR2",
                value: self.channels[2].down_counter as u64,
                width: 16,
            },
            DebugRegister {
                name: "CTL2",
                value: self.channels[2].control as u64,
                width: 8,
            },
            DebugRegister {
                name: "CTR3",
                value: self.channels[3].down_counter as u64,
                width: 16,
            },
            DebugRegister {
                name: "CTL3",
                value: self.channels[3].control as u64,
                width: 8,
            },
        ]
    }
}

// -- Save state support ------------------------------------------------------

use crate::core::save_state::{SaveError, Saveable, StateReader, StateWriter};

impl Saveable for Z80Ctc {
    fn save_state(&self, w: &mut StateWriter) {
        w.write_u8(self.vector_base);
        for ch in &self.channels {
            w.write_u8(ch.control);
            w.write_u16_le(ch.time_constant);
            w.write_u16_le(ch.down_counter);
            w.write_u16_le(ch.prescaler_count);
            w.write_bool(ch.waiting_for_tc);
            w.write_bool(ch.running);
            w.write_bool(ch.waiting_for_trigger);
            w.write_bool(ch.trigger_state);
            w.write_bool(ch.zc_pulse);
            w.write_bool(ch.interrupt_pending);
        }
    }

    fn load_state(&mut self, r: &mut StateReader) -> Result<(), SaveError> {
        self.vector_base = r.read_u8()?;
        for ch in &mut self.channels {
            ch.control = r.read_u8()?;
            ch.time_constant = r.read_u16_le()?;
            ch.down_counter = r.read_u16_le()?;
            ch.prescaler_count = r.read_u16_le()?;
            ch.waiting_for_tc = r.read_bool()?;
            ch.running = r.read_bool()?;
            ch.waiting_for_trigger = r.read_bool()?;
            ch.trigger_state = r.read_bool()?;
            ch.zc_pulse = r.read_bool()?;
            ch.interrupt_pending = r.read_bool()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state() {
        let ctc = Z80Ctc::new();
        assert_eq!(ctc.vector_base, 0);
        assert!(!ctc.interrupt_pending());
        for i in 0..4 {
            assert_eq!(ctc.read(i), 0);
            assert!(!ctc.zc_output(i));
        }
    }

    #[test]
    fn vector_base_write() {
        let mut ctc = Z80Ctc::new();
        // Write vector base to channel 0 (bit 0 = 0)
        ctc.write(0, 0xE0);
        assert_eq!(ctc.vector_base, 0xE0);

        // Low 3 bits are masked off
        ctc.write(0, 0xFE);
        assert_eq!(ctc.vector_base, 0xF8);
    }

    #[test]
    fn vector_write_ignored_on_non_channel_0() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, 0xE0); // Set base on channel 0
        ctc.write(1, 0xA0); // Should be ignored (not channel 0, not control word)
        assert_eq!(ctc.vector_base, 0xE0);
    }

    #[test]
    fn timer_mode_auto_trigger() {
        let mut ctc = Z80Ctc::new();

        // Configure channel 0: timer mode, prescaler 16, auto-trigger, interrupt enable
        // Bit 3 = 0 means auto-trigger
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | INTERRUPT_EN);
        ctc.write(0, 2); // Time constant = 2

        assert_eq!(ctc.channels[0].down_counter, 2);
        assert!(ctc.channels[0].running);

        // After 16 ticks (prescaler), counter decrements 2 → 1
        for _ in 0..16 {
            ctc.tick();
        }
        assert_eq!(ctc.channels[0].down_counter, 1);
        assert!(!ctc.interrupt_pending());

        // After 16 more ticks, counter 1 → 0 → reload to 2, interrupt fires
        for _ in 0..16 {
            ctc.tick();
        }
        assert_eq!(ctc.channels[0].down_counter, 2);
        assert!(ctc.interrupt_pending());
        assert!(ctc.zc_output(0)); // ZC pulse active
    }

    #[test]
    fn zc_pulse_lasts_one_tick() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS);
        ctc.write(0, 1); // TC = 1, auto-trigger, prescaler 16

        // After 16 ticks, counter fires
        for _ in 0..16 {
            ctc.tick();
        }
        assert!(ctc.zc_output(0));

        // On the next tick, pulse is cleared
        ctc.tick();
        assert!(!ctc.zc_output(0));
    }

    #[test]
    fn timer_mode_prescaler_256() {
        let mut ctc = Z80Ctc::new();

        // Prescaler 256, auto-trigger, TC = 1
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | PRESCALER_256);
        ctc.write(0, 1);

        // Should fire after 256 ticks (prescaler 256 × TC 1)
        for _ in 0..255 {
            ctc.tick();
        }
        assert!(!ctc.zc_output(0));

        ctc.tick(); // 256th tick
        assert!(ctc.zc_output(0));
    }

    #[test]
    fn counter_mode_rising_edge() {
        let mut ctc = Z80Ctc::new();

        // Counter mode, rising edge, interrupt enable
        ctc.write(
            0,
            CONTROL_WORD | TC_FOLLOWS | COUNTER_MODE | EDGE_RISING | INTERRUPT_EN,
        );
        ctc.write(0, 3); // TC = 3

        // Trigger 3 rising edges: counter 3 → 2 → 1 → 0 (reload)
        for _ in 0..3 {
            ctc.trigger(0, true);
            ctc.trigger(0, false);
        }

        assert_eq!(ctc.channels[0].down_counter, 3); // Reloaded
        assert!(ctc.interrupt_pending());
        assert!(ctc.zc_output(0));
    }

    #[test]
    fn counter_mode_falling_edge() {
        let mut ctc = Z80Ctc::new();

        // Counter mode, falling edge (EDGE_RISING not set)
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | COUNTER_MODE);
        ctc.write(0, 2);

        // Rising edge should NOT count
        ctc.trigger(0, true);
        assert_eq!(ctc.channels[0].down_counter, 2);

        // Falling edge SHOULD count
        ctc.trigger(0, false);
        assert_eq!(ctc.channels[0].down_counter, 1);
    }

    #[test]
    fn interrupt_priority() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, 0xE0); // Vector base = 0xE0

        // Set pending interrupts on channels 1 and 2
        ctc.channels[2].interrupt_pending = true;
        ctc.channels[1].interrupt_pending = true;

        // Channel 1 has higher priority
        assert_eq!(ctc.interrupt_vector(), 0xE0 | 2); // 0xE2

        // Acknowledge clears channel 1
        ctc.acknowledge_interrupt();
        assert!(!ctc.channels[1].interrupt_pending);
        assert!(ctc.channels[2].interrupt_pending);

        // Now channel 2 is highest priority
        assert_eq!(ctc.interrupt_vector(), 0xE0 | 4); // 0xE4
    }

    #[test]
    fn timer_mode_wait_for_trigger() {
        let mut ctc = Z80Ctc::new();

        // Timer mode, CLK/TRG trigger (bit 3 = 1), rising edge
        ctc.write(
            0,
            CONTROL_WORD | TC_FOLLOWS | CLK_TRIGGER | EDGE_RISING | INTERRUPT_EN,
        );
        ctc.write(0, 1); // TC = 1

        assert!(!ctc.channels[0].running);
        assert!(ctc.channels[0].waiting_for_trigger);

        // Ticking shouldn't do anything yet
        for _ in 0..100 {
            ctc.tick();
        }
        assert!(!ctc.interrupt_pending());

        // Rising edge starts the timer
        ctc.trigger(0, true);
        assert!(ctc.channels[0].running);
        assert!(!ctc.channels[0].waiting_for_trigger);

        // Now ticking should work: prescaler 16, TC 1 → fires after 16 ticks
        for _ in 0..16 {
            ctc.tick();
        }
        assert!(ctc.interrupt_pending());
    }

    #[test]
    fn software_reset_stops_timer() {
        let mut ctc = Z80Ctc::new();

        // Start a timer
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | INTERRUPT_EN);
        ctc.write(0, 10);
        assert!(ctc.channels[0].running);

        // Reset the channel
        ctc.write(0, CONTROL_WORD | RESET);
        assert!(!ctc.channels[0].running);

        // Ticking should not cause interrupts
        for _ in 0..1000 {
            ctc.tick();
        }
        assert!(!ctc.interrupt_pending());
    }

    #[test]
    fn reset_does_not_clear_interrupt_pending() {
        let mut ctc = Z80Ctc::new();
        ctc.channels[0].interrupt_pending = true;

        // Reset with interrupts still enabled
        ctc.write(0, CONTROL_WORD | RESET | INTERRUPT_EN);

        // Interrupt should still be pending (reset doesn't clear it)
        assert!(ctc.channels[0].interrupt_pending);
    }

    #[test]
    fn disabling_interrupts_clears_pending() {
        let mut ctc = Z80Ctc::new();
        ctc.channels[0].interrupt_pending = true;

        // Write control word with INTERRUPT_EN = 0
        ctc.write(0, CONTROL_WORD | RESET);

        // Pending interrupt should be cleared
        assert!(!ctc.channels[0].interrupt_pending);
    }

    #[test]
    fn tc_zero_means_256() {
        let mut ctc = Z80Ctc::new();

        ctc.write(0, CONTROL_WORD | TC_FOLLOWS);
        ctc.write(0, 0); // TC = 0 → 256
        assert_eq!(ctc.channels[0].down_counter, 256);
        assert_eq!(ctc.channels[0].time_constant, 256);
    }

    #[test]
    fn read_returns_current_count() {
        let mut ctc = Z80Ctc::new();

        ctc.write(0, CONTROL_WORD | TC_FOLLOWS);
        ctc.write(0, 100);
        assert_eq!(ctc.read(0), 100);

        // After one prescaler period (16 ticks), counter decrements
        for _ in 0..16 {
            ctc.tick();
        }
        assert_eq!(ctc.read(0), 99);
    }

    #[test]
    fn tc_load_clears_reset() {
        let mut ctc = Z80Ctc::new();

        // Set reset + TC follows
        ctc.write(0, CONTROL_WORD | RESET | TC_FOLLOWS);
        assert!(ctc.channels[0].control & RESET != 0);

        // Load TC: should auto-clear RESET
        ctc.write(0, 5);
        assert_eq!(ctc.channels[0].control & RESET, 0);
    }

    #[test]
    fn cascading_zc_to_trigger() {
        let mut ctc = Z80Ctc::new();

        // Channel 0: timer mode, auto-trigger, TC=1, prescaler 16
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | INTERRUPT_EN);
        ctc.write(0, 1);

        // Channel 1: counter mode, rising edge, TC=2
        ctc.write(
            1,
            CONTROL_WORD | TC_FOLLOWS | COUNTER_MODE | EDGE_RISING | INTERRUPT_EN,
        );
        ctc.write(1, 2);

        // Run 16 ticks → channel 0 fires ZC
        for _ in 0..16 {
            ctc.tick();
        }
        assert!(ctc.zc_output(0));

        // Cascade: feed ZC pulse to channel 1's trigger
        ctc.trigger(1, true);
        ctc.trigger(1, false);
        assert_eq!(ctc.channels[1].down_counter, 1); // 2 → 1

        // Another 16 ticks → channel 0 fires again
        for _ in 0..16 {
            ctc.tick();
        }

        // Cascade again
        ctc.trigger(1, true);
        ctc.trigger(1, false);
        assert_eq!(ctc.channels[1].down_counter, 2); // 1 → 0 → reload
        assert!(ctc.channels[1].interrupt_pending);
    }

    #[test]
    fn save_load_round_trip() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, 0xE0); // Vector
        ctc.write(0, CONTROL_WORD | TC_FOLLOWS | INTERRUPT_EN);
        ctc.write(0, 50);
        for _ in 0..100 {
            ctc.tick();
        }

        let mut w = StateWriter::new();
        ctc.save_state(&mut w);
        let data = w.into_vec();

        let mut ctc2 = Z80Ctc::new();
        let mut r = StateReader::new(&data);
        ctc2.load_state(&mut r).unwrap();

        assert_eq!(ctc2.vector_base, ctc.vector_base);
        for i in 0..4 {
            assert_eq!(ctc2.channels[i].control, ctc.channels[i].control);
            assert_eq!(ctc2.channels[i].down_counter, ctc.channels[i].down_counter);
            assert_eq!(ctc2.channels[i].running, ctc.channels[i].running);
            assert_eq!(
                ctc2.channels[i].prescaler_count,
                ctc.channels[i].prescaler_count
            );
        }
    }
}
