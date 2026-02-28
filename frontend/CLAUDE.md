# phosphor-frontend

SDL2-based display, audio, and input handling. This is the only crate with external C dependencies.

## Structure

- `main.rs` - Entry point, CLI arg parsing, machine instantiation, ROM/state management
- `emulator.rs` - Main loop, frame timing, machine dispatch, SDL event handling
- `video.rs` - SDL2 texture rendering, egui integration, GL context setup
- `audio.rs` - SDL2 audio callback
- `input.rs` - Keyboard/joystick mapping
- `overlay.rs` - FPS counter, debug overlay
- `debug_ui.rs` - CPU debug panels, breakpoints, disassembly, memory viewer (egui)
- `vector_gl.rs` - OpenGL vector display renderer (for DVG machines)
- `rom_path.rs` - ROM file discovery, path resolution, ZIP archive extraction

## Dependencies

- Requires SDL2: `brew install sdl2`
- `.cargo/config.toml` configures the Homebrew library path for aarch64-apple-darwin
- Uses egui/GL for debug UI panels and vector display rendering
- Uses zip crate for ROM archive loading
