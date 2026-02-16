# Zilog Z80 CPU

Cycle-accurate emulation of the Zilog Z80 microprocessor, implementing all 1604 opcode sequences across 6 prefix groups (unprefixed, CB, DD, ED, FD, DDCB, FDCB) including undocumented opcodes. Validated against [SingleStepTests/z80](https://github.com/SingleStepTests/z80) with 1,604,000 test vectors (100% pass rate).

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 1604 (all prefix groups, including undocumented) |
| Integration tests | 241 |
| Cross-validation | 1,604,000/1,604,000 (100%) |
| Timing | T-state accurate |

## Registers

| Register | Size | Description |
|----------|------|-------------|
| A, F | 8-bit | Accumulator and flags |
| B, C | 8-bit | General purpose / loop counter / port address |
| D, E | 8-bit | General purpose |
| H, L | 8-bit | General purpose / memory pointer |
| A', F', B', C', D', E', H', L' | 8-bit | Shadow register set (EX AF,AF' / EXX) |
| IX, IY | 16-bit | Index registers (DD/FD prefixes) |
| SP | 16-bit | Stack pointer |
| PC | 16-bit | Program counter |
| I | 8-bit | Interrupt vector base (IM 2) |
| R | 7-bit | Memory refresh counter |
| IFF1, IFF2 | 1-bit | Interrupt flip-flops |
| MEMPTR (WZ) | 16-bit | Internal temporary register (undocumented, affects flags) |

### Flags (F register)

| Bit | Flag | Name |
|-----|------|------|
| 7 | S | Sign |
| 6 | Z | Zero |
| 5 | Y | Undocumented (bit 5 of result) |
| 4 | H | Half-carry |
| 3 | X | Undocumented (bit 3 of result) |
| 2 | PV | Parity/Overflow |
| 1 | N | Subtract |
| 0 | C | Carry |

## Instruction Set

1604 opcode sequences across 6 prefix groups:

| Group | Prefix | Opcodes | Description |
|-------|--------|---------|-------------|
| Main | (none) | 252 | Core instructions (excludes CB/DD/ED/FD prefix bytes) |
| CB | CB | 256 | Bit operations: rotates, shifts, BIT/SET/RES |
| DD | DD | 252 | IX-indexed variants of main page |
| ED | ED | 80 | Extended: block ops, 16-bit ALU, I/O, IM, LD A,I/R |
| FD | FD | 252 | IY-indexed variants of main page |
| DDCB | DD CB d | 256 | IX+d indexed bit operations |
| FDCB | FD CB d | 256 | IY+d indexed bit operations |

### Instruction Categories

| Category | Instructions |
|----------|-------------|
| 8-bit Load | LD r,r' / LD r,n / LD r,(HL) / LD (HL),r / LD r,(IX+d) |
| 16-bit Load | LD rr,nn / LD SP,HL / LD (nn),rr / LD rr,(nn) |
| Exchange | EX AF,AF' / EXX / EX DE,HL / EX (SP),HL |
| Stack | PUSH/POP AF/BC/DE/HL/IX/IY |
| 8-bit ALU | ADD/ADC/SUB/SBC/AND/XOR/OR/CP with r, n, (HL), (IX+d) |
| 16-bit ALU | ADD HL,rr / ADC HL,rr / SBC HL,rr / INC rr / DEC rr |
| Rotate/Shift | RLCA/RRCA/RLA/RRA, CB: RLC/RRC/RL/RR/SLA/SRA/SRL/SLL |
| Bit Operations | BIT b,r / SET b,r / RES b,r (CB prefix) |
| Branch | JP/JR/DJNZ/CALL/RET/RST with conditions (NZ/Z/NC/C/PO/PE/P/M) |
| Block Transfer | LDI/LDIR/LDD/LDDR |
| Block Compare | CPI/CPIR/CPD/CPDR |
| Block I/O | INI/INIR/IND/INDR/OUTI/OTIR/OUTD/OTDR |
| I/O | IN A,(n) / OUT (n),A / IN r,(C) / OUT (C),r |
| Misc | DAA / CPL / SCF / CCF / NEG / RRD / RLD / NOP / HALT |
| Interrupt | DI / EI / IM 0/1/2 / RETI / RETN |

## Addressing Modes

| Mode | Syntax | Example |
|------|--------|---------|
| Register | `LD A,B` | 4T |
| Immediate | `LD A,$42` | 7T |
| Register Indirect | `LD A,(HL)` | 7T |
| Indexed | `LD A,(IX+5)` | 19T |
| Extended | `LD A,($1234)` | 13T |
| Implied | `NOP` | 4T |

## Architecture

### State Machine

```rust
enum ExecState {
    Fetch,                  // Read next opcode, increment R
    Execute(u8, u8),        // Execute main page opcode at cycle N
    ExecuteCB(u8, u8),      // CB prefix bit operations
    ExecuteED(u8, u8),      // ED prefix extended operations
    PrefixDD / PrefixFD,    // Index prefix: fetch next opcode
    PrefixIndexCB(u8),      // DD CB d / FD CB d: read displacement
    Interrupt(u8),          // Hardware interrupt response
}
```

### T-State Timing

Each `tick_with_bus()` call = 1 T-state. All instructions consume the correct number of T-states as verified by the SingleStepTests vectors:

- Simple register ops: 4T (NOP, LD r,r')
- Immediate loads: 7T (LD r,n)
- Memory ops: 7-19T depending on addressing mode
- Block ops: 16T (single) / 21T (repeat)
- Conditional branches: taken vs not-taken cycle counts differ

### Prefix System

The Z80's prefix bytes (CB, DD, ED, FD) modify subsequent instructions:

- **DD/FD**: Replace HL with IX/IY, H/L with IXH/IXL/IYH/IYL (undocumented), and (HL) with (IX+d)/(IY+d)
- **CB**: Select bit operation instruction page
- **DD CB d / FD CB d**: Indexed bit operations with displacement before opcode
- **ED**: Extended instruction page (block ops, 16-bit ALU, I/O)
- Consecutive DD/FD prefixes: only the last one takes effect

### Undocumented Behavior

All undocumented behaviors verified against SingleStepTests:

- **X/Y flags** (bits 3/5): Set from result, MEMPTR, or other sources per-instruction
- **MEMPTR (WZ)**: Internal register affecting flags in BIT, block ops, and I/O
- **IXH/IXL/IYH/IYL**: DD/FD prefix allows accessing index register halves
- **SLL** (CB 30-37): Shift left, bit 0 = 1 (undocumented CB instruction)
- **ED NOPs**: Undefined ED opcodes execute as 8T NOPs
- **SCF/CCF q flag**: X/Y source depends on whether prior instruction modified F
- **Block I/O repeat flags**: H and PV recomputed with adjusted B value during repeat

### Interrupts

- **NMI**: Edge-triggered, vectors through $0066, clears IFF1 (11T)
- **IRQ IM 0**: Execute instruction from data bus (13T)
- **IRQ IM 1**: Fixed vector $0038 (13T)
- **IRQ IM 2**: Vectored via I register + data bus (19T)
- **EI delay**: Interrupts not accepted until after instruction following EI
- **HALT**: Executes NOPs until interrupt accepted

## File Structure

```
core/src/cpu/z80/
  mod.rs        -- Z80 struct, state machine, dispatch, prefix handling (837 lines)
  alu.rs        -- 8/16-bit ALU, rotates, DAA, CPL, SCF, CCF, NEG, RRD/RLD (670 lines)
  load_store.rs -- Loads, stores, exchanges, I/O, LD A,I/R (807 lines)
  block.rs      -- Block transfer (LDI/LDIR), compare (CPI/CPIR), I/O (INI/OTIR) (481 lines)
  branch.rs     -- JP, JR, DJNZ, CALL, RET, RST, condition evaluation (448 lines)
  bit.rs        -- CB prefix: BIT/SET/RES, shifts/rotates (277 lines)
  stack.rs      -- PUSH/POP (64 lines)
```

## Resources

- [Z80 CPU User Manual](http://www.z80.info/zip/z80cpu_um.pdf) -- Official Zilog documentation
- [The Undocumented Z80 Documented](http://www.z80.info/zip/z80-documented.pdf) -- Sean Young's comprehensive reference
- [SingleStepTests/z80](https://github.com/SingleStepTests/z80) -- Reference test vectors (cross-validation)
- [Cross-validation details](../../../cpu-validation/README_z80.md)
