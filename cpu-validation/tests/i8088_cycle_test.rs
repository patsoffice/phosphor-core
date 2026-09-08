//! Per-cycle replay of the SingleStepTests/8088 bus trace.
//!
//! This is the second gate on the I8088, beside the state-only one in
//! `i8088_single_step_test.rs`, and it exists because that one cannot see
//! timing at all. The suite records eleven fields per CPU cycle, taken off a
//! real AMD D8088 through an Arduino8088 interface, and the state gate throws
//! all of it away.
//!
//! **The comparison widens in four steps, and this file is at step 1.** See
//! `docs/designs/cycle-accurate-i8088.md`, Decision 3. Step 1 is cycle *count*
//! only: how many T-states our core took against how many the hardware took.
//! Steps 2 to 4 add bus status and T-state, then address and data on the cycles
//! where the trace says they are valid, then queue operation status and
//! contents. Widening in that order is what keeps a failure legible: one
//! all-or-nothing comparison against eleven fields fails for one reason and
//! gets read as failing for another.
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
use phosphor_core::cpu::i8088::I8088;
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

/// How a case's cycle count compared, and why it could not be compared when it
/// could not.
enum Verdict {
    /// Our T-state count equals the hardware's.
    Match,
    /// Both counts are known and differ, by `ours - theirs`.
    Differed { ours: usize, theirs: usize },
    /// The vector carries no `cycles` array, so there is nothing to compare.
    /// Counted separately rather than as a pass, because a comparison that did
    /// not happen is not a comparison that succeeded.
    NoTrace,
    /// Our core never reached an instruction boundary.
    Hung,
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
    loop {
        ticks += 1;
        if cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)) {
            break;
        }
        // Generous: the longest recorded traces in the suite are the REP string
        // operations, and a word IDIV runs past 200 cycles on its own.
        if ticks > 2000 {
            return Verdict::Hung;
        }
    }

    if tc.cycles.is_empty() {
        return Verdict::NoTrace;
    }

    if ticks == tc.cycles.len() {
        Verdict::Match
    } else {
        Verdict::Differed {
            ours: ticks,
            theirs: tc.cycles.len(),
        }
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
    /// The first differing case, kept for the report.
    first_difference: Option<String>,
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
                match run_test_case(tc) {
                    Verdict::Match => {
                        if prefetched {
                            out.total_prefetched += 1;
                            out.matched_prefetched += 1;
                        } else {
                            out.total_empty += 1;
                            out.matched_empty += 1;
                        }
                    }
                    Verdict::Differed { ours, theirs } => {
                        if prefetched {
                            out.total_prefetched += 1;
                        } else {
                            out.total_empty += 1;
                        }
                        out.error_sum += ours as i64 - theirs as i64;
                        if out.first_difference.is_none() {
                            let queue = if prefetched { "prefetched" } else { "empty" };
                            out.first_difference = Some(format!(
                                "{}: {ours} cycles, hardware took {theirs} ({queue} queue)",
                                tc.name
                            ));
                        }
                    }
                    Verdict::NoTrace => out.no_trace += 1,
                    Verdict::Hung => out.hung += 1,
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
    let mut examples: Vec<String> = Vec::new();

    for o in &outcomes {
        files += 1;
        matched_empty += o.matched_empty;
        matched_prefetched += o.matched_prefetched;
        total_empty += o.total_empty;
        total_prefetched += o.total_prefetched;
        no_trace += o.no_trace;
        hung += o.hung;
        error_sum += o.error_sum;
        if let Some(d) = &o.first_difference
            && examples.len() < 20
        {
            examples.push(format!("{}  {}", o.filename, d));
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

    eprintln!("\nI8088 per-cycle gate, step 1 of 4: cycle count only");
    eprintln!("  {files} opcode files compared, none skipped");
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
    if !examples.is_empty() {
        eprintln!("\nFirst difference per file (first {}):", examples.len());
        for e in &examples {
            eprintln!("  {e}");
        }
    }

    // What this test asserts, and deliberately does not.
    //
    // It does NOT assert that the counts match. At M1 they overwhelmingly do
    // not, by construction: the core has no prefetch queue and charges no
    // EU-internal cycles, so the number above is a floor that M2 and M3 raise.
    // Asserting a pass here would mean either a failing suite for weeks or a
    // threshold tuned to whatever today's number happens to be, and a threshold
    // fitted to the current result is a check that cannot fail.
    //
    // What it does assert is that the gate is wired up and could report a
    // failure: that vectors were found, that they carry traces, and that the
    // core reaches an instruction boundary on every one of them. Those are the
    // three ways this file could silently become decorative.
    assert!(compared > 0, "no vectors were compared");
    assert_eq!(
        no_trace, 0,
        "{no_trace} vectors carried no cycle trace: the oracle is not being read"
    );
    assert_eq!(
        hung, 0,
        "{hung} vectors never reached an instruction boundary"
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
