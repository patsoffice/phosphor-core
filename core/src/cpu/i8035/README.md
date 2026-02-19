# Intel I8035 (MCS-48) CPU

Cycle-accurate emulation of the Intel 8035 microprocessor (MCS-48 family), implementing all 229 defined opcodes. Cross-validated against [MAME](https://github.com/mamedev/mame)'s MCS-48 implementation.

## Status

| Metric | Value |
|--------|-------|
| Opcodes | 229 defined (27 undefined treated as NOP) |
| Unit tests | 148 |
| Cross-validation | Against MAME mcs48.cpp |
| Timing | Cycle-accurate (machine-cycle granularity) |

## Registers

| Register | Size | Description |
|----------|------|-------------|
| A | 8-bit | Accumulator |
| PC | 12-bit | Program counter (stored in u16) |
| PSW | 8-bit | Program status word [CY, AC, F0, BS, 1, SP2, SP1, SP0] |
| F1 | 1-bit | User flag 1 (not in PSW) |
| T | 8-bit | Timer/counter register |
| R0-R7 | 8-bit | General-purpose registers (in internal RAM, 2 banks) |
| DBBB | 8-bit | BUS port latch (DB bus buffer) |
| P1 | 8-bit | Port 1 output latch |
| P2 | 8-bit | Port 2 output latch |

## Instruction Set

229 opcodes across a single opcode page:

| Category | Count | Details |
|----------|-------|---------|
| ALU (register) | 60 | ADD, ADDC, ANL, ORL, XRL -- @Ri (x2) and Rn (x8) variants |
| ALU (immediate) | 5 | ADD, ADDC, ANL, ORL, XRL -- #data |
| Accumulator unary | 10 | DEC, INC, CLR, CPL, SWAP, DA, RRC, RR, RL, RLC |
| Register INC/DEC | 18 | INC @Ri (x2), INC Rn (x8), DEC Rn (x8) |
| Data movement | 35 | MOV A↔Rn/@Ri, XCH, XCHD, MOV A↔T, MOV A↔PSW, MOV #imm |
| External memory | 4 | MOVX A,@Ri / MOVX @Ri,A |
| Program memory | 2 | MOVP A,@A / MOVP3 A,@A |
| Port I/O | 6 | IN A,P1/P2, INS A,BUS, OUTL BUS/P1/P2,A |
| Port RMW | 6 | ORL/ANL BUS/#data, ORL/ANL P1/#data, ORL/ANL P2/#data |
| Expander ports | 16 | MOVD, ANLD, ORLD -- 4-bit I/O via P4-P7 |
| Jumps | 17 | JMP (8 pages), JMPP @A, CALL (8 pages), RET, RETR |
| Conditional jumps | 20 | JC, JNC, JZ, JNZ, JF0, JF1, JBb (x8), JT0/1, JNT0/1, JTF, JNI |
| DJNZ | 8 | Decrement Rn and jump if non-zero |
| Status flags | 6 | CLR/CPL CY, CLR/CPL F0, CLR/CPL F1 |
| Control | 11 | EN/DIS I, EN/DIS TCNTI, STRT T/CNT, STOP TCNT, SEL RB0/1, SEL MB0/1 |
| NOP | 1 | No operation |
| Undefined | 27 | Treated as NOP |

## Addressing Modes

| Mode | Syntax | Cycles | Description |
|------|--------|--------|-------------|
| Inherent | `INC A` | 1 | Register-only, no operand |
| Immediate | `ADD A,#42h` | 2 | Operand follows opcode |
| Register | `ADD A,R3` | 1 | Register specified in opcode bits |
| Indirect | `MOV A,@R0` | 1 | R0 or R1 as pointer into internal RAM |
| External indirect | `MOVX A,@R0` | 2 | R0/R1 as pointer to external data memory |

## Architecture

### Harvard Architecture

The MCS-48 uses separate address spaces for program and data memory:

- **Program memory**: Up to 4KB (12-bit address), accessed via `read()` on the bus
- **External data memory**: Up to 256 bytes, accessed via `io_read()`/`io_write()` on the bus
- **Internal RAM**: 64 bytes (8035), contains registers, stack, and general-purpose storage

### Internal RAM Layout (64 bytes)

```
0x00-0x07  Register bank 0 (R0-R7)
0x08-0x17  Stack (8 levels × 2 bytes, stores PC[11:0] + PSW[7:4])
0x18-0x1F  Register bank 1 (R0-R7)
0x20-0x3F  General purpose RAM
```

### State Machine

```rust
enum ExecState {
    Fetch,           // Read next opcode, execute 1-cycle ops immediately
    Execute(u8),     // Second cycle of 2-cycle instructions
    Interrupt(u8),   // Hardware interrupt entry sequence (3 cycles)
    Stopped,         // Reserved idle state
}
```

### Timing

- Each `tick_with_bus()` call = 1 machine cycle (15 oscillator clocks)
- 1-cycle instructions: fetch + execute in a single machine cycle
- 2-cycle instructions: fetch in cycle 0, operand read + execute in cycle 1
- Interrupt entry: 3 machine cycles (detect → push PC+PSW → vector jump)

### Interrupts

- **External INT** -- Level-triggered, masked by `int_enabled`, vectors to 0x003
- **Timer/Counter overflow** -- Masked by `tcnti_enabled`, vectors to 0x007
- **Priority**: External INT > Timer overflow
- Interrupt entry pushes PC and PSW to internal stack, sets `in_interrupt` flag
- `RETR` clears `in_interrupt` and restores PSW; `RET` does not restore PSW

### Timer/Counter

- **Timer mode** (`STRT T`): 5-bit prescaler divides by 32; T register increments every 32 machine cycles
- **Counter mode** (`STRT CNT`): T register increments on T1 pin falling edge
- Overflow (0xFF → 0x00) sets `timer_overflow` flag and optionally triggers interrupt
- `JTF` tests and auto-clears the overflow flag

### Bus Mapping

| Address Range | Bus Method | Purpose |
|---------------|-----------|---------|
| 0x0000-0x0FFF | `read()` | Program memory (opcode fetch, MOVC) |
| 0x00-0xFF | `io_read()`/`io_write()` | External data memory (MOVX) |
| 0x100 | `io_read()`/`io_write()` | BUS port |
| 0x101-0x102 | `io_read()`/`io_write()` | P1, P2 |
| 0x104-0x107 | `io_read()`/`io_write()` | P4-P7 (expander ports) |
| 0x110-0x111 | `io_read()` | T0, T1 pins |

### Key Differences from Other CPUs

- Harvard architecture (separate program/data memory spaces)
- Internal RAM for registers, stack, and scratch -- not on the external bus
- Stack lives inside the chip (8 levels max, not arbitrary depth)
- 12-bit program counter (4KB address space, banked via A11/MB flag)
- No general-purpose memory-mapped I/O -- ports and external data use separate bus methods

## File Structure

```
core/src/cpu/i8035/
  mod.rs        -- I8035 struct, state machine, dispatch, control ops (643 lines)
  alu.rs        -- Flag helpers, ALU operations (126 lines)
  branch.rs     -- Jumps, calls, returns, conditional branches (353 lines)
  load_store.rs -- Data movement, MOVX, MOVP, port I/O, expander ports (536 lines)
```

## Resources

- [MCS-48 Datasheet](http://www.bitsavers.org/components/intel/MCS48/) -- Official Intel datasheet and user's manual
- [MAME MCS-48 Core](https://github.com/mamedev/mame/blob/master/src/devices/cpu/mcs48/mcs48.cpp) -- Reference implementation
- [Cross-validation details](../../cpu-validation/README_i8035.md)
