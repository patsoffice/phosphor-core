# Debugging: Dig Dug Hang Before Attract Mode

This documents the investigation of a Dig Dug emulation hang that occurred after
initial bring-up, where the game would freeze before reaching the attract mode
demo. The debugging process illustrates techniques for diagnosing behavioral
emulation bugs in a multi-CPU arcade system.

## System Overview

Dig Dug runs on Namco's Galaga-generation hardware:

- **3× Z80 CPUs** @ 3.072 MHz (main, sub, sound) sharing a memory bus
- **Namco 06XX**: Bus arbiter that generates periodic NMI to the main CPU
- **Namco 51XX**: Input multiplexer / credit manager
- **Namco 53XX**: DIP switch reader (MB8843 MCU, emulated behaviorally)
- **NMI DMA engine**: The main CPU's NMI handler at `0x0066` uses `EXX` to swap
  to alternate registers (BC'/DE'/HL') and performs one-byte `LDI` transfers per
  NMI, processing a command table when each DMA block completes.

## Symptom

After boot, the game would execute normally for a few seconds (EAROM access,
hardware init), then freeze permanently. The main CPU was stuck in a tight
`DJNZ` loop at `PC = 0x1BCC`, copying data but never progressing past it.

## Step 1: Frame-Boundary Hang Detection

Added a simple PC-sampling hang detector to `DigDugSystem` that checks the main
CPU's program counter at each frame boundary:

```rust
// Fields added to DigDugSystem
hang_detect_pc: u16,
hang_detect_count: u32,
hang_detected: bool,

fn check_hang(&mut self) {
    let pc = self.board.main_cpu.pc;

    // Check if PC is stuck in a small range (tight polling loop)
    if pc.wrapping_sub(self.hang_detect_pc) <= 8 {
        self.hang_detect_count += 1;
    } else {
        self.hang_detect_pc = pc;
        self.hang_detect_count = 0;
        self.hang_detected = false;
    }

    // After ~2 seconds (120 frames at 60 Hz), report the hang
    if self.hang_detect_count >= 120 && !self.hang_detected {
        self.hang_detected = true;
        // ... dump diagnostics ...
    }
}
```

The 8-byte PC window accounts for the fact that a tight loop (like `DJNZ`)
bounces between a few addresses. The 120-frame threshold filters out legitimate
waits (like EAROM access delays which resolve after ~60 frames).

**Result**: Confirmed the hang at `PC = 0x1BCC` with `B = 0x08` (DJNZ counter).
The sub CPU was at its normal IRQ service address — not stuck.

## Step 2: NMI Delivery Investigation

The DJNZ loop at `0x1BCC` is part of the NMI DMA engine — it copies data using
alternate registers while the main program waits. The DMA should advance each
time the 06XX timer fires an NMI. If NMIs weren't arriving, the DMA would never
complete and the main loop would spin forever.

Added Z80 execution state and 06XX timer internals to the diagnostics:

```rust
eprintln!("  Main state: {:?} nmi_prev={} ei_delay={}",
    cpu.state, cpu.nmi_previous, cpu.ei_delay);
eprintln!("  06XX NMI pend: {} timer_run={} cnt={} per={} st={} stretch={}",
    self.board.main_nmi_pending,
    self.board.namco06.timer_running,
    self.board.namco06.timer_counter,
    self.board.namco06.timer_period,
    self.board.namco06.timer_state,
    self.board.namco06.read_stretch);
```

**Result**: The timer was running (`timer_run=true`, `per=2048`) but
`read_stretch=true`. This flag suppresses the first NMI pulse after a read-mode
control write. It initially looked like a smoking gun — NMIs were being blocked.

## Step 3: Frame+1 Comparison

To confirm the CPU was truly stuck (not just caught at an unlucky moment), added
a one-frame-later check:

```rust
if self.hang_detect_count == 121 {
    eprintln!("=== FRAME+1 CHECK ===");
    eprintln!("  Main PC: 0x{:04X}  B={:02X}  BC'={:02X}{:02X}",
        pc, cpu.b, cpu.b_prime, cpu.c_prime);
}
```

**Result**: `B=08` and `BC'=0002` were identical one frame later — confirming
zero progress.

## Step 4: 06XX Control Write Tracing

Rather than assuming `read_stretch` was the root cause, we traced every write to
the 06XX control register during the hang to see the actual timer lifecycle:

```rust
// In Bus::write() for address 0x7100 (06XX control)
0x7100 => {
    if self.hang_detect_count >= 100 {
        eprintln!("  [06XX CTRL WRITE] data=0x{:02X} clk={} stretch_was={}",
            data, self.board.clock, self.board.namco06.read_stretch);
    }
    self.board.namco06.ctrl_write(data);
}
```

**Result**: This revealed a crucial insight. The ctrl_write trace showed a
repeating pattern of:

```
data=0x10  (stop timer)
data=0xD2  (select chip 2 = 53XX, read mode, divider=6)
```

Every write showed `stretch_was=false` — the stretch flag was being properly
cleared between timer cycles. The DMA **was** completing every ~10,350 cycles.
The CPU was not stuck at all — it was running the main game loop, which itself
was spinning waiting for game logic to advance.

This disproved the NMI hypothesis. The real problem was upstream: the game logic
was stuck in a polling loop.

## Step 5: Tracing the Game Logic

With NMI delivery confirmed working, the focus shifted to *what* the game was
waiting for. The ctrl_write pattern revealed that after the first temporary hang
resolved (EAROM init), the game only issued 53XX reads — no more 51XX reads.
This meant the game was stuck in a phase that polls DIP switch data.

Examining the sub CPU's IRQ handler, we found it checks bit 5 of shared RAM
address `$87CF`:

```
; Sub CPU IRQ handler (simplified)
LD   A, ($87CF)
BIT  5, A
RET  Z          ; ← exits early if bit 5 is clear!
; ... game logic continues ...
```

If bit 5 of `$87CF` is 0, the sub CPU IRQ handler does nothing, and game logic
never advances.

## Step 6: Finding the Root Cause

Address `$87CF` holds the last nibble returned by the 53XX chip during DMA.
The 53XX returns 4 nibbles per read cycle:

| Index | Data              |
|-------|-------------------|
|   0   | DSWA low nibble   |
|   1   | DSWA high nibble  |
|   2   | DSWB low nibble   |
|   3   | DSWB high nibble  |

Our 53XX model was returning `nibble | 0x10` — a fixed bit-4 flag with bit 5
always 0:

```rust
// WRONG — bit 5 always clear
nibble | 0x10
```

The real MB8843 MCU firmware (mode 7, used by Dig Dug) encodes the **port
index** in the upper nibble:

```rust
// CORRECT — port index encodes bit 5 for ports 2-3
(port_index << 4) | nibble
```

For ports 0-1 (DSWA), the upper nibble is `0x00` or `0x10`. For ports 2-3
(DSWB), it's `0x20` or `0x30` — which sets bit 5. The game uses bit 5 as a
"data valid" indicator that signals the DSWB portion has been read.

## The Fix

Two changes in [namco53.rs](core/src/device/namco53.rs):

```rust
pub fn read(&mut self, dswa: u8, dswb: u8) -> u8 {
    let idx = self.read_index;
    self.read_index = (self.read_index + 1) % 4;

    let nibble = match idx {
        0 => dswa & 0x0F,
        1 => (dswa >> 4) & 0x0F,
        2 => dswb & 0x0F,
        3 => (dswb >> 4) & 0x0F,
        _ => unreachable!(),
    };

    // Port index in upper nibble — game checks bit 5 as "data valid"
    (idx << 4) | nibble
}
```

And in [namco_galaga.rs](machines/src/namco_galaga.rs), set proper DIP switch
defaults matching MAME/factory settings (`dswa: 0x99, dswb: 0x24`) instead of
the previous `0xFF/0xFF` which encoded invalid option combinations.

## Takeaways

1. **Instrument iteratively.** Start with coarse detection (PC sampling), then
   add layers of detail as hypotheses form. Each round narrows the search space.

2. **Trace the data, not just the mechanism.** The NMI timer was working
   correctly the whole time. The bug was in the *data* being transferred by the
   DMA, not the DMA mechanism itself. Tracing control register writes proved the
   timer was cycling properly, which redirected the investigation.

3. **Frame+1 comparison is cheap and decisive.** Comparing two consecutive
   frames of state immediately distinguishes "truly stuck" from "caught at an
   unlucky moment."

4. **Behavioral emulation of MCUs needs precise output encoding.** The 53XX
   is a full microcontroller running firmware. Getting the high-level behavior
   right (returning DIP switch nibbles) isn't enough — the bit-level encoding
   of the output must match what the game code expects.

5. **Check the consumer, not just the producer.** The breakthrough came from
   reading the sub CPU's IRQ handler to understand what it expected from the
   shared RAM, then tracing backwards to the 53XX output format.
