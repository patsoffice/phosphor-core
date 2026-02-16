# phosphor-frontend

SDL2-based display, audio, and input handling. This is the only crate with external C dependencies.

## Structure

- `emulator.rs` - Main loop, frame timing, machine dispatch
- `video.rs` - SDL2 texture rendering
- `audio.rs` - SDL2 audio callback
- `input.rs` - Keyboard/joystick mapping
- `overlay.rs` - FPS counter, debug overlay
- `rom_path.rs` - ROM file discovery and path resolution

## Dependencies

- Requires SDL2: `brew install sdl2`
- `.cargo/config.toml` configures the Homebrew library path for aarch64-apple-darwin
