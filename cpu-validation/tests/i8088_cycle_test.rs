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
//! Two comparisons are live here:
//!
//! - **Cycle count**, reported and not asserted. Execution is still atomic, so
//!   the core charges nothing for effective-address calculation or for operand
//!   bus cycles, and the count is a floor rather than an answer.
//! - **Queue operations in order**, asserted exactly. Which bytes the EU took
//!   out of the prefetch queue, whether each was a First or a Subsequent byte,
//!   and where the queue was flushed. This is most of what the doc calls step 4
//!   of the ladder: what is missing from it is the *position* of each operation
//!   in the cycle stream, which cannot be checked until the cycle counts are
//!   right, and which the hardware reports one cycle late in any case.
//!
//! Bus status, T-state, address and data are still unread.
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
use phosphor_core::cpu::i8088::{I8088, QueueStatus};
use phosphor_cpu_validation::{I8088InitialState, I8088TestCase, QueueOp, TracingBus20};

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
    /// Our core never reached an instruction boundary.
    hung: bool,
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

    let mut ticks = 0usize;
    let mut ours: Vec<QueueEvent> = Vec::new();
    loop {
        ticks += 1;
        let retired = cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
        // Sample the QS lines every cycle, exactly as the recording rig did.
        if let Some((status, byte)) = cpu.queue_status {
            ours.push(match status {
                QueueStatus::First => QueueEvent::Read(QueueOp::First, byte),
                QueueStatus::Subsequent => QueueEvent::Read(QueueOp::Subsequent, byte),
                QueueStatus::Emptied => QueueEvent::Flush,
            });
        }
        if retired {
            break;
        }
        // Generous: the longest recorded traces in the suite are the REP string
        // operations, and a word IDIV runs past 200 cycles on its own.
        if ticks > 2000 {
            return Verdict {
                counts: None,
                queue: None,
                hung: true,
            };
        }
    }

    if tc.cycles.is_empty() {
        return Verdict {
            counts: None,
            queue: None,
            hung: false,
        };
    }

    Verdict {
        counts: Some((ticks, tc.cycles.len())),
        queue: Some(compare_queue_events(&ours, &recorded_queue_events(tc))),
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
    /// The first differing case of each kind, kept for the report.
    first_difference: Option<String>,
    first_queue_difference: Option<String>,
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

                if ours == theirs {
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
            }

            out
        })
        .collect();

    let mut files = 0usize;
    let mut matched_empty = 0usize;
    let mut matched_prefetched = 0usize;
    let mut total_empty = 0usize;
    let mut total_prefetched = 0usize;
    let mut no_trace = 0usize;
    let mut hung = 0usize;
    let mut error_sum = 0i64;
    let mut queue_matched = 0usize;
    let mut queue_total = 0usize;
    let mut examples: Vec<String> = Vec::new();
    let mut queue_examples: Vec<String> = Vec::new();

    for o in &outcomes {
        files += 1;
        matched_empty += o.matched_empty;
        matched_prefetched += o.matched_prefetched;
        total_empty += o.total_empty;
        total_prefetched += o.total_prefetched;
        no_trace += o.no_trace;
        hung += o.hung;
        error_sum += o.error_sum;
        queue_matched += o.queue_matched;
        queue_total += o.queue_total;
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

    // What this test asserts, and deliberately does not.
    //
    // It does NOT assert that the CYCLE COUNTS match. They overwhelmingly do
    // not, by construction: execution is still atomic, so the core charges
    // nothing for the cycles the EU spends computing an effective address or
    // for the bus cycles an operand access takes. The number above is a floor
    // that M3 raises. Asserting a pass would mean either a failing suite for
    // weeks or a threshold tuned to whatever today's figure happens to be, and
    // a threshold fitted to the current result is a check that cannot fail.
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
}

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
