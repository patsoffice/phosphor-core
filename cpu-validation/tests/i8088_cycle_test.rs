//! Per-cycle replay of the SingleStepTests/8088 bus trace.
//!
//! This is the second gate on the I8088, beside the state-only one in
//! `i8088_single_step_test.rs`, and it exists because that one cannot see
//! timing at all. The suite records eleven fields per CPU cycle, taken off a
//! real AMD D8088 through an Arduino8088 interface, and the state gate throws
//! all of it away.
//!
//! **The comparison widens in steps.** See
//! `docs/designs/cycle-accurate-i8088.md`, Decision 3. Widening in order is
//! what keeps a failure legible: one all-or-nothing comparison against eleven
//! fields fails for one reason and gets read as failing for another.
//!
//! Three comparisons are live here, and only one of them is asserted.
//!
//! - **Queue operations in order**, ASSERTED exactly. Which bytes the EU took
//!   out of the prefetch queue, whether each was a First or a Subsequent byte,
//!   and where the queue was flushed. This is most of what the doc calls step 4
//!   of the ladder: what is missing from it is the *position* of each operation
//!   in the cycle stream, which cannot be checked until the cycle counts are
//!   right, and which the hardware reports one cycle late in any case.
//! - **Cycle count**, reported. Execution is still atomic, so the core charges
//!   nothing for effective-address calculation or for operand bus cycles, and
//!   the count is a floor rather than an answer.
//! - **Bus cycles in order**, reported. Every CODE, MEMR, MEMW, IOR and IOW
//!   transaction the core ran, as kind, address and byte, against the recording.
//!   This is the most diagnostic of the three, because a failure says *where*
//!   rather than *how much*: the commonest one is that the hardware slipped a
//!   prefetch in between an operand read and its write-back, in execution time
//!   this core does not yet spend, and the mismatch shows that as an ordering
//!   difference with every address and byte still correct.
//!
//! Interrupt-acknowledge cycles are M4; a trace containing one is left
//! uncompared rather than silently matched against nothing. I/O cycles are
//! compared, which takes reading the recording's second set of command lines:
//! see [`recorded_bus_cycles`].
//!
//! The vectors are a fixed, external, hardware-recorded standard. Nothing in
//! here may adjust them, and no tolerance may be widened to make a milestone
//! pass. What this file is allowed to do is report a number.
//!
//! ```text
//! PHOSPHOR_REQUIRE_VECTORS=1 cargo test --release -p phosphor-cpu-validation \
//!     --test i8088_cycle_test -- --nocapture
//! ```
//!
//! Run it under `--release`. The suite is 2.5 million vectors and a debug build
//! is a different program at a different speed.

use std::io::Read;

use rayon::prelude::*;

use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8088::{BusStatus as CoreBusStatus, I8088, QueueStatus};
use phosphor_cpu_validation::{
    BusStatus, I8088InitialState, I8088TestCase, QueueOp, TState, TracingBus20,
};

/// Opcodes the suite ships no file for at all, which is why this gate needs no
/// skip list of its own.
///
/// The state gate skips 44 files, but almost all of those are opcodes we do not
/// implement rather than data we cannot use, and an unimplemented opcode still
/// has a cycle count worth measuring once it exists. The genuinely unusable set
/// turns out to be empty, because the suite simply does not record these: a
/// prefix has no standalone execution to record, and HALT and WAIT were left
/// out deliberately. A skip list naming them would be a list that never fires.
/// [`the_unrecorded_opcodes_really_have_no_files`] is what keeps that claim
/// true rather than assumed.
const NOT_IN_THE_SUITE: &[&str] = &[
    // Segment override, LOCK, REPNE and REP: prefixes, per the suite's
    // metadata.json, which marks them "prefix" and gives them no tests.
    "26", "2E", "36", "3E", "F0", "F1", "F2", "F3",
    // "HALT is not included in this test set", and "WAIT is not included in
    // this test set", both from the suite README's per-instruction notes.
    "F4", "9B",
];

/// What one case's replay produced.
struct Verdict {
    /// Our T-state count, and the hardware's, when there was a trace.
    counts: Option<(usize, usize)>,
    /// The queue operations we performed against the ones recorded, compared
    /// as ordered sequences. `None` when there was no trace to compare.
    queue: Option<Result<(), String>>,
    /// The bus cycles this core ran against the ones recorded, compared as
    /// ordered sequences of kind, address and byte.
    fetches: Option<Result<(), String>>,
    /// Our core never reached an instruction boundary.
    hung: bool,
}

/// One completed bus cycle: what kind, where, and what was on the data pins.
///
/// The address is taken from T1, the only cycle it is on the multiplexed pins,
/// and the byte from T3. Pairing them is what makes a transaction comparable at
/// all: neither field alone identifies it. That pairing is exactly what the
/// external address latch on a real board does, which is why the recorded trace
/// leaves it to the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BusCycle {
    kind: Kind,
    address: u32,
    byte: u8,
}

/// The kinds of bus cycle this core drives. Interrupt acknowledge is the one
/// the suite records nowhere and this core cannot yet produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Code,
    MemRead,
    MemWrite,
    IoRead,
    IoWrite,
}

impl std::fmt::Display for BusCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let k = match self.kind {
            Kind::Code => "F",
            Kind::MemRead => "R",
            Kind::MemWrite => "W",
            Kind::IoRead => "I",
            Kind::IoWrite => "O",
        };
        write!(f, "{k}{:05X}:{:02X}", self.address, self.byte)
    }
}

/// The bus cycles the hardware recorded, in order.
///
/// A bus cycle spans four rows of the trace. The address is on the row whose
/// ALE pin is asserted; the data is on the row where the i8288 asserts a
/// command line, which is T3. Using the status line rather than the T-state
/// name means this still finds the byte if a wait state moves it.
fn recorded_bus_cycles(tc: &I8088TestCase) -> Vec<BusCycle> {
    let mut out = Vec::new();
    let mut pending: Option<(Kind, u32)> = None;
    for c in &tc.cycles {
        if let Some(addr) = c.address() {
            let kind = match c.status() {
                BusStatus::CODE => Some(Kind::Code),
                BusStatus::MEMR => Some(Kind::MemRead),
                BusStatus::MEMW => Some(Kind::MemWrite),
                BusStatus::IOR => Some(Kind::IoRead),
                BusStatus::IOW => Some(Kind::IoWrite),
                // INTA, HALT and PASV are not driven by this core. A trace
                // containing an interrupt acknowledge is left uncompared
                // rather than silently matched against nothing.
                _ => None,
            };
            pending = kind.map(|k| (k, addr));
        }
        // T3 is where the data is valid, and where the i8288 asserts either the
        // read line or one of the two write lines.
        //
        // **There are two sets of them and an I/O cycle asserts the second.**
        // Field 3 is MRDC/AMWC/MWTC and field 4 is IORC/AIOWC/IOWC, which
        // `CommandLines::io_lines_are_a_separate_field_from_memory_lines` in the
        // library states outright. Looking only at field 3 would mean no
        // recorded I/O cycle ever reached this comparison, and all eight of
        // `E4`-`E7` and `EC`-`EF` reading as this core inventing a bus cycle:
        // `in al, 1Bh` would come out as "2 bus cycles, hardware ran 1".
        let commanded = !c.3.is_idle() || !c.4.is_idle();
        if c.t_state() == TState::T3
            && commanded
            && let Some((kind, address)) = pending.take()
        {
            out.push(BusCycle {
                kind,
                address,
                byte: c.6,
            });
        }
    }
    out
}

fn compare_bus_cycles(ours: &[BusCycle], theirs: &[BusCycle]) -> Result<(), String> {
    let render = |f: &[BusCycle]| {
        f.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    if ours.len() != theirs.len() {
        return Err(format!(
            "{} bus cycles, hardware ran {}: got [{}] want [{}]",
            ours.len(),
            theirs.len(),
            render(ours),
            render(theirs),
        ));
    }
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        if !same_bus_cycle(a, b) {
            return Err(format!(
                "bus cycle {i} is {a}, hardware had {b}: got [{}] want [{}]",
                render(ours),
                render(theirs),
            ));
        }
    }
    Ok(())
}

/// The filler the recording rig prefetched, and the byte it reports for every
/// code fetch past an instruction's own bytes. See its use in
/// [`same_bus_cycle`].
const CODE_FETCH_FILLER: u8 = 0x90;

/// Whether two bus cycles are the same event.
///
/// Kind and address always have to match. **The byte on a code fetch does not,
/// where the recording reports the rig's filler.** The suite's README says "all
/// bytes fetched after the initial instruction bytes are set to 0x90", and that
/// is a property of the recording rather than of the address: the rig reports
/// `0x90` for such a fetch whatever memory actually holds there. Every case the
/// suite seeds is therefore a disagreement waiting to happen, and the two that
/// happen are a fetch that runs into the operand the case seeded, and a
/// backwards branch that lands back on the instruction's own bytes.
///
/// A recorded byte that is *not* the filler is real and is still compared, so a
/// fetch of an instruction's own bytes is held to the recording as before. This
/// is the data on a code fetch only: reads and writes are compared whole.
fn same_bus_cycle(ours: &BusCycle, theirs: &BusCycle) -> bool {
    if ours.kind != theirs.kind || ours.address != theirs.address {
        return false;
    }
    ours.byte == theirs.byte || (ours.kind == Kind::Code && theirs.byte == CODE_FETCH_FILLER)
}

/// One queue operation: what the EU did, and the byte it read.
///
/// A flush carries no byte. The recorded trace puts a zero in the byte column
/// for one, and comparing that against ours would be comparing a field the
/// hardware does not define, so the byte is dropped here for `Emptied`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueEvent {
    Read(QueueOp, u8),
    Flush,
}

impl std::fmt::Display for QueueEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueEvent::Read(QueueOp::First, b) => write!(f, "F:{b:02X}"),
            QueueEvent::Read(QueueOp::Subsequent, b) => write!(f, "S:{b:02X}"),
            QueueEvent::Read(op, b) => write!(f, "{op:?}:{b:02X}"),
            QueueEvent::Flush => write!(f, "E"),
        }
    }
}

fn render(events: &[QueueEvent]) -> String {
    events
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The queue operations the hardware recorded for this case, in order.
///
/// The trace reports each operation on the cycle *after* it happened, which
/// does not matter here because this compares order rather than position.
fn recorded_queue_events(tc: &I8088TestCase) -> Vec<QueueEvent> {
    tc.cycles
        .iter()
        .filter_map(|c| c.queue_op())
        .map(|(op, byte)| match op {
            QueueOp::Emptied => QueueEvent::Flush,
            op => QueueEvent::Read(op, byte),
        })
        .collect()
}

/// Compare two queue-operation sequences.
///
/// The suite defines a test's span as ending when the next instruction's First
/// Byte is read, and that read is the boundary rather than part of the trace:
/// the recorded array stops on the cycle before it. So the two sequences cover
/// the same span and compare exactly, with no allowance at either end. The
/// first draft of this function subtracted one for a trailing First Byte that
/// is not there, which failed every well-behaved case while printing two
/// identical sequences side by side.
fn compare_queue_events(ours: &[QueueEvent], theirs: &[QueueEvent]) -> Result<(), String> {
    if ours.len() != theirs.len() {
        return Err(format!(
            "{} queue operations, hardware had {}: got [{}] want [{}]",
            ours.len(),
            theirs.len(),
            render(ours),
            render(theirs),
        ));
    }
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        if a != b {
            return Err(format!(
                "queue operation {i} is {a}, hardware had {b}: got [{}] want [{}]",
                render(ours),
                render(theirs),
            ));
        }
    }
    Ok(())
}

fn load_initial_state(cpu: &mut I8088, bus: &mut TracingBus20, state: &I8088InitialState) {
    // The recording rig filled memory the test does not name with NOP: "All
    // bytes fetched after the initial instruction bytes are set to 0x90".
    // Leaving it zero here means the BIU prefetches 0x00 where the hardware
    // prefetched 0x90, so every fetch past the end of the instruction compares
    // unequal for a reason that is about this harness rather than about the
    // core. The state gate does not need this, because the suite names every
    // location an instruction actually touches.
    bus.memory.fill(0x90);

    cpu.ax = state.regs.ax;
    cpu.bx = state.regs.bx;
    cpu.cx = state.regs.cx;
    cpu.dx = state.regs.dx;
    cpu.cs = state.regs.cs;
    cpu.ss = state.regs.ss;
    cpu.ds = state.regs.ds;
    cpu.es = state.regs.es;
    cpu.sp = state.regs.sp;
    cpu.bp = state.regs.bp;
    cpu.si = state.regs.si;
    cpu.di = state.regs.di;
    cpu.ip = state.regs.ip;
    cpu.flags = state.regs.flags;

    for &(addr, val) in &state.ram {
        bus.memory[(addr & 0xF_FFFF) as usize] = val;
    }

    // Install the prefetch queue. Half the suite's cases run from a full one,
    // and until now this harness deserialized the array and dropped it.
    //
    // The README says to set the queue after reset has flushed it and then
    // "add the length of the queue contents to your PC register", because on
    // the part IP *is* the prefetch pointer. Here IP is the architectural one,
    // which is what the vectors' `ip` field reports, and the prefetch pointer
    // is separate: it starts ahead of IP by the number of bytes queued, which
    // is the same arithmetic seen from the other side.
    cpu.load_prefetch_queue(&state.queue);
}

/// Serve the ports the way the recording did. Without this an `IN` reads 0xFF
/// where the part read something else, and the comparison of its IOR cycle's
/// data byte fails for a reason that is about this harness.
fn load_port_reads(bus: &mut TracingBus20, tc: &I8088TestCase) {
    bus.port_reads = tc.port_reads();
}

/// Run one case and compare T-state counts.
///
/// The instruction is run to its boundary exactly as the state gate runs it,
/// and the number of `tick_with_bus` calls is our cycle count. That equivalence
/// is the whole content of the milestone: one tick has to *be* one T-state for
/// this comparison to mean anything, and where it is not, this reports the gap
/// rather than hiding it.
fn run_test_case(tc: &I8088TestCase) -> Verdict {
    let mut cpu = I8088::new();
    let mut bus = TracingBus20::new();

    load_initial_state(&mut cpu, &mut bus, &tc.initial);
    load_port_reads(&mut bus, tc);

    // The span being measured is the suite's, not this replay's.
    //
    // "Instruction cycles begin from the cycle in which the CPU's queue status
    // lines indicate that an instruction First Byte has been fetched", and end
    // when the next instruction's First Byte is read. That is not the same
    // span as "from when this harness started the CPU until the instruction
    // retired", and comparing the two was wrong at both ends:
    //
    // - At the start, a case beginning from an empty queue has to fetch its
    //   opcode before it can read it, and the hardware did that *before* its
    //   trace began. Counting those cycles charged this core four T-states the
    //   recording never showed, on exactly half the suite.
    // - At the end, the hardware kept prefetching until the next instruction's
    //   first byte came out of the queue, which is several cycles past where
    //   this replay stopped.
    //
    // So: run to the first queue read, start measuring there, and stop on the
    // next First Byte.
    let mut ours: Vec<QueueEvent> = Vec::new();
    let mut fetches: Vec<BusCycle> = Vec::new();
    let mut pending: Option<(Kind, u32)> = None;
    let mut ticks = 0usize;
    let mut measuring = false;
    let mut retired = false;
    let mut elapsed = 0usize;
    let hung = |_: ()| Verdict {
        counts: None,
        queue: None,
        fetches: None,
        hung: true,
    };

    loop {
        ticks += 1;
        // Generous: the longest recorded traces in the suite are the REP string
        // operations, and a word IDIV runs past 200 cycles on its own.
        if ticks > 4000 {
            return hung(());
        }
        // Retirement as of *before* this tick. A one-byte instruction retires
        // on the very cycle its opcode is read, and that read is a First Byte
        // belonging to the instruction being measured rather than to the next
        // one. Testing the flag after the tick closed the span an instruction
        // early on every single-byte opcode.
        let was_retired = retired;
        retired |= cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

        // A First Byte does not by itself mean a new instruction. A prefix
        // reads as one and so does the opcode behind it, which is the README's
        // "multiple First Byte statuses until the first byte that is a
        // non-prefixed opcode byte is read". So the span closes on the first
        // First Byte *after* this instruction has retired, which is the only
        // reading of "the next instruction" the CPU can actually supply.
        let next_instruction =
            matches!(cpu.queue_status, Some((QueueStatus::First, _))) && was_retired;

        if !measuring {
            // Still before the span. The opcode's own fetch happens here for a
            // case starting from an empty queue, and is not part of the trace.
            if cpu.queue_status.is_some() {
                measuring = true;
            } else {
                continue;
            }
        } else if next_instruction {
            // The span is over, and this cycle belongs to the next instruction
            // rather than to this one.
            break;
        } else {
            elapsed += 1;
        }

        // Sample the QS lines every cycle, exactly as the recording rig did.
        if let Some((status, byte)) = cpu.queue_status {
            ours.push(match status {
                QueueStatus::First => QueueEvent::Read(QueueOp::First, byte),
                QueueStatus::Subsequent => QueueEvent::Read(QueueOp::Subsequent, byte),
                QueueStatus::Emptied => QueueEvent::Flush,
            });
        }
        // And latch the address off T1 and the byte off T3, the same way the
        // external latch on the board does.
        let kind = match cpu.bus.status {
            CoreBusStatus::Code => Some(Kind::Code),
            CoreBusStatus::MemRead => Some(Kind::MemRead),
            CoreBusStatus::MemWrite => Some(Kind::MemWrite),
            CoreBusStatus::IoRead => Some(Kind::IoRead),
            CoreBusStatus::IoWrite => Some(Kind::IoWrite),
            _ => None,
        };
        if let Some(kind) = kind {
            if let Some(address) = cpu.bus.address {
                pending = Some((kind, address));
            }
            if let Some(byte) = cpu.bus.data
                && let Some((kind, address)) = pending.take()
            {
                fetches.push(BusCycle {
                    kind,
                    address,
                    byte,
                });
            }
        }
    }

    if tc.cycles.is_empty() {
        return Verdict {
            counts: None,
            queue: None,
            fetches: None,
            hung: false,
        };
    }

    // `elapsed` counts the cycles after the opening First Byte; the cycle
    // carrying it is the trace's first row, so add it back.
    Verdict {
        counts: Some((elapsed + 1, tc.cycles.len())),
        queue: Some(compare_queue_events(&ours, &recorded_queue_events(tc))),
        fetches: Some(compare_bus_cycles(&fetches, &recorded_bus_cycles(tc))),
        hung: false,
    }
}

/// Whether a case begins with a prefilled prefetch queue.
///
/// The suite runs half its instructions from a full queue, and says so through
/// a non-empty `initial.queue`. The two populations are structurally different
/// and must be reported apart: a prefetched case's opcode, ModR/M and
/// displacement bytes cost no bus cycles at all, and its trace opens with two
/// `Ti` because it takes two cycles to begin a fetch after reading from a full
/// queue. A core with no queue cannot reproduce either, so folding the two
/// populations into one number would hide which half of the gap is the queue's.
fn is_prefetched(tc: &I8088TestCase) -> bool {
    !tc.initial.queue.is_empty()
}

/// One opcode file's totals.
#[derive(Default)]
struct FileOutcome {
    filename: String,
    /// Cases whose count matched, by population.
    /// Cases whose cycle count matched, restricted to instructions whose
    /// execution time this core models at all.
    matched_modeled: usize,
    total_modeled: usize,
    matched_empty: usize,
    matched_prefetched: usize,
    /// Cases compared, by population.
    total_empty: usize,
    total_prefetched: usize,
    no_trace: usize,
    hung: usize,
    /// Summed signed error, for the average over/undercount.
    error_sum: i64,
    /// Cases whose queue-operation sequence matched the recording.
    queue_matched: usize,
    queue_total: usize,
    /// Cases whose bus-cycle sequence matched the recording, split by
    /// population the way the cycle count is.
    ///
    /// Reported apart because the aggregate cannot distinguish "the operand
    /// path interleaves wrongly" from "the loader schedules its fetches
    /// wrongly", and those are different pieces of work. A single percentage
    /// over both populations is what would let this sit at a third for several
    /// milestones with nobody able to say what it was made of.
    fetch_matched: usize,
    fetch_total: usize,
    fetch_matched_empty: usize,
    fetch_total_empty: usize,
    fetch_matched_prefetched: usize,
    fetch_total_prefetched: usize,
    /// The first differing case of each kind, kept for the report.
    first_difference: Option<String>,
    first_queue_difference: Option<String>,
    first_fetch_difference: Option<String>,
}

#[test]
fn i8088_cycle_counts_against_the_hardware_trace() {
    let test_dir = phosphor_cpu_validation::vector_dir("8088/v2");
    let test_dir = test_dir.as_path();
    if !phosphor_cpu_validation::require_test_data(
        test_dir,
        "run: git submodule update --init cpu-validation/test_data/8088",
    ) {
        return;
    }

    let mut entries: Vec<_> = std::fs::read_dir(test_dir)
        .expect("Failed to read test directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "gz"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let outcomes: Vec<FileOutcome> = entries
        .par_iter()
        .map(|entry| {
            let filename = entry.file_name().to_string_lossy().into_owned();

            let gz_data = std::fs::read(entry.path())
                .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", entry.path(), e));
            let mut decoder = flate2::read::GzDecoder::new(&gz_data[..]);
            let mut json = String::new();
            decoder
                .read_to_string(&mut json)
                .unwrap_or_else(|e| panic!("Failed to decompress {:?}: {}", entry.path(), e));
            let tests: Vec<I8088TestCase> = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", entry.path(), e));

            assert!(!tests.is_empty(), "Test file {filename} is empty");

            let mut out = FileOutcome {
                filename,
                ..Default::default()
            };

            for tc in &tests {
                let prefetched = is_prefetched(tc);
                let verdict = run_test_case(tc);

                if verdict.hung {
                    out.hung += 1;
                    continue;
                }

                let Some((ours, theirs)) = verdict.counts else {
                    out.no_trace += 1;
                    continue;
                };

                if prefetched {
                    out.total_prefetched += 1;
                } else {
                    out.total_empty += 1;
                }
                // Split out the instructions whose microcode time this core
                // models. The others are short by all of it, so averaging them
                // together describes neither population.
                let modeled = I8088::models_execution_time(&tc.bytes);
                if modeled {
                    out.total_modeled += 1;
                }

                if ours == theirs {
                    if modeled {
                        out.matched_modeled += 1;
                    }
                    if prefetched {
                        out.matched_prefetched += 1;
                    } else {
                        out.matched_empty += 1;
                    }
                } else {
                    out.error_sum += ours as i64 - theirs as i64;
                    if out.first_difference.is_none() {
                        let queue = if prefetched { "prefetched" } else { "empty" };
                        out.first_difference = Some(format!(
                            "{}: {ours} cycles, hardware took {theirs} ({queue} queue)",
                            tc.name
                        ));
                    }
                }

                if let Some(queue) = verdict.queue {
                    out.queue_total += 1;
                    match queue {
                        Ok(()) => out.queue_matched += 1,
                        Err(why) => {
                            if out.first_queue_difference.is_none() {
                                out.first_queue_difference = Some(format!("{}: {why}", tc.name));
                            }
                        }
                    }
                }

                if let Some(f) = verdict.fetches {
                    out.fetch_total += 1;
                    if prefetched {
                        out.fetch_total_prefetched += 1;
                    } else {
                        out.fetch_total_empty += 1;
                    }
                    match f {
                        Ok(()) => {
                            out.fetch_matched += 1;
                            if prefetched {
                                out.fetch_matched_prefetched += 1;
                            } else {
                                out.fetch_matched_empty += 1;
                            }
                        }
                        Err(why) => {
                            if out.first_fetch_difference.is_none() {
                                let queue = if prefetched { "prefetched" } else { "empty" };
                                out.first_fetch_difference =
                                    Some(format!("{}: {why} ({queue} queue)", tc.name));
                            }
                        }
                    }
                }
            }

            out
        })
        .collect();

    let mut files = 0usize;
    let mut matched_modeled = 0usize;
    let mut total_modeled = 0usize;
    let mut matched_empty = 0usize;
    let mut matched_prefetched = 0usize;
    let mut total_empty = 0usize;
    let mut total_prefetched = 0usize;
    let mut no_trace = 0usize;
    let mut hung = 0usize;
    let mut error_sum = 0i64;
    let mut queue_matched = 0usize;
    let mut queue_total = 0usize;
    let mut fetch_matched = 0usize;
    let mut fetch_total = 0usize;
    let mut fetch_matched_empty = 0usize;
    let mut fetch_total_empty = 0usize;
    let mut fetch_matched_prefetched = 0usize;
    let mut fetch_total_prefetched = 0usize;
    let mut examples: Vec<String> = Vec::new();
    let mut queue_examples: Vec<String> = Vec::new();
    let mut fetch_examples: Vec<String> = Vec::new();

    for o in &outcomes {
        files += 1;
        matched_modeled += o.matched_modeled;
        total_modeled += o.total_modeled;
        matched_empty += o.matched_empty;
        matched_prefetched += o.matched_prefetched;
        total_empty += o.total_empty;
        total_prefetched += o.total_prefetched;
        no_trace += o.no_trace;
        hung += o.hung;
        error_sum += o.error_sum;
        queue_matched += o.queue_matched;
        queue_total += o.queue_total;
        fetch_matched += o.fetch_matched;
        fetch_total += o.fetch_total;
        fetch_matched_empty += o.fetch_matched_empty;
        fetch_total_empty += o.fetch_total_empty;
        fetch_matched_prefetched += o.fetch_matched_prefetched;
        fetch_total_prefetched += o.fetch_total_prefetched;
        if let Some(d) = &o.first_fetch_difference
            && fetch_examples.len() < 20
        {
            fetch_examples.push(format!("{}  {}", o.filename, d));
        }
        if let Some(d) = &o.first_difference
            && examples.len() < 20
        {
            examples.push(format!("{}  {}", o.filename, d));
        }
        if let Some(d) = &o.first_queue_difference
            && queue_examples.len() < 20
        {
            queue_examples.push(format!("{}  {}", o.filename, d));
        }
    }

    let compared = total_empty + total_prefetched;
    let matched = matched_empty + matched_prefetched;
    let pct = |n: usize, d: usize| {
        if d == 0 {
            0.0
        } else {
            n as f64 * 100.0 / d as f64
        }
    };

    eprintln!("\nI8088 per-cycle gate: cycle count, and queue operations in order");
    eprintln!("  {files} opcode files compared, none skipped");
    eprintln!(
        "  {queue_matched} of {queue_total} vectors match on the queue-operation \
         sequence ({:.2}%)",
        pct(queue_matched, queue_total)
    );
    eprintln!(
        "  {fetch_matched} of {fetch_total} vectors match on the bus-cycle \
         sequence ({:.2}%)",
        pct(fetch_matched, fetch_total)
    );
    eprintln!(
        "    empty queue:  {fetch_matched_empty} of {fetch_total_empty} ({:.2}%)",
        pct(fetch_matched_empty, fetch_total_empty)
    );
    eprintln!(
        "    prefetched:   {fetch_matched_prefetched} of {fetch_total_prefetched} ({:.2}%)",
        pct(fetch_matched_prefetched, fetch_total_prefetched)
    );
    eprintln!(
        "  {matched} of {compared} vectors match on cycle count ({:.2}%)",
        pct(matched, compared)
    );
    eprintln!(
        "    empty queue:  {matched_empty} of {total_empty} ({:.2}%)",
        pct(matched_empty, total_empty)
    );
    eprintln!(
        "    prefetched:   {matched_prefetched} of {total_prefetched} ({:.2}%)",
        pct(matched_prefetched, total_prefetched)
    );
    eprintln!(
        "    of the {total_modeled} whose execution time is modeled at all: \
         {matched_modeled} ({:.2}%)",
        pct(matched_modeled, total_modeled)
    );
    if compared > matched {
        eprintln!(
            "  mean signed error over the {} that differed: {:+.2} cycles",
            compared - matched,
            error_sum as f64 / (compared - matched) as f64
        );
    }
    if no_trace > 0 {
        eprintln!("  {no_trace} vectors carried no cycle trace and were not compared");
    }
    if hung > 0 {
        eprintln!("  {hung} vectors never reached an instruction boundary");
    }
    if !fetch_examples.is_empty() {
        eprintln!(
            "\nFirst bus-cycle difference per file (first {}):",
            fetch_examples.len()
        );
        for e in &fetch_examples {
            eprintln!("  {e}");
        }
    }
    // **Which files the bus-cycle residual is actually in.** The example list
    // above says what went wrong in each file and nothing about how much, so a
    // file with four failures and a file with four thousand read the same.
    // Ranking them is what says which one to open, and the difference is large:
    // the residual is not spread over the corpus but piled in a handful of
    // files.
    let mut fetch_worst: Vec<(usize, &str)> = outcomes
        .iter()
        .map(|o| (o.fetch_total - o.fetch_matched, o.filename.as_str()))
        .filter(|(failed, _)| *failed > 0)
        .collect();
    fetch_worst.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    if !fetch_worst.is_empty() {
        let shown = fetch_worst.len().min(20);
        eprintln!(
            "\nBus-cycle failures by file ({} of {} files, worst {shown}):",
            fetch_worst.len(),
            files,
        );
        for (failed, name) in fetch_worst.iter().take(shown) {
            eprintln!("  {failed:>7}  {name}");
        }
    }
    if !queue_examples.is_empty() {
        eprintln!(
            "\nFirst queue-sequence difference per file (first {}):",
            queue_examples.len()
        );
        for e in &queue_examples {
            eprintln!("  {e}");
        }
    }
    if !examples.is_empty() {
        eprintln!(
            "\nFirst count difference per file (first {}):",
            examples.len()
        );
        for e in &examples {
            eprintln!("  {e}");
        }
    }

    // And the same question asked the useful way round. The list above is
    // ordered by filename, so it says what 0x00 through 0x14 are doing and
    // nothing about the rest; this one says which opcodes are furthest from the
    // hardware, which is where the next row of the timing table comes from.
    let mut worst: Vec<&FileOutcome> = outcomes
        .iter()
        .filter(|o| o.total_empty + o.total_prefetched > 0)
        .collect();
    worst.sort_by(|a, b| {
        let rate = |o: &FileOutcome| {
            (o.matched_empty + o.matched_prefetched) as f64
                / (o.total_empty + o.total_prefetched) as f64
        };
        rate(a)
            .partial_cmp(&rate(b))
            .unwrap()
            .then_with(|| a.filename.cmp(&b.filename))
    });
    eprintln!("\nFurthest from the hardware on cycle count (worst 25 files):");
    for o in worst.iter().take(25) {
        let total = o.total_empty + o.total_prefetched;
        let matched = o.matched_empty + o.matched_prefetched;
        eprintln!(
            "  {}  {matched} of {total} ({:.1}%)  {}",
            o.filename,
            pct(matched, total),
            o.first_difference.as_deref().unwrap_or(""),
        );
        // With, where there is one, the bus-cycle difference for the same file,
        // which says *where* the missing time is rather than how much of it
        // there is.
        if let Some(d) = &o.first_fetch_difference {
            eprintln!("      {d}");
        }
    }

    // What this test asserts, and deliberately does not.
    //
    // It does NOT assert that the CYCLE COUNTS match. They do not yet, and
    // asserting a pass would mean a failing suite for months.
    //
    // It DOES hold them to a RATCHET, which is a different thing from the
    // fitted threshold this comment used to argue against. That argument was
    // right about a threshold set at today's figure to claim the work is done,
    // and wrong about one set there to stop the figure falling: the first
    // cannot fail, the second cannot fail *today*, which is the point. Six
    // milestones passed with the count barely moving and nothing could say so.
    //
    // [`RATCHET`] is two-sided for the same reason. A floor alone rots: it
    // drifts below the truth as the core improves and quietly stops protecting
    // anything, which is how the old figure survived so long. Overshooting it
    // by more than [`RATCHET_SLACK`] fails too, and says what to write instead,
    // so a gain has to be banked in the same change that earns it. The slack is
    // wide enough that an experiment moving hundredths does not trip it and
    // narrow enough that a real win cannot be left unrecorded.
    //
    // It DOES assert that the QUEUE OPERATIONS match, exactly, on every vector.
    // That is not a fitted threshold: it is equality against a hardware
    // recording, with no tolerance anywhere, and it went from 0 to 3,007,000
    // over the course of one milestone. The four bugs found on the way there
    // were each a real defect, and this is what keeps them fixed.
    //
    // And it asserts the three ways the file could silently become decorative:
    // that vectors were found, that they carry traces, and that the core
    // reaches an instruction boundary on every one of them.
    assert!(compared > 0, "no vectors were compared");
    assert_eq!(
        no_trace, 0,
        "{no_trace} vectors carried no cycle trace: the oracle is not being read"
    );
    assert_eq!(
        hung, 0,
        "{hung} vectors never reached an instruction boundary"
    );
    assert_eq!(
        queue_matched,
        queue_total,
        "{} vectors disagree with the hardware about what the EU took out of \
         the prefetch queue, in what order. See the examples above.",
        queue_total - queue_matched,
    );

    let mut broken: Vec<String> = Vec::new();
    for &(name, floor, matched, total) in &[
        ("cycle count", RATCHET.count, matched, compared),
        ("cycle count, empty queue", RATCHET.count_empty, matched_empty, total_empty),
        (
            "cycle count, prefetched",
            RATCHET.count_prefetched,
            matched_prefetched,
            total_prefetched,
        ),
        ("bus-cycle order", RATCHET.bus, fetch_matched, fetch_total),
        (
            "bus-cycle order, empty queue",
            RATCHET.bus_empty,
            fetch_matched_empty,
            fetch_total_empty,
        ),
        (
            "bus-cycle order, prefetched",
            RATCHET.bus_prefetched,
            fetch_matched_prefetched,
            fetch_total_prefetched,
        ),
    ] {
        let now = pct(matched, total);
        if now < floor {
            broken.push(format!(
                "  {name} FELL to {now:.2}%, below the recorded {floor:.2}%"
            ));
        } else if now > floor + RATCHET_SLACK {
            broken.push(format!(
                "  {name} ROSE to {now:.2}%, more than {RATCHET_SLACK:.2} above the \
                 recorded {floor:.2}%: raise it to {:.2}",
                (now * 100.0).floor() / 100.0
            ));
        }
    }
    assert!(
        broken.is_empty(),
        "the per-cycle ratchet moved:\n{}\n\nSee RATCHET in this file.",
        broken.join("\n")
    );
}

/// The percentages this gate is held to, as last recorded.
///
/// Each is the measured figure rounded *down* to two places, so that the
/// comparison cannot fail on the last bit of a float. Raise them in the same
/// change that earns the gain: the gate fails either way round, and says which.
struct Ratchet {
    count: f64,
    count_empty: f64,
    count_prefetched: f64,
    bus: f64,
    bus_empty: f64,
    bus_prefetched: f64,
}

/// Recorded 2026-09-05, over 3,007,000 vectors in 323 files, when the loader
/// learned where the part stops for a T-state. See `timing::loader_stall` in
/// the core.
const RATCHET: Ratchet = Ratchet {
    count: 73.76,
    count_empty: 54.83,
    count_prefetched: 92.68,
    bus: 38.78,
    // **This one is a floor under a number that is not yet meaningful.** The
    // loader still takes an instruction's first byte a T-state after the part
    // does, so every empty-queue case is one bus cycle out at the front and
    // this half cannot pass whatever else is right. It is here so that fixing
    // the loader is visible as a jump rather than as a number nobody was
    // watching.
    bus_empty: 0.03,
    bus_prefetched: 77.54,
};

/// How far above [`RATCHET`] a figure may sit before the gate insists it be
/// written down. One point is about 30,000 vectors on the whole corpus.
const RATCHET_SLACK: f64 = 1.00;

/// This gate has no skip list, and that is a claim about the data rather than a
/// convenience.
///
/// [`NOT_IN_THE_SUITE`] names the opcodes the suite says it does not record. If
/// a future version of the test set started shipping one of them, the main gate
/// above would silently begin comparing it: prefixes have no standalone
/// execution here and HLT blocks forever in the harness, so it would report a
/// wall of failures that are about the harness rather than about the core. This
/// is what would say so first, and name the file.
#[test]
fn the_unrecorded_opcodes_really_have_no_files() {
    let test_dir = phosphor_cpu_validation::vector_dir("8088/v2");
    if !phosphor_cpu_validation::require_test_data(
        &test_dir,
        "run: git submodule update --init cpu-validation/test_data/8088",
    ) {
        return;
    }

    let present: Vec<&str> = NOT_IN_THE_SUITE
        .iter()
        .copied()
        .filter(|stem| test_dir.join(format!("{stem}.json.gz")).exists())
        .collect();

    assert!(
        present.is_empty(),
        "the suite now ships vectors for {present:?}, which this gate assumed it never would. \
         Either the test set changed or NOT_IN_THE_SUITE is wrong; do not add a skip list \
         without working out which."
    );
}

/// The suite's own claim about its two populations, checked rather than assumed.
///
/// The README says half the instructions execute from a full prefetch queue. If
/// that stopped being true, or if `initial.queue` stopped deserializing, the
/// split reported above would quietly collapse into one population and the
/// prefetched line would read 0 of 0 without anything failing.
#[test]
fn the_suite_really_does_run_half_its_cases_from_a_full_queue() {
    let test_dir = phosphor_cpu_validation::vector_dir("8088/v2");
    let test_dir = test_dir.as_path();
    if !phosphor_cpu_validation::require_test_data(
        test_dir,
        "run: git submodule update --init cpu-validation/test_data/8088",
    ) {
        return;
    }

    // One file is enough to establish the shape, and 0x90 (NOP) is the simplest
    // instruction in the set.
    let path = test_dir.join("90.json.gz");
    let gz = std::fs::read(&path).expect("90.json.gz is present");
    let mut json = String::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_string(&mut json)
        .expect("decompresses");
    let tests: Vec<I8088TestCase> = serde_json::from_str(&json).expect("parses");

    let prefetched = tests.iter().filter(|t| is_prefetched(t)).count();
    let fraction = prefetched as f64 / tests.len() as f64;
    assert!(
        (0.3..0.7).contains(&fraction),
        "expected roughly half the cases to start from a full queue, got {prefetched} of {} ({fraction:.2})",
        tests.len()
    );

    // And every trace opens with a First Byte, which is what defines where a
    // test begins. Without this the cycle counts being compared above would not
    // be counting the same span the hardware was.
    for tc in tests.iter().take(100) {
        let first = tc
            .cycles
            .iter()
            .find_map(|c| c.queue_op())
            .map(|(op, _)| op);
        assert_eq!(
            first,
            Some(QueueOp::First),
            "{}: the first queue operation in a trace must be a First Byte",
            tc.name
        );
    }
}
