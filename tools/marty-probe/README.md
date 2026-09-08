# marty-probe

Runs one case of the 8088 test corpus through the reference emulator and prints
what it did, cycle by cycle, with **the microcode line that spent each clock**.

## Why this exists

The reference is straight-line imperative code that calls `cycle()` as it goes,
so "which T-state does 0x0c7 spend?" has no answer you can read off a line: it
depends on where the bus was when the routine got there. Answering such
questions by hand-simulating its clock accounting has never once converged, and
has twice put a fitted constant into this core wearing a citation. This answers
them by lookup.

The recording in `cpu-validation/test_data` cannot answer them either. It
carries the bus pins and the queue status lines and nothing else, and its queue
lines lag the event by one T-state: `q_op` is recorded as `last_queue_op`,
rolled over at the end of each cycle (`cycle.rs:367`, `mod.rs:1167`). An event
one clock early and a report one clock late are indistinguishable in it. That
ambiguity is what this tool removes.

Two things it settled immediately, both of which had been guessed wrong:

- **A flush's effect and its report are one clock apart.** `; FLUSH; BFS; AS_TR`
  and the microcode line share a clock, and the `Emptied` status appears on the
  clock after. Not two conventions to reconcile: one clock for the queue and the
  reload request, the next for the status line.
- **`FETCH_END` is a real clock and it is the last cycle of the instruction's
  span.** The boundary fetch preloads the byte it waited for and then spends a
  clock (`biu.rs:329`), and the recording ends on it. It is visible as the
  trailing `; FOQR; FETCH_END` line with a `*` in the queue column.

## Setting it up

The reference is a path dependency on an ephemeral checkout, so `Cargo.toml`
points at `/tmp/martypc` and that **will not survive a reboot**. To restore it:

```bash
git clone --depth 1 https://github.com/dbalsom/martypc.git /tmp/martypc
```

A sparse checkout of `cpu_808x` is enough to *read* the reference but not to
build it; this needs the whole crate.

This package is deliberately outside the phosphor workspace (`exclude` in the
root `Cargo.toml`) so that nothing shipped can depend on a foreign emulator core
and a missing checkout cannot break `cargo build`.

## Using it

```bash
cargo run --manifest-path tools/marty-probe/Cargo.toml -- C3
cargo run --manifest-path tools/marty-probe/Cargo.toml -- CB --queue 4 --case 3
cargo run --manifest-path tools/marty-probe/Cargo.toml -- FF.5 --vectors cpu-validation/test_data
```

It selects from the same population the surveys do: a full queue and no prefix.

### The other half of the corpus

`--prefixed` selects the cases that *begin* with a prefix instead of the ones
that do not. They are about half of the full-queue population and they were
invisible until this flag existed, which is why a family can read 100% here and
still be failing in the per-cycle gate.

**They are all one clock out at the prefix.** `88 --diff --prefixed` is the
clearest instance: the prefix's own First Byte read is on cycle 0 in both cores,
and on cycle 2 the reference reads the *opcode* while this core reads it on
cycle 3. Everything after is shifted with it. This is the session's root defect
one byte further along: the loader takes the byte after a prefix a T-state later
than the part does.

Three numbers that look inconsistent and are not, so that the next reader does
not spend the time working it out again:

- **`--prefixed` reports 0.00% cycle-identical.** A one-clock shift in a queue
  read breaks cycle-for-cycle equality on every case, whatever the span is.
- **The gate's queue-operation sequence is still 100.00%.** It compares the
  order of the operations, not the clocks they land on, and a uniform shift
  leaves the order alone.
- **The gate's cycle *count* is 87.09% on the prefetched half, not 50%.** The
  pause only costs a clock where the loader was not going to be waiting anyway,
  which is the short instructions from a full queue. See `timing::PREFIX_PAUSE`
  and the note about absorption in `begin_execute_phase`.

So the span is often right while every cycle inside it is wrong, and the
bus-cycle order gate is where that shows: 88.03% on the prefetched half against
94.06% on the empty one.

### Diffing against this core

```bash
cargo run --manifest-path tools/marty-probe/Cargo.toml -- CB --diff
```

Prints both cores' cycles side by side with the reference's microcode line
against each, and marks the divergences: `<<` the first, `<` the rest. A clean
result is one line of output and nothing marked.

**The reference is run live rather than read out of the corpus.** The corpus is
the same emulator's output, but its per-cycle format carries no queue length:
the deserializer hardcodes `q_len: 0` (`cpu_validator.rs:682`). A prefetch
decision turns on queue length, so a diff against the corpus can show that one
went differently but never why. The live run has the real length and the
microcode line besides. Every diff checks its own span against the recording's
and says so loudly if they differ, so a mis-set-up probe cannot pass quietly.

The queue length is shown but **not compared**: the two cores sample it at
different points in a cycle. Read it, do not trust an equality on it.

Three normalizations, so that real divergences stand out. The reference reports
its status pins as passive outside T1 and T2, so this core's status is masked
the same way. Its live state builder calls an idle bus `T1` where the recording
says `Ti` (`mod.rs:1132`), so a passive `T1` is shown as idle. And no shift is
applied to the queue column: each window is anchored on its own first-byte
report, which already takes out the recording's one-cycle reporting lag.
Shifting again makes every queue event look one clock out.

### The worklist

```bash
cargo run --release --manifest-path tools/marty-probe/Cargo.toml -- all --diff --sweep --limit 12
```

Sweeps the corpus and ranks **the microcode line the first divergence happened
on**, commonest first, with the files it happened in. A survey can say which
opcode files are wrong and by how much; it cannot say what the part was doing at
the moment the two parted company. Two files failing on the same line are one
fix, and this is what says so.

It also prints the headline this work is really measured by: how many cases are
identical to the reference cycle for cycle.

## What it found first

**This core takes its bytes out of the queue one T-state after the part does.**

`00 --diff --case 1` is the clearest instance. Everything matches to cycle 6. On
cycle 7, the part's queue is at 2 and this core's at 3, because the part read
its displacement byte on that cycle (`1DE: Q -> tmpbL ; FOQR`) and this core
reads it on cycle 8. The part's fetch decision at the end of that T2 therefore
sees room and runs `FETCH_NORMAL`, starting an address cycle that becomes a code
fetch on cycle 10; ours sees the queue one byte fuller, takes the policy-delay
branch instead, and runs no fetch at all. The operand read then lands a clock
late and the whole tail follows.

**The recording cannot show this and neither can the queue-operation gate.** The
recording reports a queue operation on the cycle after it happens, so a read
that is one clock late here and a report that is one clock late there cancel
exactly: the queue-status columns agree, which is why that gate reads 100.00%
while the bus-cycle sequence sits at 81.64%. Only the queue *length* at a
prefetch decision separates them, and only a live run has it.

## Reading the output

The trace begins at reset, two cycles before the recording's window. **The
window opens on the cycle showing `<-q <opcode>`**, the first-byte queue read,
so `recording index = trace index - 1` for a case that starts from a full
queue. Check it against the `<-q` line rather than assuming it.

Columns worth knowing, left to right: the bus T-state, then the *address*
cycle's `Tr`/`Ts`/`T0`/`Td`, the data transfer, the fetch state, the queue
(length, contents, and the byte read), the **microcode line and its source
text**, and the trace comments.

The comments are the events: `SUSP`, `FLUSH`, `BFS` (begin fetch start),
`AS_TR` (address start, at Tr), `DECIDE` (prefetch decision), `FETCH_NEXT`,
`FETCH_END`, `FOQR` (fetch on queue read).

Compare against this core with `side_by_side` in
`cpu-validation/tests/i8088_transfer_timing.rs`, and settle anything about *when*
a bus cycle starts with `fetch_gap_diff`, which compares bus events to bus
events and so admits no reporting convention at all.
