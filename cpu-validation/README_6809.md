# M6809 Cross-Validation

Validates phosphor-core's M6809 test vectors against
[elmerucr/MC6809](https://github.com/elmerucr/MC6809), an independent
cycle-accurate 6809 emulator.

## Results

266 opcodes validated, 266,000 test vectors (1,000 per opcode):
**266,000/266,000 tests pass (100%)** across all three opcode pages.

## Prerequisites

- C++17 compiler (clang++ or g++)
- Git submodules initialized

## Setup

```bash
# From the repository root
git submodule update --init

# Build
make -C cross-validation validate

# Generate test vectors (must run from cpu-validation/ directory)
cd cpu-validation && cargo run --bin gen_m6809_tests --release -- all
```

## Usage

```bash
# Validate a single opcode
./cross-validation/validate cpu-validation/test_data/m6809/86.json

# Validate all opcodes
./cross-validation/validate cpu-validation/test_data/m6809/*.json
```

## What It Validates

For each test case, the harness:
1. Sets all CPU registers and 64KB memory to the initial state
2. Executes one instruction using elmerucr/MC6809
3. Compares final registers (PC, A, B, DP, X, Y, U, S, CC)
4. Compares final memory at all accessed addresses
5. Compares total cycle count

Bus-level cycle traces (per-cycle address/data/direction) are not validated
since elmerucr/MC6809 does not expose per-cycle bus activity.

## Test Vector Format

Each JSON file contains 1,000 test cases for a single opcode:

```json
{
  "name": "86 42",
  "initial": {
    "pc": 4096, "a": 0, "b": 65, "dp": 0,
    "x": 30010, "y": 1024, "u": 512, "s": 42075, "cc": 75,
    "ram": [[4096, 134], [4097, 66]]
  },
  "final": {
    "pc": 4098, "a": 66, "b": 65, "dp": 0,
    "x": 30010, "y": 1024, "u": 512, "s": 42075, "cc": 73,
    "ram": [[4096, 134], [4097, 66]]
  },
  "cycles": [
    [4096, 134, "read"],
    [4097, 66, "read"]
  ]
}
```

Fields:
- **name** — hex bytes of the instruction
- **initial/final** — full CPU state (all 9 registers + accessed RAM)
- **cycles** — per-cycle bus trace: `[address, data, "read"|"write"|"internal"]`

## Test Generation

The test generator (`gen_m6809_tests.rs`) produces 1,000 randomized test
vectors per opcode:

1. Randomize all 64KB of memory and all 9 CPU registers
2. Clamp PC to a valid range (ensures operand bytes fit in address space)
3. Place the opcode (and page prefix if applicable) at PC
4. Execute with `cpu.tick_with_bus()` until the instruction completes (max 200 cycles)
5. Record all bus cycles and snapshot initial/final state
6. For indexed instructions, skip undefined postbytes
7. For EXG/TFR, skip undefined register codes
8. Retry on timeout (max 10x attempts per vector)

```bash
# Generate a single opcode
cd cpu-validation && cargo run --bin gen_m6809_tests -- 0x86

# Generate all opcodes
cd cpu-validation && cargo run --bin gen_m6809_tests -- all
```

Output: `cpu-validation/test_data/m6809/<opcode>.json` (e.g., `86.json`,
`10_8e.json` for page 2, `11_83.json` for page 3).

## Opcode Coverage

266 opcodes across 3 pages:

| Page | Prefix | Count | Examples |
|------|--------|-------|----------|
| Page 1 | (none) | 238 | LDA, ADDA, BRA, JSR, TFR, EXG |
| Page 2 | 0x10 | 19 | CMPD, CMPY, LDY, STY, LDS, STS, long branches, SWI2 |
| Page 3 | 0x11 | 9 | CMPU, CMPS, SWI3 |

## Excluded Opcodes (2)

These opcodes are excluded because they halt the CPU waiting for an interrupt,
which cannot complete in single-step validation with no interrupt sources:

- **SYNC (0x13)** — halts CPU until any interrupt
- **CWAI (0x3C)** — pushes entire state, masks CC, halts until interrupt

## Self-Validation

Phosphor also validates against its own test vectors as a Rust integration test:

```bash
cargo test -p phosphor-cpu-validation
```

This runs `m6809_single_step_test.rs`, which loads every JSON file and replays
each test case against phosphor-core, asserting registers, memory, cycle count,
and per-cycle bus traces.

## Files

| File | Purpose |
|------|---------|
| `cpu-validation/src/lib.rs` | TracingBus, JSON types (shared by M6809 and M6800) |
| `cpu-validation/src/bin/gen_m6809_tests.rs` | Test vector generator (266 opcodes x 1,000 vectors) |
| `cpu-validation/tests/m6809_single_step_test.rs` | Self-validation (phosphor against its own vectors) |
| `cross-validation/validate.cpp` | Cross-validation harness |
| `cross-validation/mc6809/` | Git submodule: elmerucr/MC6809 reference emulator |
