/// Describes a single input button that a machine accepts.
pub struct InputButton {
    /// Machine-defined button identifier, passed to `set_input()`.
    pub id: u8,
    /// Human-readable name for display/configuration (e.g., "P1 Left", "Coin").
    pub name: &'static str,
}

/// Describes an analog axis that a machine accepts (trackball, spinner, etc.).
pub struct AnalogInput {
    /// Machine-defined axis identifier, passed to `set_analog()`.
    pub id: u8,
    /// Human-readable name for display/configuration (e.g., "Trackball X").
    pub name: &'static str,
}

use super::debug::BusDebug;
use super::memory_map::{MemoryMap, WatchpointHit, WatchpointKind};
use super::save_state::SaveError;

// ---------------------------------------------------------------------------
// Timing configuration
// ---------------------------------------------------------------------------

/// Timing and display configuration for an emulated machine.
///
/// Provides a single source of truth for CPU clock rate, scanline timing,
/// and display dimensions. Derived values ([`cycles_per_frame`](Self::cycles_per_frame),
/// [`frame_rate_hz`](Self::frame_rate_hz)) are computed from these fields to
/// prevent inconsistencies.
pub struct TimingConfig {
    pub cpu_clock_hz: u64,
    pub cycles_per_scanline: u64,
    pub total_scanlines: u64,
    pub display_width: u32,
    pub display_height: u32,
}

impl TimingConfig {
    pub const fn cycles_per_frame(&self) -> u64 {
        self.total_scanlines * self.cycles_per_scanline
    }

    pub const fn frame_rate_hz(&self) -> f64 {
        self.cpu_clock_hz as f64 / self.cycles_per_frame() as f64
    }

    pub const fn display_size(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }
}

// ---------------------------------------------------------------------------
// Sub-traits
// ---------------------------------------------------------------------------

/// Video output capabilities: display size and frame rendering.
pub trait Renderable {
    /// Native display resolution as (width, height) in pixels.
    fn display_size(&self) -> (u32, u32);

    /// Render the current video state into an RGB24 pixel buffer.
    ///
    /// The buffer must be at least `width * height * 3` bytes (from `display_size()`).
    /// Pixels are stored left-to-right, top-to-bottom, 3 bytes per pixel (R, G, B).
    ///
    /// The machine is responsible for converting its internal video representation
    /// (e.g., 4bpp column-major video RAM + palette) into this standard format.
    fn render_frame(&self, buffer: &mut [u8]);

    /// Optional debug overlay text (e.g., dirty-tracking stats).
    ///
    /// Returns a short string to display below the FPS counter when the
    /// overlay is active. Machines without stats return `None` (the default).
    fn overlay_stats(&self) -> Option<String> {
        None
    }
}

/// Audio output capabilities: PCM sample generation.
///
/// Machines without audio hardware can skip implementing this trait
/// (defaults produce silence with a zero sample rate).
pub trait AudioSource {
    /// Fill the buffer with mono i16 PCM samples at the machine's native
    /// sample rate. Returns the number of samples written.
    fn fill_audio(&mut self, _buffer: &mut [i16]) -> usize {
        0 // default: silence
    }

    /// Native audio sample rate in Hz (e.g., 894886 / some divisor).
    fn audio_sample_rate(&self) -> u32 {
        0
    }
}

/// Input handling: buttons and analog axes.
pub trait InputReceiver {
    /// Handle an input event. `button` is a machine-defined ID from `input_map()`.
    /// `pressed` is true for key-down, false for key-up.
    ///
    /// Called per-event, not per-frame. The frontend may call this multiple times
    /// between frames as input events arrive. Each call latches the button state
    /// so that `run_frame()` sees the accumulated input.
    fn set_input(&mut self, button: u8, pressed: bool);

    /// Get the list of input buttons this machine accepts.
    /// The frontend uses this to build key mappings and display configuration UI.
    fn input_map(&self) -> &[InputButton];

    /// Handle an analog input event. `axis` is a machine-defined ID from `analog_map()`.
    /// `delta` is a signed motion value (e.g., mouse dx/dy in pixels).
    ///
    /// Called per-event as motion occurs. The machine accumulates deltas internally.
    fn set_analog(&mut self, _axis: u8, _delta: i32) {}

    /// Get the list of analog axes this machine accepts.
    /// The frontend uses this to determine whether to capture mouse/trackball motion.
    fn analog_map(&self) -> &[AnalogInput] {
        &[]
    }
}

/// Debug/inspection capabilities for interactive debugging.
///
/// Machines without debug support can skip implementing this trait
/// (defaults return None / 0, disabling the debugger).
pub trait MachineDebug {
    /// Access bus debug capabilities (shared ref — reads, device/CPU discovery).
    fn debug_bus(&self) -> Option<&dyn BusDebug> {
        None
    }

    /// Access bus debug capabilities (mutable ref — writes).
    fn debug_bus_mut(&mut self) -> Option<&mut dyn BusDebug> {
        None
    }

    /// Number of clock ticks per frame (used by debug UI for cycle counting in run mode).
    fn cycles_per_frame(&self) -> u64 {
        0
    }

    /// Advance one cycle. Returns bitmask of CPUs at instruction boundaries.
    /// Bit 0 = CPU 0, bit 1 = CPU 1, etc.
    fn debug_tick(&mut self) -> u32 {
        0
    }

    /// Consume a pending watchpoint hit from the last tick, if any.
    ///
    /// The debugger polls this after each `debug_tick()`. When `Some` is
    /// returned, the debugger pauses execution and displays the hit.
    ///
    /// Default: delegates to `BusDebug::take_watchpoint_hit()` via `debug_bus_mut()`.
    fn take_watchpoint_hit(&mut self) -> Option<WatchpointHit> {
        self.debug_bus_mut()
            .and_then(|bus| bus.take_watchpoint_hit())
    }

    /// Set a memory watchpoint in the address space of `cpu_index`.
    ///
    /// Default: delegates to `BusDebug::set_watchpoint()` via `debug_bus_mut()`.
    fn set_watchpoint(&mut self, cpu_index: usize, addr: u16, kind: WatchpointKind) {
        if let Some(bus) = self.debug_bus_mut() {
            bus.set_watchpoint(cpu_index, addr, kind);
        }
    }

    /// Clear a memory watchpoint in the address space of `cpu_index`.
    ///
    /// Default: delegates to `BusDebug::clear_watchpoint()` via `debug_bus_mut()`.
    fn clear_watchpoint(&mut self, cpu_index: usize, addr: u16, kind: WatchpointKind) {
        if let Some(bus) = self.debug_bus_mut() {
            bus.clear_watchpoint(cpu_index, addr, kind);
        }
    }

    /// Clear all memory watchpoints across all address spaces.
    ///
    /// Default: delegates to `BusDebug::clear_all_watchpoints()` via `debug_bus_mut()`.
    fn clear_all_watchpoints(&mut self) {
        if let Some(bus) = self.debug_bus_mut() {
            bus.clear_all_watchpoints();
        }
    }

    /// Get the memory map for a CPU's address space (for region introspection).
    ///
    /// Default: delegates to `BusDebug::memory_map()` via `debug_bus()`.
    fn memory_map(&self, cpu_index: usize) -> Option<&MemoryMap> {
        self.debug_bus()?.memory_map(cpu_index)
    }
}

// ---------------------------------------------------------------------------
// Machine trait
// ---------------------------------------------------------------------------

/// Machine-agnostic interface for emulated systems.
///
/// Each machine (Joust, Robotron, etc.) implements this trait to provide
/// a uniform interface for the frontend. The frontend is a pure rendering
/// engine that does not know about specific hardware (PIAs, blitters,
/// palette formats, etc.).
///
/// Composed from sub-traits: [`Renderable`], [`AudioSource`],
/// [`InputReceiver`], and [`MachineDebug`].
pub trait Machine: Renderable + AudioSource + InputReceiver + MachineDebug {
    /// Run one frame of emulation (advance the clock by one frame's worth of cycles).
    fn run_frame(&mut self);

    /// Reset the machine to its initial power-on state.
    fn reset(&mut self);

    /// Native frame rate in Hz (e.g., 60.10 for Joust, 61.04 for Missile Command).
    /// Used by the frontend for real-time frame throttling.
    fn frame_rate_hz(&self) -> f64 {
        60.0
    }

    /// Short identifier for this machine type (e.g., "joust", "pacman").
    /// Used to validate save files against the correct machine.
    fn machine_id(&self) -> &str {
        ""
    }

    /// Capture complete machine state for later restoration.
    /// Returns `None` if this machine does not support save states.
    fn save_state(&self) -> Option<Vec<u8>> {
        None
    }

    /// Restore machine state from a previous `save_state()` snapshot.
    fn load_state(&mut self, _data: &[u8]) -> Result<(), SaveError> {
        Err(SaveError::InvalidFormat("save states not supported".into()))
    }

    /// Return battery-backed RAM contents for saving, or None if this machine has none.
    fn save_nvram(&self) -> Option<&[u8]> {
        None
    }

    /// Load battery-backed RAM contents from a previous save.
    fn load_nvram(&mut self, _data: &[u8]) {}
}
