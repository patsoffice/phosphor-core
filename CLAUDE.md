# CLAUDE.md

## Project: Phosphor Emulator

Cycle-accurate retro CPU emulator framework in Rust with arcade machine support.

### Build & Test

```bash
cargo build                                                    # Build entire workspace
cargo test                                                     # Run all tests
cargo test m6809_alu_shift_test                                # Run specific test category
cargo cargo clippy --all-features --all-targets                # Check code quality
cargo clippy --all-features --all-targets --allow-dirty --fix. # Run this if there are clippy warnings before you fix them yourself
cargo fmt                                                      # Format code
cargo run --package phosphor-frontend -- joust /path/to/roms --scale 3
```

- All tests must pass before committing
- `cargo clippy` must pass with no warnings
- `cargo fmt` must pass with no warnings

### Workspace Crates

| Crate                      | Purpose                                 | Dependencies                           |
|----------------------------|-----------------------------------------|----------------------------------------|
| `phosphor-core`            | CPU implementations, Bus trait, devices | None (pure Rust)                       |
| `phosphor-machines`        | Arcade/system board implementations     | phosphor-core                          |
| `phosphor-frontend`        | SDL2 display, audio, input              | phosphor-core, phosphor-machines, sdl2 |
| `phosphor-cpu-validation`  | Test vector generation & validation     | phosphor-core, serde, rand             |

- Never create circular dependencies between crates

### SDL2 Dependency

- `phosphor-frontend` requires SDL2: `brew install sdl2`
- `.cargo/config.toml` sets the Homebrew library path for aarch64-apple-darwin automatically
- Core and machines crates remain zero-dep pure Rust

### Testing Requirements

- Every new instruction must have integration tests
- Test both A and B register variants where applicable
- Include edge cases: zero, overflow, sign boundary (0x7F/0x80), carry propagation
- Use `CcFlag::X as u8` in assertions, never raw hex values

### CPU Validation

```bash
# Self-validation
cargo test -p phosphor-cpu-validation

# Cross-validation (against reference emulators)
cd cross-validation && make && ./bin/validate_m6809 ../cpu-validation/test_data/m6809/*.json
```

- If cross-validation differs from datasheet for timings, use the datasheet values
- Any changes to the CPUs must run the cross-validation script

### Commit Style

- Prefix: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- Summary line under 80 chars with counts where relevant
- Body: each logical change on its own `-` bullet
- Summarize what was added/changed and why, not just file names

### Design Priorities

1. **Correctness** - Cycle-accurate hardware matching
2. **Clarity** - Educational, maintainable code
3. **Performance** - Fast enough for real-time

### README

- Keep roadmap checkboxes current
- Update CPU-specific READMEs when adding instructions or changing opcode counts
