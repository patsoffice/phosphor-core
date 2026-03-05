# Cross-Validation

Validates phosphor-core's CPU test vectors against independent reference
emulators.

- **M6809, M6800, I8035**: use [mame4all](https://github.com/ValveSoftware/steamlink-sdk/tree/master/examples/mame4all) via git submodule
- **MB88XX**: uses [MAME 0.148](https://github.com/mamedev/mame/tree/mame0148) via shallow clone (mame4all lacks MB88XX support; MAME 0.148 is the last release with a simple C-style CPU core)

## Validators

| CPU    | Reference Emulator | Result                    |
|--------|--------------------|---------------------------|
| M6809  | elmerucr/MC6809    | 266,000/266,000 (100%)    |
| M6809  | mame4all           | 261,601/266,000 (98.3%)   |
| M6800  | mame4all           | 191,996/192,000 (99.998%) |
| I8035  | mame4all           | 221,000/225,000 (98.2%)   |
| MB88XX | MAME 0.148         | 256,000/256,000 (100%)    |

## Prerequisites

- C++17 compiler (clang++ or g++)
- Git submodules initialized
- MAME 0.148 shallow clone (for MB88XX only)

## Setup

```bash
# From the repository root
git submodule update --init

# Shallow-clone MAME 0.148 (for MB88XX validator only, ~60MB)
git clone --depth 1 --branch mame0148 \
    https://github.com/mamedev/mame.git cross-validation/mame0148

# Build all validators
make -C cross-validation
```

The `mame0148/` directory is gitignored (not a submodule) since MAME's full
history is very large and only two source files are needed from it.

## Usage

```bash
# Validate M6809 against elmerucr/MC6809
./cross-validation/bin/validate_m6809 cpu-validation/test_data/m6809/*.json

# Validate M6809 against mame4all
./cross-validation/bin/validate_m6809_mame cpu-validation/test_data/m6809/*.json

# Validate M6800 against mame4all
./cross-validation/bin/validate_m6800 cpu-validation/test_data/m6800/*.json

# Validate I8035 against mame4all
./cross-validation/bin/validate_i8035 cpu-validation/test_data/i8035/*.json

# Validate MB88XX against MAME 0.148
./cross-validation/bin/validate_mb88xx cpu-validation/test_data/mb88xx/*.json
```

## What It Validates

For each test case, the harness:
1. Sets all CPU registers and memory to the initial state
2. Executes one instruction using the reference emulator
3. Compares final registers
4. Compares final memory at all accessed addresses
5. Compares total cycle count

Bus-level cycle traces (per-cycle address/data/direction) are not validated
since the reference emulators do not expose per-cycle bus activity.
