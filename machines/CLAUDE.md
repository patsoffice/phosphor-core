# phosphor-machines

Arcade and system board implementations. Each machine implements the `Bus` trait to connect CPUs to memory, I/O, and peripherals.

## Adding a New Machine

- Implement the `Bus` trait for your system struct
- Use the borrow-splitting `unsafe` pattern for `tick()` (CPU and bus access disjoint memory)
- ROM loading goes through `rom_loader.rs` utilities (ZIP extraction is handled by the frontend's `rom_path.rs`)
- Video rendering is per-scanline during `run_frame()`

## Board Wrapper Pattern

Games sharing hardware (e.g. Joust/Robotron on Williams, Pac-Man/Ms. Pac-Man on Namco Pac) use a two-level structure:

1. **Board struct** (e.g. `WilliamsBoard`, `NamcoPacBoard`) — owns CPUs, memory, and devices. Provides inherent methods: `render_frame()`, `fill_audio()`, `tick()`, etc.
2. **Game wrapper struct** (e.g. `JoustSystem`) — owns a `board` field plus game-specific state. Implements `Machine` and sub-traits by forwarding to the board with explicit one-line methods.

Forwarding is done with plain method calls (no macros):

```rust
impl Renderable for JoustSystem {
    fn display_size(&self) -> (u32, u32) {
        williams::TIMING.display_size()
    }
    fn render_frame(&self, buffer: &mut [u8]) {
        self.board.render_frame(buffer);
    }
}
```

Game-specific methods (`run_frame`, `reset`, `set_input`, `debug_tick`, `save_state`) are implemented directly without delegation. See `joust.rs` as the reference example.

## Shared Board Modules

Games sharing hardware use a shared board struct. When adding a new game on existing hardware, use the appropriate board:

- `williams.rs` - Williams 2nd-gen (M6809 + M6800 sound)
- `namco_pac.rs` - Namco Pac-Man (Z80 + Namco WSG)
- `namco_galaga.rs` - Namco Galaga (Z80 + Namco audio)
- `tkg04.rs` - Nintendo TKG-04 (Z80 + I8035 + DMA)
- `mcr2.rs` - Bally Midway MCR II (Z80 + SSIO + CTC)
- `atari_dvg.rs` - Atari DVG vector (M6502 + DVG)
- `gottlieb.rs` - Gottlieb System 80 (I8088 + M6502 sound)

## Reference Examples

- `joust.rs` - Reference for Board Wrapper Pattern (Williams board)
- `simple6502.rs`, `simple6800.rs`, `simple6809.rs`, `simplez80.rs` - Minimal test harnesses
