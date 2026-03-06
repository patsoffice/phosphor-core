# Atari Mathbox

Hardware math coprocessor used in Battlezone (1980), Red Baron (1980), and Tempest (1981).

## Hardware

The mathbox is a micro-coded arithmetic processor built from four 4-bit AMD 2901 bit-slice ALUs forming a 16-bit datapath, running at 3 MHz — roughly twice the speed of the 1.5 MHz 6502 host CPU. It executes 24-bit microcode instructions from six 256×4 PROMs (256 words × 24 bits), with the D and Y busses tied together to create an 8-bit bidirectional interface to the 6502 data bus.

The high and low 2901 pairs have separate I2 inputs, enabling independent byte-level operations for loading 16-bit registers from 8-bit CPU writes.

## Registers

The Atari documentation names the registers A, B, E, F, X, Y, X', Y', N, Z. MAME maps these to R0–RF:

| MAME | Atari Name | Write Offsets | Purpose |
| ---- | ---------- | ------------- | ------- |
| R0 | A | $00/$01 | Rotation coefficient (cos θ) |
| R1 | B | $02/$03 | Rotation coefficient (sin θ) |
| R2 | E | $04/$05 | Input coordinate / distance operand |
| R3 | F | $06/$07 | Input coordinate / distance operand |
| R4 | X | $08/$09 | Working coordinate (modified by rotation) |
| R5 | Y | $0A/$0B | Working coordinate (modified by rotation) |
| R6 | N | $0C | Division step counter (quotient bit count) |
| R7 | X' | $15/$16 | Rotation/division result |
| R8 | Y' | $1A/$1B | Rotation/division result |
| R9 | — | — | Internal (rotation intermediate) |
| RA | Z_low | $0D/$0E | Alternate division dividend (low) |
| RB | Z_high | $0F/$10 | Alternate division dividend (high) |
| RC–RF | — | — | Internal temporaries |

Values are signed 16-bit fixed-point fractions (1.15 format). Multiplication results are shifted right by 16 bits with rounding.

## CPU Interface

- **32 command addresses** ($00–$1F): writing a byte loads a register and/or triggers computation
- **Status register**: D7=1 when busy (in practice, the CPU simply waits the known cycle count; emulation treats completion as instantaneous)
- **Result output**: 16-bit value read as two bytes — YLOW (low) and YHIGH (high)

On Tempest: status at $6040, result low at $6060, result high at $6070, commands at $6080–$609F.

## Operations

### Register Loads ($00–$0A, $0C–$10, $15–$16, $1A–$1B)

Load low or high byte of a working register. Each write also latches the register value as the result.

### Rotation ($0B, $11, $12)

2D matrix multiply for coordinate rotation using sin/cos coefficients in A (R0) and B (R1):

- **$0B** — "Multiply" (54–59 cycles): R4 -= R2, R5 -= R3, then:
  - R7 (X') = A\*X − B\*Y  (with rounding)
  - R8 (Y') = B\*X + A\*Y  (with rounding)
  - Falls through to division ($13)
- **$11** — "Complete Division" (119–131+5N cycles): same multiply as $0B but without the initial subtract; sets REGf=0 to skip the fallthrough, then continues to compute Y' and divide
- **$12** — "Y' Multiply" (50–55 cycles): computes only the R8 (Y') half of the rotation

### Division ($13, $14)

Iterative non-restoring division controlled by R6 (N = quotient bit count):

- **$13** — "Y'/X' Divide" (11–13+5N cycles): divides R7 by (R8:R9)
- **$14** — "Z/X' Divide" (10–12+5N cycles): divides R7 by (RB:RA)

Battlezone uses N=10 (10 iterations) instead of 16, scaling screen coordinates down by 64× to compensate for the reduced precision.

### Distance Approximation ($1D, $1E)

Fast octagonal approximation of sqrt(dx² + dy²):

```test
result = max(|dx|, |dy|) + 3/8 × min(|dx|, |dy|)
```

- **$1D**: computes |R2 − R0| and |R3 − R1| first, then approximates
- **$1E**: approximates using R2 and R3 directly

### Window Test ($1C)

Midpoint subdivision for line clipping. The hardware iterates R6 times, bisecting the interval between (R4, R5) and (R7, R8) at each step.

## Microcode Architecture

The 256×24-bit microcode word is organized as:

| Bits | Field | Purpose |
| ---- | ----- | ------- |
| 23–20 | A select | Source register / jump address |
| 19–16 | B select | Destination register |
| 15 | I2HI | High-byte 2901 source control |
| 14 | I2LO | Low-byte 2901 source control |
| 13–12 | I1, I0 | Operand pair selection |
| 11 | STALL | Halt execution at instruction end |
| 10–8 | I5–I3 | ALU function selection |
| 7 | LDAB | Load jump target latch |
| 6–4 | I8–I6 | Destination selection |
| 3 | SIGN | Arithmetic vs logical shift |
| 2 | JMP | Conditional jump (if MSB\*=0) |
| 1 | MULT | Conditional ADD (inverts I1 if Q0=0) |
| 0 | CARIN | Carry-in control |

The MULT bit enables conditional addition (CADD), which changes `ADD n,m` to `ADD 0,m` when Q0=0 — the key mechanism for implementing multiplication via shift-and-conditionally-add.

## Reference

- <https://6502disassembly.com/va-battlezone/mathbox.html>
- <https://github.com/historicalsource/battlezone/blob/main/MBUDOC.DOC>
- MAME `src/mame/atari/mathbox.cpp`
