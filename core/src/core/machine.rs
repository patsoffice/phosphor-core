/// Describes a single input button that a machine accepts.
pub struct InputButton {
    /// Machine-defined button identifier, passed to `set_input()`.
    pub id: u8,
    /// Human-readable name for display/configuration (e.g., "P1 Left", "Coin").
    pub name: &'static str,
}

/// Machine-agnostic interface for emulated systems.
///
/// Each machine (Joust, Robotron, etc.) implements this trait to provide
/// a uniform interface for the frontend. The frontend is a pure rendering
/// engine that does not know about specific hardware (PIAs, blitters,
/// palette formats, etc.).
pub trait Machine {
    /// Native display resolution as (width, height) in pixels.
    fn display_size(&self) -> (u32, u32);

    /// Run one frame of emulation (advance the clock by one frame's worth of cycles).
    fn run_frame(&mut self);

    /// Render the current video state into an RGB24 pixel buffer.
    ///
    /// The buffer must be at least `width * height * 3` bytes (from `display_size()`).
    /// Pixels are stored left-to-right, top-to-bottom, 3 bytes per pixel (R, G, B).
    ///
    /// The machine is responsible for converting its internal video representation
    /// (e.g., 4bpp column-major video RAM + palette) into this standard format.
    fn render_frame(&self, buffer: &mut [u8]);

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

    /// Reset the machine to its initial power-on state.
    fn reset(&mut self);
}
