# MOS 6502 CPU

Cycle-accurate emulation of the NMOS MOS Technology 6502 microprocessor, implementing all 151 legal opcodes with bus-level accuracy. Validated against [SingleStepTests/65x02](https://github.com/SingleStepTests/65x02) with 1,510,000 test vectors (100% pass rate).

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 151 (all legal NMOS) |
| Unit tests | 255 |
| Cross-validation | 1,510,000/1,510,000 (100%) |
| Timing | Cycle-accurate with bus-level traces |

## Registers

| Register | Size | Description |
|----------|------|-------------|
| A | 8-bit | Accumulator |
| X | 8-bit | Index register X |
| Y | 8-bit | Index register Y |
| SP | 8-bit | Stack pointer (page 1: $0100-$01FF) |
| PC | 16-bit | Program counter |
| P | 8-bit | Processor status (N, V, -, B, D, I, Z, C) |

## Instruction Set

151 legal NMOS 6502 opcodes:

| Category | Count | Details |
|----------|-------|---------|
| Load (LDA/LDX/LDY) | 18 | LDA: 8 modes, LDX: 5 modes, LDY: 5 modes |
| Store (STA/STX/STY) | 13 | STA: 7 modes, STX: 3 modes, STY: 3 modes |
| Arithmetic (ADC/SBC) | 16 | 8 addressing modes each, with BCD support |
| Compare (CMP/CPX/CPY) | 14 | CMP: 8 modes, CPX: 3 modes, CPY: 3 modes |
| Logical (AND/ORA/EOR) | 24 | 8 addressing modes each |
| BIT test | 2 | Zero page, absolute |
| Shift/Rotate | 20 | ASL, LSR, ROL, ROR: accumulator + 4 memory modes |
| Memory INC/DEC | 8 | 4 addressing modes each |
| Branch | 8 | BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ |
| Jump/Subroutine | 5 | JMP abs, JMP (ind), JSR, RTS, RTI |
| Stack | 4 | PHA, PLA, PHP, PLP |
| Flag set/clear | 7 | CLC, SEC, CLI, SEI, CLV, CLD, SED |
| Transfer | 6 | TAX, TAY, TXA, TYA, TSX, TXS |
| Register INC/DEC | 4 | INX, INY, DEX, DEY |
| Misc | 2 | NOP, BRK |

## Addressing Modes

| Mode | Syntax | Cycles | Description |
|------|--------|--------|-------------|
| Immediate | `LDA #$42` | 2 | Operand follows opcode |
| Zero Page | `LDA $10` | 3 | 8-bit address (page 0) |
| Zero Page,X | `LDA $10,X` | 4 | ZP + X, wraps within page 0 |
| Zero Page,Y | `LDX $10,Y` | 4 | ZP + Y, wraps within page 0 |
| Absolute | `LDA $1234` | 4 | Full 16-bit address |
| Absolute,X | `LDA $1234,X` | 4+ | Absolute + X, +1 if page cross (reads) |
| Absolute,Y | `LDA $1234,Y` | 4+ | Absolute + Y, +1 if page cross (reads) |
| (Indirect,X) | `LDA ($10,X)` | 6 | ZP pointer pre-indexed by X |
| (Indirect),Y | `LDA ($10),Y` | 5+ | ZP pointer post-indexed by Y, +1 page cross |
| Implied | `INX` | 2 | No operand |
| Accumulator | `ASL A` | 2 | Operates on accumulator |

Page-crossing penalty: For read operations, +1 cycle only when index addition crosses a 256-byte page boundary. For stores and RMW operations, the penalty cycle **always** occurs.

## Architecture

### State Machine

```rust
enum ExecState {
    Fetch,            // Read next opcode
    Execute(u8, u8),  // Execute opcode at cycle N
    Interrupt(u8),    // Hardware interrupt sequence
}
```

### Bus Behavior

The NMOS 6502 performs a bus read or write on **every** cycle -- there are no internal-only cycles. What would be "dead" cycles are instead dummy reads from predictable addresses:

- Implied instructions: dummy read from PC
- Stack pulls: dummy read from stack[SP] before increment
- Indexed ZP: dummy read from un-indexed ZP address
- RMW: writes old value back before writing modified value

### BCD Arithmetic

The 6502's decimal mode (D flag) implements NMOS-specific flag behavior:
- **ADC**: N and V from intermediate binary result, Z from binary result, C from BCD result
- **SBC**: All flags from binary result; only the accumulator gets BCD correction

This matches the hardware quirks documented in the NMOS 6502 datasheet and verified by the SingleStepTests vectors.

### Interrupts

- **NMI** -- Edge-triggered, vectors through $FFFA/$FFFB, pushes PC and P (B=0)
- **IRQ** -- Level-triggered, masked by I flag, vectors through $FFFE/$FFFF, pushes PC and P (B=0)
- **BRK** -- Software interrupt, same vector as IRQ ($FFFE), pushes PC+2 and P (B=1)

BRK and IRQ are distinguished by the B flag in the pushed status byte.

### NMOS Hardware Quirks

- **JMP indirect page wrap**: `JMP ($xxFF)` fetches the high byte from `$xx00` instead of `$(xx+1)00`
- **RMW write-back**: Memory RMW instructions write the original value, then the modified value
- **Branch timing**: 2 cycles (not taken), 3 cycles (taken, no page cross), 4 cycles (taken, page cross)

## File Structure

```
core/src/cpu/m6502/
  mod.rs        -- M6502 struct, state machine, dispatch, implied ops (610 lines)
  alu.rs        -- Flag helpers, addressing mode helpers (960 lines)
  binary.rs     -- ADC, SBC, CMP, CPX, CPY, AND, ORA, EOR, BIT (624 lines)
  shift.rs      -- ASL, LSR, ROL, ROR memory modes (172 lines)
  unary.rs      -- INC, DEC memory modes (120 lines)
  load_store.rs -- LDA, LDX, LDY, STA, STX, STY (393 lines)
  branch.rs     -- Branches, JMP, JSR, RTS, RTI (326 lines)
  stack.rs      -- PHA, PLA, PHP, PLP, BRK, interrupt handler (162 lines)
```

## Resources

- [6502 Reference](http://www.obelisk.me.uk/6502/reference.html) -- Instruction reference
- [SingleStepTests/65x02](https://github.com/SingleStepTests/65x02) -- Reference test vectors (cross-validation)
- [NMOS 6502 Datasheet](http://archive.6502.org/datasheets/mos_6502_mpu_nov_1985.pdf) -- Official MOS Technology datasheet
- [Cross-validation details](../../cpu-validation/README_6502.md)
