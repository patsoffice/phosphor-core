# Motorola 6800 CPU

Cycle-accurate emulation of the Motorola 6800 microprocessor, implementing all 192 opcodes. Cross-validated against [mame4all](https://github.com/mamedev/mame)'s M6800 implementation with 192,000 test vectors (99.998% pass rate).

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 192 |
| Unit tests | 343 |
| Cross-validation | 191,996/192,000 (99.998%) |
| Timing | Cycle-accurate |

## Registers

| Register | Size | Description |
|----------|------|-------------|
| A | 8-bit | Accumulator A |
| B | 8-bit | Accumulator B |
| X | 16-bit | Index register |
| SP | 16-bit | Stack pointer (points to next free location) |
| PC | 16-bit | Program counter |
| CC | 6-bit | Condition codes (H, I, N, Z, V, C) -- bits 6-7 always read as 1 |

## Instruction Set

192 opcodes across a single opcode page:

| Category | Count | Details |
|----------|-------|---------|
| ALU (A register) | 10 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA, LDAA -- imm/dir/idx/ext |
| ALU (B register) | 10 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB, LDAB -- imm/dir/idx/ext |
| Store (A/B) | 2 | STAA, STAB -- dir/idx/ext |
| 16-bit loads | 3 | CPX, LDS, LDX -- imm/dir/idx/ext |
| 16-bit stores | 2 | STS, STX -- dir/idx/ext |
| Unary (inherent) | 22 | NEG, COM, LSR, ROR, ASR, ASL, ROL, DEC, INC, TST, CLR (A & B) |
| Unary (memory) | 24 | NEG, COM, LSR, ROR, ASR, ASL, ROL, DEC, INC, TST, JMP, CLR -- idx/ext |
| Branch | 15 | BRA, BHI, BLS, BCC, BCS, BNE, BEQ, BVC, BVS, BPL, BMI, BGE, BLT, BGT, BLE |
| Jump/Subroutine | 5 | BSR, JSR (idx/ext), RTS, RTI |
| Stack | 4 | PSHA, PSHB, PULA, PULB |
| Transfer/Flag | 10 | TAP, TPA, TAB, TBA, CLC, SEC, CLV, SEV, CLI, SEI |
| Register ops | 4 | INX, DEX, INS, DES |
| Misc | 5 | NOP, DAA, ABA, SBA, CBA, TSX, TXS |
| Interrupt | 3 | SWI, WAI, RTI |

## Addressing Modes

| Mode | Syntax | Cycles | Description |
|------|--------|--------|-------------|
| Inherent | `INCA` | 2 | Register-only, no operand |
| Immediate | `LDAA #$42` | 2 | Operand follows opcode |
| Direct | `LDAA $10` | 3 | 8-bit address (always page 0, no DP register) |
| Indexed | `LDAA $10,X` | 5 | X register + unsigned 8-bit offset |
| Extended | `LDAA $1234` | 4 | Full 16-bit address |

Key difference from M6809: The M6800 has no direct page register -- direct addressing always references page 0 ($0000-$00FF). The indexed mode uses only the X register with unsigned offsets.

## Architecture

### State Machine

```rust
enum ExecState {
    Fetch,               // Read next opcode
    Execute(u8, u8),     // Execute opcode at cycle N
    Halted { .. },       // TSC/RDY asserted
    Interrupt(u8),       // Hardware interrupt sequence
    WaitForInterrupt,    // WAI wait state
}
```

### Interrupts

- **NMI** -- Edge-triggered, pushes all registers, vectors through $FFFC/$FFFD
- **IRQ** -- Level-triggered, masked by I flag, vectors through $FFF8/$FFF9
- **SWI** -- Software interrupt, vectors through $FFFA/$FFFB
- **WAI** -- Pushes all registers, halts until interrupt

### Key Differences from M6809

- No direct page register (direct mode always uses page 0)
- No Y or U registers
- No multi-byte opcode prefixes (single opcode page)
- 16-bit register ops (INX, DEX) take 4 cycles (vs 2 on M6809)
- CC bits 6-7 always read as 1 (TPA returns `CC | 0xC0`)
- Stack pointer points to next free location (TSX adds 1, TXS subtracts 1)

## File Structure

```
core/src/cpu/m6800/
  mod.rs        -- M6800 struct, state machine, dispatch, inherent ops (731 lines)
  alu.rs        -- Flag helpers, addressing mode helpers (547 lines)
  alu/binary.rs -- ADD, SUB, CMP, SBC, ADC, AND, BIT, EOR, ORA
  alu/shift.rs  -- ASL, ASR, LSR, ROL, ROR
  alu/unary.rs  -- NEG, COM, CLR, INC, DEC, TST
  branch.rs     -- Branches, BSR, JMP, JSR (407 lines)
  load_store.rs -- LDA/B, STA/B, LDX, STX, LDS, STS, CPX (481 lines)
  stack.rs      -- PSH/PUL, SWI, WAI, RTI, interrupt handler (353 lines)
```

## Resources

- [MC6800 Programming Manual](http://www.bitsavers.org/components/motorola/6800/Motorola_M6800_Programming_Reference_Manual_M68PRM_D_Nov76.pdf) -- Official Motorola programming reference
- [MC6800 Datasheet](http://www.bitsavers.org/components/motorola/6800/MC6800_8-Bit_Microprocessing_Unit.pdf) -- Instruction timing and pinout
- [MAME 6800 Core](https://github.com/mamedev/mame/tree/master/src/devices/cpu/m6800) -- Reference implementation
- [Cross-validation details](../../cpu-validation/README_6800.md)
