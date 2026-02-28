# Intel 8088 CPU

Instruction-level emulation of the Intel 8088 microprocessor, implementing 279 opcodes across all major instruction categories. The 8088 is the 8-bit external bus variant of the 8086, used in the original IBM PC and Gottlieb System 80 arcade boards. Validated against [SingleStepTests/8088](https://github.com/SingleStepTests/8088) with 2,577,000 test vectors (100% pass rate).

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 279 (documented + sub-opcode variants) |
| Unit tests | 325 |
| Cross-validation | 2,577,000/2,577,000 (100%) |
| Timing | Instruction-level (not cycle-accurate) |

## Registers

| Register | Size | Description |
|----------|------|-------------|
| AX (AH:AL) | 16-bit | Accumulator |
| BX (BH:BL) | 16-bit | Base register |
| CX (CH:CL) | 16-bit | Count register |
| DX (DH:DL) | 16-bit | Data register |
| SP | 16-bit | Stack pointer |
| BP | 16-bit | Base pointer |
| SI | 16-bit | Source index |
| DI | 16-bit | Destination index |
| CS | 16-bit | Code segment |
| DS | 16-bit | Data segment |
| SS | 16-bit | Stack segment |
| ES | 16-bit | Extra segment |
| IP | 16-bit | Instruction pointer |
| FLAGS | 16-bit | Status and control flags |

### FLAGS Register

| Bit | Flag | Name |
|-----|------|------|
| 0 | CF | Carry |
| 2 | PF | Parity (even parity of low byte) |
| 4 | AF | Auxiliary carry (BCD half-carry) |
| 6 | ZF | Zero |
| 7 | SF | Sign |
| 8 | TF | Trap (single-step) |
| 9 | IF | Interrupt enable |
| 10 | DF | Direction (0=up, 1=down) |
| 11 | OF | Overflow |

Bits 12-15 and bit 1 are always 1 on the 8088.

### Memory Model

20-bit physical address = (segment << 4) + offset, giving 1 MB address space. The external bus is 8-bit, so 16-bit memory accesses require two bus cycles.

## Instruction Set

279 opcode sequences across the single-byte opcode map plus ModR/M sub-opcodes:

### Instruction Categories

| Category | Count | Instructions |
|----------|-------|-------------|
| Data movement | 36 | MOV (reg/mem/imm/seg), LEA, LES, LDS, PUSH, POP, XCHG |
| Arithmetic | 32 | ADD, ADC, SUB, SBB, INC, DEC, NEG, CMP, TEST |
| Logic | 12 | AND, OR, XOR, NOT |
| Shift/Rotate | 8 | SHL, SHR, SAR, ROL, ROR, RCL, RCR (by 1 or CL) |
| Multiply/Divide | 8 | MUL, IMUL, DIV, IDIV (byte and word), AAM, AAD |
| BCD | 4 | DAA, DAS, AAA, AAS |
| String ops | 10 | MOVS, CMPS, STOS, LODS, SCAS (byte/word, with REP) |
| Control flow | 24 | Jcc (16 conditions), JMP, CALL, JCXZ, LOOP/LOOPZ/LOOPNZ |
| Returns | 4 | RET, RETF (with/without SP adjust) |
| Interrupts | 4 | INT 3, INT n, INTO, IRET |
| Flag control | 9 | CLC, STC, CMC, CLD, STD, CLI, STI, SAHF, LAHF |
| Stack | 2 | PUSHF, POPF |
| I/O | 8 | IN, OUT (AL/AX, imm8/DX port) |
| Type conversion | 3 | CBW, CWD, XLAT |
| Segment push/pop | 7 | PUSH/POP ES, CS, SS, DS |
| Special | 2 | HLT, WAIT (NOP) |

### Addressing Modes

The ModR/M byte encodes 8 memory addressing modes (3 displacement variants each) plus register-direct:

| Mode | Effective Address | Default Segment |
|------|-------------------|-----------------|
| [BX+SI+disp] | BX + SI + disp | DS |
| [BX+DI+disp] | BX + DI + disp | DS |
| [BP+SI+disp] | BP + SI + disp | SS |
| [BP+DI+disp] | BP + DI + disp | SS |
| [SI+disp] | SI + disp | DS |
| [DI+disp] | DI + disp | DS |
| [BP+disp] | BP + disp | SS |
| [BX+disp] | BX + disp | DS |
| [disp16] | Direct address | DS |
| Register | r8 or r16 | (none) |

Displacement variants: none (mod=00), 8-bit sign-extended (mod=01), 16-bit (mod=10).

Segment override prefixes (CS:, DS:, ES:, SS:) can override the default segment for any memory operand.

### Prefixes

| Byte | Prefix | Description |
|------|--------|-------------|
| 0x26 | ES: | Segment override |
| 0x2E | CS: | Segment override |
| 0x36 | SS: | Segment override |
| 0x3E | DS: | Segment override |
| 0xF0 | LOCK | Bus lock (no-op in emulation) |
| 0xF2 | REPNZ | Repeat while not zero / not equal |
| 0xF3 | REP/REPZ | Repeat / repeat while zero / equal |

## Architecture

### State Machine

```rust
enum ExecState {
    Fetch,          // Read next opcode, consume prefixes
    Execute,        // Execute the decoded instruction
    Halted,         // HLT: wait for NMI or IRQ (if IF=1)
}
```

The 8088 uses instruction-level execution: each `tick_with_bus()` call executes one complete instruction. This differs from the cycle-accurate M6809 and Z80 implementations but is sufficient for the Gottlieb System 80 arcade board where cycle-level timing is not critical.

### Interrupts

- **NMI**: Edge-triggered, vectors through IVT entry 2 (0000:0008). Cannot be masked.
- **IRQ**: Level-triggered, masked by IF flag. Vector number provided by bus.
- **INT n**: Software interrupt to vector n. Pushes FLAGS, CS, IP; clears IF and TF.
- **IRET**: Restores IP, CS, FLAGS from stack.
- **Divide error**: INT 0 on DIV/IDIV overflow or divide-by-zero, and AAM with base=0.

The Interrupt Vector Table (IVT) occupies the first 1024 bytes of memory (256 vectors x 4 bytes each at 0000:0000).

### 8088-Specific Quirks

Verified against SingleStepTests hardware captures:

- **PUSH SP**: Pushes the decremented value of SP (unlike 286+)
- **Divide error IP**: Pushes the current IP (past the instruction), not the faulting instruction address (unlike 286+)
- **IDIV with REP prefix**: Undocumented — REP/REPNE prefix negates the quotient
- **IDIV quotient range**: -127..=127 (byte) and -32767..=32767 (word); the minimum value (-128/-32768) triggers a divide error
- **AAM base=0**: Updates SZP flags as if result were 0, then triggers INT 0
- **Divide error flags**: Arithmetic flags (CF, PF, AF, ZF, SF, OF) are undefined after a divide error — the 8088's internal division microcode modifies them unpredictably

## File Structure

```text
core/src/cpu/i8088/
  mod.rs        -- I8088 struct, state machine, interrupt dispatch
  registers.rs  -- Reg8, Reg16, SegReg enums and accessors
  flags.rs      -- FLAGS register helpers, parity table
  decode.rs     -- Prefix consumption, ModR/M parsing, push/pop
  addressing.rs -- Operand resolution, effective address calculation
  alu.rs        -- Arithmetic/logic operations, shifts, BCD
  execute.rs    -- Opcode dispatch, instruction implementation, tests
```

## Skipped Test Vectors

44 opcode files are skipped in validation (279 pass out of 323 total):

| Opcodes | Reason |
|---------|--------|
| 0x26, 0x2E, 0x36, 0x3E, 0xF0-0xF3 | Prefix bytes (no standalone execution) |
| 0xE4-0xE7, 0xEC-0xEF | IN/OUT: test vectors embed I/O data in cycle array, not RAM |
| 0xF4 | HLT: blocks forever in test harness (no interrupt source) |
| 0xD8-0xDF | FPU ESC opcodes (no 8087 coprocessor) |
| 0xD6 | SALC (undocumented) |
| 0x60-0x6F | Hardware-dependent aliases |
| 0xC0, 0xC1, 0xC8, 0xC9 | RET/RETF alias encodings |
| 0x0F | POP CS (undocumented) |
| 0xD0.6, 0xD1.6, 0xD2.6, 0xD3.6 | SETMO/SETMOC (undocumented) |
| 0xF6.1, 0xF7.1 | TEST aliases (undocumented duplicate encodings) |
| 0xFF.7 | Undefined sub-opcode |

## Resources

- [Intel 8088 Data Sheet](https://datasheets.chipdb.org/Intel/x86/808x/datashts/8088/231456-006.pdf) -- Official Intel documentation
- [SingleStepTests/8088](https://github.com/SingleStepTests/8088) -- Reference test vectors (cross-validation)
- [Cross-validation details](../../../cpu-validation/README_i8088.md)
