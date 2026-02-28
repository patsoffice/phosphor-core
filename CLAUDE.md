# CLAUDE.md

## Project: Phosphor Emulator

Cycle-accurate retro CPU emulator framework in Rust with arcade machine support.

### Build & Test

```bash
cargo build                                                    # Build entire workspace
cargo test -p phosphor-core                                    # Test CPU/device changes
cargo test -p phosphor-machines                                # Test machine/board changes
cargo test -p phosphor-macros                                  # Test proc macro changes
cargo test -p phosphor-frontend                                # Test frontend changes
cargo test -p phosphor-cpu-validation                          # CPU validation (slow — only after CPU changes)
cargo test m6809_alu_shift_test                                # Run specific test category
cargo clippy --all-features --all-targets                      # Check code quality
cargo clippy --all-features --all-targets --allow-dirty --fix  # Auto-fix clippy warnings
cargo fmt                                                      # Format code
cargo run --package phosphor-frontend -- joust /path/to/roms --scale 3
```

- Test the crate you changed; also test downstream crates when changing `phosphor-core` or `phosphor-macros`
- `cargo clippy` must pass with no warnings
- `cargo fmt` must pass with no warnings

### Workspace Crates

| Crate                      | Purpose                                 | Dependencies                                          |
|----------------------------|-----------------------------------------|-------------------------------------------------------|
| `phosphor-core`            | CPU implementations, Bus trait, devices | phosphor-macros                                       |
| `phosphor-macros`          | Proc macros                             | syn, quote, proc-macro2                               |
| `phosphor-machines`        | Arcade/system board implementations     | phosphor-core, phosphor-macros, inventory             |
| `phosphor-frontend`        | SDL2 display, audio, input, debug UI    | phosphor-core, phosphor-machines, sdl2, egui, gl, zip |
| `phosphor-cpu-validation`  | Test vector generation & validation     | phosphor-core, serde, serde_json, rand, flate2        |
| `cross-validation`         | C++ cross-validate against ref emulators| (non-Cargo, uses Makefile)                            |

- Never create circular dependencies between crates

### SDL2 Dependency

- `phosphor-frontend` requires SDL2: `brew install sdl2`
- `.cargo/config.toml` sets the Homebrew library path for aarch64-apple-darwin automatically
- Core and machines crates have no external C dependencies (only Rust crates)

### Testing Requirements

- Every new instruction must have integration tests
- Test both A and B register variants where applicable
- Include edge cases: zero, overflow, sign boundary (0x7F/0x80), carry propagation
- Use each CPU's flag enum in assertions (e.g. `CcFlag::X as u8` for M68xx), never raw hex values

### CPU Validation

```bash
# Self-validation
cargo test -p phosphor-cpu-validation

# Cross-validation (against reference emulators)
cd cross-validation && make
./bin/validate_m6809 ../cpu-validation/test_data/m6809/*.json
./bin/validate_m6800 ../cpu-validation/test_data/m6800/*.json
./bin/validate_i8035 ../cpu-validation/test_data/i8035/*.json
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
