# Z80 Cross-Validation

Validates phosphor-core's Z80 implementation against
[SingleStepTests/z80](https://github.com/SingleStepTests/z80), a
community-standard test suite with 1000 randomized test vectors per opcode
sequence, covering all prefix groups, undocumented flags, internal registers
(MEMPTR/WZ), and T-state timing.

## Results

1604 opcode sequences validated across 6 prefix groups:
**1,604,000/1,604,000 tests pass (100%)**.

## Prerequisites

- Git submodules initialized (`z80` test data)

## Setup

```bash
# From the repository root
git submodule update --init

# The test data is located at:
# cpu-validation/test_data/z80/v1/*.json
```

## Usage

```bash
# Run validation (all 1604 opcodes, 1.604M tests)
cargo test -p phosphor-cpu-validation -- z80

# Expected output:
#   Z80 SingleStepTests: 1604000 passed, 0 failed across 1604 files
```

## What It Validates

For each of the 1000 test cases per opcode, the harness:

1. Sets all CPU registers (A-L, I, R, IX, IY, SP, PC, shadow set, IFF1/IFF2, IM) and memory
2. Loads I/O port data from the test case `ports` field
3. Executes one instruction using phosphor-core's Z80
4. Compares all final registers including:
   - Main registers (A, F, B, C, D, E, H, L)
   - Shadow registers (AF', BC', DE', HL')
   - 16-bit registers (IX, IY, SP, PC)
   - System registers (I, R, IM, IFF1, IFF2)
   - Internal state (MEMPTR/WZ, EI delay, p flag, q flag)
5. Compares final memory at all accessed addresses
6. Compares total T-state count

## Test Vector Format

Each JSON file contains 1000 test cases. File naming encodes the prefix group:

| Pattern | Group | Example |
|---------|-------|---------|
| `XX.json` | Main page | `00.json` (NOP) |
| `cb XX.json` | CB prefix | `cb 00.json` (RLC B) |
| `dd XX.json` | DD prefix (IX) | `dd 09.json` (ADD IX,BC) |
| `ed XX.json` | ED prefix | `ed a0.json` (LDI) |
| `fd XX.json` | FD prefix (IY) | `fd 09.json` (ADD IY,BC) |
| `dd cb __ XX.json` | DDCB (IX bit ops) | `dd cb __ 06.json` (RLC (IX+d)) |
| `fd cb __ XX.json` | FDCB (IY bit ops) | `fd cb __ 06.json` (RLC (IY+d)) |

```json
{
  "name": "ED A0 0042",
  "initial": {
    "pc": 1234, "sp": 65535,
    "a": 66, "b": 0, "c": 3, "d": 0, "e": 0, "f": 0,
    "h": 16, "l": 0, "i": 0, "r": 0,
    "ix": 0, "iy": 0, "wz": 0,
    "af_": 0, "bc_": 0, "de_": 0, "hl_": 0,
    "iff1": 0, "iff2": 0, "im": 0, "ei": 0, "p": 0, "q": 0,
    "ram": [[1234, 237], [1235, 160], [4096, 66]]
  },
  "final": { ... },
  "cycles": [[1234, 237, "read"], [1235, 160, "read"], ...],
  "ports": []
}
```

Fields:

- **name** -- hex bytes identifying the test case
- **initial/final** -- full CPU state including shadow registers, MEMPTR, and internal flags
- **cycles** -- per-T-state bus trace: `[address|null, data|null, "read"|"write"|"..."]`
- **ports** -- I/O port data: `[port_address, data, "r"|"w"]` for IN/OUT instructions

## Internal State Fields

The Z80 test vectors include internal state not visible to programs but essential for
correct undocumented flag behavior:

| Field | Maps to | Description |
|-------|---------|-------------|
| `wz` | `memptr` | Internal temporary register, affects X/Y flags in BIT, block ops |
| `ei` | `ei_delay` | True if last instruction was EI (delays interrupt acceptance) |
| `p` | `p` | True after LD A,I / LD A,R (affects PV if interrupted) |
| `q` | `q` | True if last instruction modified F (affects SCF/CCF X/Y flags) |
| `af_`, `bc_`, `de_`, `hl_` | shadow regs | 16-bit packed shadow register pairs |

## Notable Undocumented Behaviors

The cross-validation exposed several undocumented Z80 behaviors requiring precise
implementation:

### MEMPTR (WZ)

An internal 16-bit register used as a temporary in many instructions. Not accessible to
programs, but its value leaks into the X/Y flags (bits 3/5) for certain instructions,
particularly `BIT b,(HL)` and block operations.

### Block I/O Repeat Flags

For INIR/INDR/OTIR/OTDR, the repeat path (when B != 0) modifies H and PV flags
differently from the single-shot INI/IND/OUTI/OUTD:

- **H flag**: Re-derived from B's low nibble and the N/C flags
- **PV flag**: Uses XNOR of base PV with parity of adjusted B value
- **X/Y flags**: Taken from high byte of rewound PC (not MEMPTR)

### SCF/CCF Q Flag

The X/Y flags for SCF and CCF depend on whether the previous instruction modified the
F register (tracked by the `q` internal flag):

- `q != 0`: X/Y from A only
- `q == 0`: X/Y from A OR old_F

### R Register

The refresh register increments once per M1 cycle. Bit 7 is preserved from the last
`LD R,A` instruction. Prefix bytes (CB, DD, ED, FD) each trigger their own M1 cycle
and increment R.

## Prefix Group Coverage

| Group | Files | Tests | Description |
|-------|-------|-------|-------------|
| Main | 252 | 252,000 | Standard opcodes (excludes prefix bytes) |
| CB | 256 | 256,000 | Bit operations: rotates, shifts, BIT/SET/RES |
| DD | 252 | 252,000 | IX-indexed variants |
| ED | 80 | 80,000 | Extended: block ops, 16-bit ALU, I/O |
| FD | 252 | 252,000 | IY-indexed variants |
| DDCB | 256 | 256,000 | IX+d indexed bit operations |
| FDCB | 256 | 256,000 | IY+d indexed bit operations |
| **Total** | **1604** | **1,604,000** | |

## Files

| File | Purpose |
|------|---------|
| `cpu-validation/src/lib.rs` | TracingBus (with I/O port queue), Z80TestCase/Z80CpuState JSON types |
| `cpu-validation/tests/z80_single_step_test.rs` | Validation runner (1604 opcodes x 1000 vectors) |
| `cpu-validation/test_data/z80/` | Git submodule: SingleStepTests/z80 reference vectors |
