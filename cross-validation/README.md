# M6809 Cross-Validation

Validates phosphor-core's M6809 test vectors against
[elmerucr/MC6809](https://github.com/elmerucr/MC6809), an independent
cycle-accurate 6809 emulator.

## Prerequisites

- C++17 compiler (clang++ or g++)
- Git submodules initialized

## Setup

```bash
# From the repository root
git submodule update --init

# Build
make -C cross-validation
```

## Usage

```bash
# Validate a single opcode
./cross-validation/validate cpu-validation/test_data/m6809/86.json

# Validate multiple opcodes
./cross-validation/validate cpu-validation/test_data/m6809/*.json
```

## What It Validates

For each test case, the harness:
1. Sets all CPU registers and memory to the initial state
2. Executes one instruction using elmerucr/MC6809
3. Compares final registers (PC, A, B, DP, X, Y, U, S, CC)
4. Compares final memory at all accessed addresses
5. Compares total cycle count

Bus-level cycle traces (per-cycle address/data/direction) are not validated
since elmerucr/MC6809 does not expose per-cycle bus activity.
