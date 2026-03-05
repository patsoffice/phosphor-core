# Debugging: Dig Dug Emulation Issues

This documents two investigations of Dig Dug emulation bugs: (1) a hang before
attract mode, and (2) half-speed player movement. Both were caused by incorrect
53XX HLE output format. The debugging process illustrates techniques for
diagnosing behavioral emulation bugs in a multi-CPU arcade system.

## System Overview

Dig Dug runs on Namco's Galaga-generation hardware:

- **3x Z80 CPUs** @ 3.072 MHz (main, sub, sound) sharing a memory bus
- **Namco 06XX**: Bus arbiter that generates periodic NMI to the main CPU and
  multiplexes access to custom I/O chips via chip_select signals
- **Namco 51XX**: Input multiplexer / credit manager (MB8843 MCU, LLE)
- **Namco 53XX**: DIP switch reader (MB8843 MCU, HLE behavioral model)
- **NMI DMA engine**: The main CPU's NMI handler at `0x0066` uses `EXX` to swap
  to alternate registers (BC'/DE'/HL') and performs one-byte `LDI` transfers per
  NMI, processing a command table when each DMA block completes

### 06XX I/O Protocol

The game communicates with custom chips through a two-phase protocol:

1. Write a control byte to 06XX (`0x7100`): selects chip, read/write mode, and
   timer divider
2. The 06XX timer fires NMIs at intervals determined by the divider
3. Each NMI triggers one byte of DMA transfer to/from the selected chip
4. Write `0x10` to stop the timer and complete the transaction

Dig Dug uses two patterns: `0x71` (chip 0 = 51XX, 1 byte) and `0xD2` (chip 1 =
53XX, 2 bytes).

---

## Issue 1: Hang Before Attract Mode

### Symptom

After boot, the game executed normally for a few seconds (EAROM access, hardware
init), then froze permanently. The main CPU was stuck in a tight `DJNZ` loop at
`PC = 0x1BCC`.

### Hang Investigation

#### Step 1: Frame-Boundary Hang Detection

Added a PC-sampling hang detector that checks the main CPU's program counter
each frame. An 8-byte window accounts for tight loops (like `DJNZ`). A
120-frame threshold (approximately 2 seconds) filters out legitimate waits like EAROM
delays.

**Result**: Confirmed the hang at `PC = 0x1BCC` with `B = 0x08`.

#### Step 2: NMI Delivery Check

The DJNZ loop is part of the NMI DMA engine — it copies data while the main
program waits. If NMIs weren't arriving, the DMA would never complete.

Added 06XX timer internals to diagnostics. The timer was running correctly with
`read_stretch` properly clearing between cycles. A frame+1 comparison confirmed
the CPU was truly stuck (identical register state one frame later).

#### Step 3: Control Write Tracing

Traced every write to the 06XX control register. The DMA **was** completing
every ~10,350 cycles — the CPU was not stuck in DMA, it was running the main
game loop which itself was spinning waiting for game logic to advance.

This disproved the NMI hypothesis. The ctrl_write pattern showed the game only
issued 53XX reads (no 51XX reads), meaning it was stuck polling DIP switch data.

#### Step 4: Tracing Game Logic

The sub CPU's IRQ handler checks bit 5 of shared RAM address `$87CF`:

```asm
LD   A, ($87CF)
BIT  5, A
RET  Z          ; exits early if bit 5 is clear
```

Address `$87CF` holds the last byte returned by the 53XX during DMA. Our 53XX
HLE was returning `nibble | 0x10` (bit 5 always 0), so the sub CPU IRQ handler
always returned early and game logic never advanced.

### Fix

Changed the 53XX HLE to encode the port index in the upper nibble:
`(port_index << 4) | nibble`. For ports 2-3 (DSWB), this sets bit 5. Also set
proper DIP switch defaults matching MAME (`dswa: 0x99, dswb: 0x24`).

---

## Issue 2: Half-Speed Player Movement

### Symptom

After the initial hang was fixed, the game ran but player movement was half
speed. MAME shows dx=2 every 4 frames; our emulator showed dx=2 every 8 frames.

### Half-Speed Investigation

#### Step 1: Per-Frame Diagnostic Logging

Added per-frame logging of:
- 06XX ctrl register state
- Game counter at `$8423` (16-bit, controls game speed)
- I/O bytes read from custom chips
- Vblank delivery counts
- Player sprite position

**Finding**: The game counter at `$8423` incremented only every 2 frames instead
of every frame. Since the game waits for `counter + 4` to advance, half-speed
counter = half-speed movement.

#### Step 2: Vblank Handler Disassembly

Disassembled the vblank handler at `0x0280` (reached via RST 38H → JP 0x0280).
Found two paths:

1. **Fast path** (`$879A != 0`): Jump directly to counter increment at `0x02C0`
2. **Slow path** (`$879A == 0`): Perform LDIR copies (~5366 cycles), then check
   bit 5 of `$87CF`. If bit 5 is clear, **exit without incrementing counter**.

#### Step 3: Variable Tracking

Added watches on game variables:
- `$879A` (game mode): Always 0 during gameplay — the slow path always runs
- `$87CF` (53XX output): Alternates between `0x19` (bit 5 clear) and `0x32`
  (bit 5 set) on consecutive frames

This created a pattern where the counter only incremented on frames where
`$87CF` had bit 5 set, causing the every-other-frame behavior.

#### Step 4: Tracing the 53XX Output Pattern

The 53XX HLE cycled through 4 nibbles per read cycle:

| Index | Output | Bit 5 |
|-------|--------|-------|
| 0     | 0x09   | clear |
| 1     | 0x19   | clear |
| 2     | 0x24   | set   |
| 3     | 0x32   | set   |

Each `0xD2` transaction reads 2 bytes. The first transaction (indices 0,1)
stores `0x19` at `$87CF` — bit 5 clear. The second transaction (indices 2,3)
stores `0x32` — bit 5 set.

When the first D2 completed before vblank, the handler saw bit 5 clear and
skipped the counter increment. Only on alternating frames (when the second D2's
result was the latest) did the counter advance.

#### Step 5: Analyzing the Real 53XX Firmware

Fetched MAME's 53XX source. The 53XX is actually LLE in MAME — it runs real
MB8843 firmware (`53xx.bin`). The firmware packs two R-port nibbles per IRQ
using the carry flag to select the O port half:

- IRQ 0: `OUTO` with CF=0 (R0 → low nibble), `OUTO` with CF=1 (R1 → high
  nibble) → outputs full DSWA byte
- IRQ 1: Same with R2/R3 → outputs full DSWB byte

This confirmed by MAME's old (pre-LLE) HLE code:

```c
case 0: return READ_PORT(0) | (READ_PORT(1) << 4);  // DSWA
case 1: return READ_PORT(2) | (READ_PORT(3) << 4);  // DSWB
```

The firmware returns **full DIP switch bytes** cycling through **2 reads** (not
4 nibbles with port index encoding).

### Root Cause

Our 53XX HLE was wrong in two ways:

1. **4-nibble cycle instead of 2-byte cycle**: The real firmware packs two
   nibbles per IRQ, so each read returns a full DIP switch byte
2. **Spurious port index encoding**: `(idx << 4) | nibble` doesn't match the
   firmware output. The real chip outputs raw DIP switch bytes.

With the 4-nibble model, the port index in the upper nibble created an
artificial alternation of bit 5 between DSWA reads (bit 5 clear) and DSWB reads
(bit 5 set). The game's vblank handler uses bit 5 to detect whether all DIP
switch reads are complete (DSWB has bit 5 set in the actual DIP switch data).

### Fix

Changed the 53XX HLE from:

```rust
// Wrong: 4-nibble cycle with port index encoding
self.read_index = (self.read_index + 1) % 4;
(idx << 4) | nibble
```

To:

```rust
// Correct: 2-byte cycle returning raw DIP switch bytes
self.read_index = (self.read_index + 1) % 2;
match idx { 0 => dswa, 1 => dswb, _ => unreachable!() }
```

With `dswb = 0x24` (bit 5 = 1), the vblank handler now always sees bit 5 set
after a D2 transaction, and the counter increments every frame.

Also fixed the 51XX MCU clock divider: `ClockDivider::new(1, 2)` = CPU/2 =
1.536 MHz, matching MAME's `MASTER_CLOCK/6/2`. The previous `1:6` divisor gave
0.512 MHz (3x too slow), though this alone didn't cause the half-speed issue.

---

## Debugging Techniques

1. **PC-sampling hang detection**: Check program counter at frame boundaries
   with an 8-byte window and multi-frame threshold. Cheap and decisive for
   distinguishing real hangs from legitimate waits.

2. **Frame+1 comparison**: Comparing register state across two consecutive
   frames immediately distinguishes "truly stuck" from "caught at an unlucky
   moment."

3. **Control register tracing**: Logging 06XX ctrl writes reveals the I/O
   transaction pattern and proves timer/NMI delivery correctness without
   modifying the timer code.

4. **Variable watches**: Monitoring game RAM addresses at frame boundaries shows
   the game's internal state machine transitions. Adding writes watches (logging
   the writer's PC) traces causality.

5. **ROM disassembly at runtime**: Dumping ROM bytes from the emulator and
   feeding them to a disassembler reveals the game's actual logic without
   needing external ROM files.

6. **Trace the consumer, not just the producer**: Both bugs were found by
   reading the game code that *consumes* the 53XX output, then tracing
   backwards to the output format mismatch.

7. **Check reference implementations carefully**: MAME's 53XX was LLE (running
   actual firmware), while our HLE assumed a different output format. The old
   MAME HLE code (pre-0.131) documented the correct format in just two lines.
