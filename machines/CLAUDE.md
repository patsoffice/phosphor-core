# phosphor-machines

Arcade and system board implementations. Each machine implements the `Bus` trait to connect CPUs to memory, I/O, and peripherals.

## Adding a New Machine

- Implement the `Bus` trait for your system struct
- Use the borrow-splitting `unsafe` pattern for `tick()` (CPU and bus access disjoint memory)
- ROM loading goes through `rom_loader.rs` utilities (supports ZIP archives)
- Video rendering is per-scanline during `run_frame()`

## Existing Machines

- `joust.rs` - Williams 2nd-gen arcade (M6809 main + M6800 sound CPU)
- `pacman.rs` - Namco Pac-Man hardware (Z80 + Namco WSG audio)
- `missile_command.rs` - Atari Missile Command (M6502 + POKEY audio)
- `simple*.rs` - Minimal test harnesses for each CPU type
