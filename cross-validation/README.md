# Cross-Validation

Validates phosphor-core's CPU test vectors against independent reference
emulators. All MAME-based validators use the
[mame4all](https://github.com/ValveSoftware/steamlink-sdk/tree/master/examples/mame4all)
source via git submodule.

## Validators

| CPU   | Reference Emulator | Result                    |
|-------|--------------------|---------------------------|
| M6809 | elmerucr/MC6809    | 266,000/266,000 (100%)    |
| M6809 | mame4all           | 261,601/266,000 (98.3%)   |
| M6800 | mame4all           | 191,996/192,000 (99.998%) |
| I8035 | mame4all           | 221,000/225,000 (98.2%)   |

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
# Validate M6809 against elmerucr/MC6809
./cross-validation/bin/validate_m6809 cpu-validation/test_data/m6809/*.json

# Validate M6809 against mame4all
./cross-validation/bin/validate_m6809_mame cpu-validation/test_data/m6809/*.json

# Validate M6800 against mame4all
./cross-validation/bin/validate_m6800 cpu-validation/test_data/m6800/*.json

# Validate I8035 against mame4all
./cross-validation/bin/validate_i8035 cpu-validation/test_data/i8035/*.json
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
