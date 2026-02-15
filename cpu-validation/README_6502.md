# M6502 Cross-Validation

Validates phosphor-core's NMOS 6502 implementation against
[SingleStepTests/65x02](https://github.com/SingleStepTests/65x02), a
community-standard test suite with cycle-by-cycle bus traces validated against
multiple independent emulators and real hardware analysis.

## Results

151 legal NMOS 6502 opcodes validated, 10,000 test vectors per opcode:
**1,510,000/1,510,000 tests pass (100%)**.

## Prerequisites

- Git submodules initialized (`65x02` test data)

## Setup

```bash
# From the repository root
git submodule update --init

# The test data is located at:
# cpu-validation/test_data/65x02/6502/v1/*.json
```

## Usage

```bash
# Run validation (all 151 opcodes, 1.51M tests)
cargo test -p phosphor-cpu-validation -- m6502

# Expected output:
#   Validated 1510000 tests across 151 opcode files
```

## What It Validates

For each of the 10,000 test cases per opcode, the harness:
1. Sets all CPU registers (A, X, Y, SP, P, PC) and memory to the initial state
2. Executes one instruction using phosphor-core's M6502
3. Compares final registers (A, X, Y, SP, P, PC)
4. Compares final memory at all accessed addresses
5. Compares total cycle count
6. Compares cycle-by-cycle bus traces (address, data, read/write)

Unlike the M6809 and M6800 cross-validation (which only compare registers and
cycle counts), the 6502 validation also verifies **per-cycle bus activity** --
every dummy read, wrong-page read, and RMW write-back is checked against the
reference vectors.

## Test Vector Format

Each JSON file contains 10,000 test cases for a single opcode (SingleStepTests
convention):

```json
{
  "name": "A9 42",
  "initial": {
    "pc": 1234, "s": 253, "a": 0, "x": 0, "y": 0, "p": 36,
    "ram": [[1234, 169], [1235, 66]]
  },
  "final": {
    "pc": 1236, "s": 253, "a": 66, "x": 0, "y": 0, "p": 36,
    "ram": [[1234, 169], [1235, 66]]
  },
  "cycles": [[1234, 169, "read"], [1235, 66, "read"]]
}
```

Fields:
- **name** -- hex bytes of the instruction
- **initial/final** -- full CPU state (6 registers + accessed RAM)
- **cycles** -- per-cycle bus trace: `[address, data, "read"|"write"]`

Note: The 6502 performs a bus read or write on **every** cycle -- there are no
"internal" cycles in the bus trace (unlike the M6809/M6800 format which
includes `"internal"` entries).

## 6502 Bus Activity Quirks

The SingleStepTests vectors revealed several NMOS 6502 hardware behaviors that
required implementation fixes:

### No Internal Cycles
The 6502 performs a bus read or write on every single cycle. What would be
"internal" processing cycles on other CPUs are implemented as dummy reads from
predictable addresses (PC, stack pointer, or the operand address).

### Dummy Reads
- **2-cycle implied instructions** (NOP, CLC, TAX, INX, etc.): Dummy read from PC
- **Stack pull** (PLA, PLP): Dummy read from stack[SP] before incrementing
- **Stack push** (PHA, PHP): Dummy read from PC before pushing
- **Indexed zero-page** (ZP,X / ZP,Y): Dummy read from un-indexed ZP address
- **Branches taken**: Dummy read from next sequential PC
- **Branches with page cross**: Dummy read from wrong-page address `(old_PCH : target_PCL)`

### Wrong-Page Reads
When an indexed addressing mode crosses a page boundary, the 6502 first reads
from an incorrect address formed by combining the original high byte with the
indexed low byte. For read operations, this only occurs on actual page crosses.
For stores and RMW operations, this dummy read **always** occurs (even without
a page cross).

### RMW Write-Back
Read-modify-write instructions (ASL, LSR, ROL, ROR, INC, DEC on memory) write
the **original** unmodified value back to the address before writing the
modified result. This produces two consecutive writes to the same address.

## Opcode Coverage

All 151 legal NMOS 6502 opcodes:

| Category | Count | Opcodes |
|----------|-------|---------|
| Load (LDA/LDX/LDY) | 18 | 8 LDA modes + 5 LDX + 5 LDY |
| Store (STA/STX/STY) | 13 | 7 STA modes + 3 STX + 3 STY |
| Arithmetic (ADC/SBC) | 16 | 8 modes each |
| Compare (CMP/CPX/CPY) | 14 | 8 CMP + 3 CPX + 3 CPY |
| Logical (AND/ORA/EOR/BIT) | 26 | 8+8+8 modes + 2 BIT |
| Shift/Rotate (ASL/LSR/ROL/ROR) | 20 | 5 modes each (incl. accumulator) |
| Memory INC/DEC | 8 | 4 modes each |
| Branch | 8 | BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ |
| Jump/Subroutine | 5 | JMP abs, JMP ind, JSR, RTS, RTI |
| Stack | 4 | PHA, PLA, PHP, PLP |
| Flag set/clear | 7 | CLC, SEC, CLI, SEI, CLV, CLD, SED |
| Transfer | 6 | TAX, TAY, TXA, TYA, TSX, TXS |
| Register INC/DEC | 4 | INX, INY, DEX, DEY |
| Misc | 2 | NOP, BRK |

## Files

| File | Purpose |
|------|---------|
| `cpu-validation/src/lib.rs` | TracingBus, M6502TestCase/M6502CpuState JSON types |
| `cpu-validation/tests/m6502_single_step_test.rs` | Validation runner (151 opcodes x 10,000 vectors) |
| `cpu-validation/test_data/65x02/` | Git submodule: SingleStepTests/65x02 reference vectors |
