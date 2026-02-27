# phosphor-machines

Arcade and system board implementations. Each machine implements the `Bus` trait to connect CPUs to memory, I/O, and peripherals.

## Adding a New Machine

- Implement the `Bus` trait for your system struct
- Use the borrow-splitting `unsafe` pattern for `tick()` (CPU and bus access disjoint memory)
- ROM loading goes through `rom_loader.rs` utilities (supports ZIP archives)
- Video rendering is per-scanline during `run_frame()`

## Board Wrapper Pattern

Games sharing hardware (e.g. Joust/Robotron on Williams, Pac-Man/Ms. Pac-Man on Namco Pac) use a two-level structure:

1. **Board struct** (e.g. `WilliamsBoard`, `NamcoPacBoard`) — owns CPUs, memory, and devices. Provides inherent methods: `render_frame()`, `fill_audio()`, `tick()`, etc.
2. **Game wrapper struct** (e.g. `JoustSystem`) — owns a `board` field plus game-specific state. Implements `Machine` and sub-traits by forwarding to the board with explicit one-line methods.

Forwarding is done with plain method calls (no macros):

```rust
impl Renderable for JoustSystem {
    fn display_size(&self) -> (u32, u32) {
        (williams::DISPLAY_WIDTH, williams::DISPLAY_HEIGHT)
    }
    fn render_frame(&self, buffer: &mut [u8]) {
        self.board.render_frame(buffer);
    }
}
```

Game-specific methods (`run_frame`, `reset`, `set_input`, `debug_tick`, `save_state`) are implemented directly without delegation. See `joust.rs` as the reference example.

## Existing Machines

- `joust.rs` - Williams 2nd-gen arcade (M6809 main + M6800 sound CPU)
- `robotron.rs` - Williams 2nd-gen arcade (M6809, twin-stick)
- `pacman.rs` - Namco Pac-Man hardware (Z80 + Namco WSG audio)
- `mspacman.rs` - Ms. Pac-Man (Namco Pac board + decode latch copy protection)
- `donkey_kong.rs` - Nintendo TKG-04 board (Z80 + I8035 + DMA)
- `donkey_kong_jr.rs` - Nintendo TKG-04 board (24KB ROM, gfx bank)
- `satans_hollow.rs` - Bally Midway MCR II (Z80 + SSIO + CTC)
- `asteroids.rs` - Atari vector arcade (M6502 + DVG)
- `missile_command.rs` - Atari Missile Command (M6502 + POKEY audio)
- `ccastles.rs` - Crystal Castles (M6502 + 2×POKEY + trackball)
- `gridlee.rs` - Videa arcade (M6809 + bitmap video + trackball)
- `simple*.rs` - Minimal test harnesses for each CPU type
