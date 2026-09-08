# Intel 8088 CPU

Instruction-level emulation of the Intel 8088 microprocessor, implementing 279 opcodes across all major instruction categories. The 8088 is the 8-bit external bus variant of the 8086, used in the original IBM PC and Gottlieb System 80 arcade boards. Validated against [SingleStepTests/8088](https://github.com/SingleStepTests/8088) with 2,577,000 test vectors (100% pass rate).

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 279 (documented + sub-opcode variants) |
| Unit tests | 325 |
| Cross-validation | 2,797,000/2,797,000 (100%) |
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

### Bus interface unit and execution unit

One `execute_cycle()` is one T-state, and two things run in it independently.

The **BIU** keeps the four-byte prefetch queue full. When there is room it runs
a CODE bus cycle, four T-states long: the address is latched on T1 and the byte
arrives on T3. A fetch that ends with room left runs straight into the next one,
back to back; restarting from idle costs two idle cycles first. Its fetch
pointer is separate from IP and runs ahead of it by however many bytes are
queued.

The **EU** takes one byte per T-state out of the queue and stalls when it is
empty. It walks the shape of an instruction (opcode, ModR/M, displacement,
immediate) using the length table in `format.rs`, and runs the instruction when
the last byte arrives. A control transfer flushes the queue on the following
T-state, which is what a taken branch costs.

```rust
enum Biu {
    Idle,                            // queue full, nothing to do
    Restarting(u8),                  // counting idle cycles before T1
    Fetching { t: u8, addr: u32 },   // inside a CODE bus cycle
}
```

The queue is directly observable, and validated as such: `queue_status` carries
the QS0/QS1 lines, and all 3,007,000 recorded vectors agree with this core about
which bytes the EU took out of the queue, in what order, and where it was
flushed.

**Execution is still atomic.** An instruction's operand reads and writes, and
the cycles the EU spends computing an effective address, all happen on the
T-state its last byte arrives. That is why the Timing row above still says
instruction-level, and it is the next milestone.

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

22 opcode files are skipped in validation (301 pass out of 323 total):

| Opcodes | Reason |
|---------|--------|
| 0x26, 0x2E, 0x36, 0x3E, 0xF0-0xF3 | Prefix bytes (no standalone execution) |
| 0xE4-0xE7, 0xEC-0xEF | IN/OUT: test vectors embed I/O data in cycle array, not RAM |
| 0xF4 | HLT: blocks forever in test harness (no interrupt source) |
| 0xD8-0xDF | FPU ESC opcodes (no 8087 coprocessor) |
| 0xD6 | SALC (undocumented) |
| 0x0F | POP CS (undocumented) |
| 0xD0.6, 0xD1.6, 0xD2.6, 0xD3.6 | SETMO/SETMOC (undocumented) |
| 0xFF.7 | Undefined sub-opcode |

22 files came off this list on 2026-09-04, and not one of them was found by this
gate. All four defects were about instruction *length* or about *control flow*,
neither of which a state-only comparison can see: it checks what an instruction
leaves behind, not what it did on the way.

- **0x60-0x6F** are the conditional jumps sixteen above them, not
  "hardware-dependent aliases". The suite's hardware capture disassembles 0x60
  as JO, 0x65 as JNZ, 0x6A as JP and 0x6F as JNLE, each with a rel8. Dispatch
  now covers `0x60..=0x7F` and the low four bits select the condition either
  way. Found by the loader: it fetched the rel8 the executor never consumed.
- **0xF6.1 and 0xF7.1** are TEST, the same instruction as 0xF6.0 and 0xF7.0.
  The group's dispatch handled reg=0 and left reg=1 doing nothing, so its
  immediate was never consumed. Found the same way.
- **0xC0, 0xC1, 0xC8 and 0xC9** are RET and RETF, aliases of 0xC2, 0xC3, 0xCA
  and 0xCB. They consumed their bytes but did not return, so they never flushed
  the prefetch queue. Found by the queue: the recorded traces end with an `E`
  this core did not produce.

An opcode with no implementation here still consumes its operand bytes, which is
what the part does: it fetches every byte of an instruction whether or not it
acts on one. The remaining skipped opcodes are skipped for their semantics, not
their length.

## Resources

- [Intel 8088 Data Sheet](https://datasheets.chipdb.org/Intel/x86/808x/datashts/8088/231456-006.pdf) -- Official Intel documentation
- [SingleStepTests/8088](https://github.com/SingleStepTests/8088) -- Reference test vectors (cross-validation)
- [Cross-validation details](../../../cpu-validation/README_i8088.md)
