# Cross-Validation

Validates phosphor-core's CPU test vectors against independent reference
emulators.

All CPUs use [MAME 0.148](https://github.com/mamedev/mame/tree/mame0148)
via shallow clone (last MAME release with simple legacy CPU cores).

## Validators

| CPU    | Reference Emulator | Result                    | Notes                                                                                          |
|--------|--------------------|---------------------------|------------------------------------------------------------------------------------------------|
| M6809  | MAME 0.148         | 264,090/266,000 (99.28%)  | DAA CC diffs (683); EXG/TFR undefined regs (967); indexed mode cycle table FIXME (260)         |
| M6800  | MAME 0.148         | 192,000/192,000 (100%)    | CC bits 6-7 masked (undefined on M6800)                                                        |
| I8035  | MAME 0.148         | 228,966/229,000 (99.985%) | PSW bit 3 masked; expander port ops skip P2/A; 34 DA A failures (MAME carry bug)               |
| MB88XX | MAME 0.148         | 256,000/256,000 (100%)    |                                                                                                |

## Prerequisites

- C++17 compiler (clang++ or g++)
- MAME 0.148 shallow clone

## Setup

```bash
# Shallow-clone MAME 0.148 (~60MB)
git clone --depth 1 --branch mame0148 \
    https://github.com/mamedev/mame.git cross-validation/mame0148

# Build all validators
make -C cross-validation
```

The `mame0148/` directory is gitignored (not a submodule) since MAME's full
history is very large and only a few source files are needed from it.

## Usage

```bash
# Validate M6809 against MAME 0.148
./cross-validation/bin/validate_m6809 cpu-validation/test_data/m6809/*.json

# Validate M6800 against MAME 0.148
./cross-validation/bin/validate_m6800 cpu-validation/test_data/m6800/*.json

# Validate I8035 against MAME 0.148
./cross-validation/bin/validate_i8035 cpu-validation/test_data/i8035/*.json

# Validate MB88XX against MAME 0.148
./cross-validation/bin/validate_mb88xx cpu-validation/test_data/mb88xx/*.json
```

## Architecture

All validators share a common framework header (`mame0148_shim.h`) that
provides minimal stubs for the MAME 0.148 device infrastructure. Each CPU
has a thin per-CPU shim (`<cpu>/emu.h`) that defines flat memory arrays and
address space routing. The validator `.cpp` files `#include` the MAME `.c`
source directly for access to internal CPU state.

The shim supports two MAME patterns:

- **Legacy** (M6800, MCS48, MB88XX): C-style `CPU_INIT/RESET/EXECUTE` macros
  with separate state structs and `legacy_cpu_device`
- **Modern** (M6809): C++ device classes with virtual methods, enabled via
  `#define SHIM_MODERN_CPU_DEVICE` which provides `machine_config`,
  `address_space_config`, and an extended `cpu_device` with constructor and
  `state_add()` support

## What It Validates

For each test case, the harness:

1. Sets all CPU registers and memory to the initial state
2. Executes one instruction using the reference emulator
3. Compares final registers
4. Compares final memory at all accessed addresses
5. Compares total cycle count

Bus-level cycle traces (per-cycle address/data/direction) are not validated
since the reference emulators do not expose per-cycle bus activity.
