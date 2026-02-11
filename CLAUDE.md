# CLAUDE.md

## Project: Phosphor Core (M6809/6502 Emulator)

### Build & Test

```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build --package phosphor-core
cargo build --package phosphor-machines

# Run all tests
cargo test

# Run specific test category
cargo test m6809_alu_shift_test
```

- `cargo fmt` to format code
- `cargo clippy` to check code quality
- All tests must pass before committing

### Architecture Rules

- CPU instructions go in `core/src/cpu/m6809/alu.rs` (ALU ops) or `load_store.rs` (load/store)
- Opcode dispatch entries go in `core/src/cpu/m6809/mod.rs` `execute_instruction()`
- Inherent-mode instructions use `if cycle == 0 { ... }` pattern
- Immediate-mode instructions use `alu_imm()` helper
- Always transition to `ExecState::Fetch` when instruction completes

### Flag Conventions

- Use `CcFlag` enum, never raw hex values (0x01, 0x02, etc.)
- All instruction doc comments must document flag behavior
- Use `set_flags_arithmetic()` for add/sub, `set_flags_logical()` for AND/OR/EOR/TST, `set_flags_shift()` for shift/rotate
- V flag for shift/rotate = N XOR C (post-operation)

### Testing Requirements

- Every new instruction must have integration tests
- Tests go in `tests/m6809_*_test.rs` files, grouped by category
- Test both A and B register variants
- Include edge cases: zero, overflow, sign boundary (0x7F/0x80), carry propagation
- Use `CcFlag::X as u8` in assertions, not raw hex

### Commit Style

- Summarize what was added (opcodes, tests) and why

### README

- Update opcode/test counts in all locations when adding instructions
- Keep roadmap checkboxes current
