# CLAUDE.md

## Project: Phosphor Emulator

### Build & Test

```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build --package phosphor-core
cargo build --package phosphor-machines
cargo build --package phosphor-frontend

# Run all tests
cargo test

# Run specific test category
cargo test m6809_alu_shift_test

# Run the emulator
cargo run --package phosphor-frontend -- joust /path/to/roms --scale 3
```

- `cargo fmt` to format code
- `cargo clippy` to check code quality
- All tests must pass before committing

### SDL2 Dependency

- The `phosphor-frontend` crate requires SDL2: `brew install sdl2`
- `.cargo/config.toml` sets the Homebrew library path for aarch64-apple-darwin automatically
- Core and machines crates remain zero-dep pure Rust

### Architecture Rules

- CPU instructions go in `core/src/cpu/<cpu>/alu.rs` (ALU ops) or `load_store.rs` (load/store)
- Opcode dispatch entries go in `core/src/cpu/<cpu>/mod.rs` `execute_instruction()`
- Inherent-mode instructions use `if cycle == 0 { ... }` pattern
- Immediate-mode instructions use `alu_imm()` helper
- Always transition to `ExecState::Fetch` when instruction completes
- M6800 follows identical patterns to M6809 (same flag helpers, addressing mode helpers, state machine)
- M6800 has no DP register (direct mode always page 0), no Y/U registers, no multi-byte opcode prefixes

### Flag Conventions

- Use `CcFlag` enum, never raw hex values (0x01, 0x02, etc.)
- All instruction doc comments must document flag behavior
- Use `set_flags_arithmetic()` for add/sub, `set_flags_logical()` for AND/OR/EOR/TST, `set_flags_shift()` for shift/rotate
- V flag for shift/rotate = N XOR C (post-operation)

### Testing Requirements

- Every new instruction must have integration tests
- Tests go in `tests/m6809_*_test.rs` or `m6800_*_test.rs` files, grouped by category
- Test both A and B register variants
- Include edge cases: zero, overflow, sign boundary (0x7F/0x80), carry propagation
- Use `CcFlag::X as u8` in assertions, not raw hex

### CPU Validation

Cross-validation infrastructure for M6809 (266 opcodes, 266,000 test vectors):

```bash
# Generate test vectors for all opcodes
cd cpu-validation && cargo run --bin gen_m6809_tests -- all

# Self-validation (phosphor-core against its own test vectors)
cargo test -p phosphor-cpu-validation

# Cross-validation (against elmerucr/MC6809 reference emulator)
cd cross-validation && make && ./validate ../cpu-validation/test_data/m6809/*.json
```

- Test generator filters undefined indexed postbytes and undefined EXG/TFR register codes
- SYNC (0x13) and CWAI (0x3C) are intentionally excluded (they halt waiting for interrupts)
- If cross-validation differs from datasheet for timings, use the datasheet values

### Commit Style

- Summarize what was added (opcodes, tests) and why

### README

- Keep roadmap checkboxes current
