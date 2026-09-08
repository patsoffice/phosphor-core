//! Run one case of the 8088 test corpus through the reference emulator and
//! print what it did, cycle by cycle, with the microcode line that spent each
//! clock.
//!
//! **This exists because reading the reference is not the same as knowing when
//! it does things.** Its CPU is straight-line imperative code that calls
//! `cycle()` as it goes, so a question like "which T-state does 0x0c7 spend?"
//! has no answer you can read off a line: it depends on where the bus happened
//! to be when the routine got there. Every wrong turn in this core's timing
//! work has come from answering such a question by hand-simulating the
//! reference's clock accounting, which has never once converged. The trace this
//! prints answers them by lookup instead.
//!
//! The recording in `cpu-validation/test_data` cannot answer them either. It
//! carries the bus pins and the queue status lines and nothing else, and its
//! queue lines lag the event by one T-state (`q_op` is recorded as
//! `last_queue_op`, rolled over at the end of each cycle), which is exactly the
//! ambiguity that costs a day: an event one clock early and a report one clock
//! late look identical in it.
//!
//! Usage:
//!
//! ```text
//! cargo run --manifest-path tools/marty-probe/Cargo.toml -- C3
//! cargo run --manifest-path tools/marty-probe/Cargo.toml -- CB --queue 4 --case 3
//! ```
//!
//! The reference checkout is a path dependency and will not survive a reboot;
//! `README.md` beside this file has the clone command.

use std::io::Read;

use clap::Parser;
use marty_core::{
    cpu_common::{builder::CpuBuilder, Cpu, CpuAddress, CpuOption, CpuType, Register16, TraceMode},
    cpu_validator::{CycleState, VRegisters, VRegistersDelta},
};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
struct TestStateInitial {
    regs: VRegisters,
    ram: Vec<[u32; 2]>,
    queue: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct TestStateFinal {
    #[allow(dead_code)]
    regs: VRegistersDelta,
    #[allow(dead_code)]
    ram: Vec<[u32; 2]>,
    #[allow(dead_code)]
    queue: Vec<u8>,
}

#[derive(Deserialize)]
struct CpuTest {
    name: String,
    bytes: Vec<u8>,
    #[serde(rename = "initial")]
    initial_state: TestStateInitial,
    #[serde(rename = "final")]
    #[allow(dead_code)]
    final_state: TestStateFinal,
    cycles: Vec<CycleState>,
}

#[derive(Parser)]
#[command(about = "Trace one 8088 test case through the reference emulator, with microcode lines")]
struct Args {
    /// Opcode file stem, as the corpus names it: `C3`, `81.0`, `FF.5`.
    stem: String,
    /// Queue length the case must start with. The surveys use 4.
    #[arg(long, default_value_t = 4)]
    queue: usize,
    /// Which matching case to trace, in file order.
    #[arg(long, default_value_t = 0)]
    case: usize,
    /// Corpus root holding `8088/v2`.
    #[arg(long, default_value = "cpu-validation/test_data")]
    vectors: String,
    /// Instead of a trace, report the first cycle on which this core and the
    /// reference disagree, and the microcode line the reference was on there.
    #[arg(long)]
    diff: bool,
    /// With `--diff`, sweep every case in the file rather than one, and rank the
    /// microcode lines by how often they carry the first divergence. Pass
    /// `--stem all` to sweep the whole corpus.
    #[arg(long)]
    sweep: bool,
    /// How many cases `--sweep` looks at per file.
    #[arg(long, default_value_t = 200)]
    limit: usize,
    /// Select cases that *begin with a prefix* instead of cases that do not.
    ///
    /// The default population is the surveys': a full queue and no prefix, so
    /// that the loader is not the variable under test. That leaves a third of
    /// the corpus permanently out of view, and the per-cycle gate counts it, so
    /// a family can be at 100% here and still be failing there. This is how to
    /// look at it.
    #[arg(long)]
    prefixed: bool,
}

/// One cycle of either core, reduced to what both can report and the gate
/// actually compares.
///
/// `qlen` is shown but deliberately **not** compared. The two cores sample the
/// queue at different points within a cycle, so a raw comparison would call
/// every cycle a divergence; it is here because a prefetch decision turns on it,
/// and seeing it beside the decision is what explains one.
#[derive(Eq, Clone, Debug)]
struct Beat {
    bus: &'static str,
    t: &'static str,
    addr: Option<u32>,
    q: &'static str,
    qlen: u32,
}

impl PartialEq for Beat {
    fn eq(&self, other: &Self) -> bool {
        self.bus == other.bus && self.t == other.t && self.addr == other.addr && self.q == other.q
    }
}

fn main() {
    let args = Args::parse();

    if args.sweep {
        sweep(&args);
        return;
    }

    let path = format!("{}/8088/v2/{}.json.gz", args.vectors, args.stem);
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let mut json = String::new();
    flate2::read::GzDecoder::new(file)
        .read_to_string(&mut json)
        .unwrap_or_else(|e| panic!("decompress {path}: {e}"));
    let tests: Vec<CpuTest> = serde_json::from_str(&json).expect("parse corpus file");

    // The same population every survey restricts itself to: a full queue and no
    // prefix, so that the loader is not the variable under test.
    let mut seen = 0;
    let test = tests
        .iter()
        .filter(|t| {
            !t.cycles.is_empty()
                && t.initial_state.queue.len() == args.queue
                && t.bytes.first().is_some_and(|&b| is_prefix(b)) == args.prefixed
        })
        .find(|_| {
            let hit = seen == args.case;
            seen += 1;
            hit
        })
        .unwrap_or_else(|| {
            panic!(
                "no case {} in {} with a queue of {}",
                args.case, args.stem, args.queue
            )
        });

    let mut cpu = CpuBuilder::new()
        .with_cpu_type(CpuType::Intel8088)
        .with_trace_mode(TraceMode::CycleText)
        .build()
        .expect("build the reference CPU");
    run_reference(&mut cpu, test);

    if args.diff {
        let states = cpu.get_cycle_states().clone();
        let start = window_start(&states);
        let mc = their_microcode(cpu.get_cycle_trace(), start);
        let theirs = their_beats(&states);
        let Some(ours) = our_beats(test) else {
            println!("{}: this core did not terminate on that case", args.stem);
            return;
        };
        println!(
            "{} {}   ours {} cycles, the reference {}, the recording {}",
            args.stem,
            test.name,
            ours.len(),
            theirs.len(),
            test.cycles.len()
        );
        if theirs.len() != test.cycles.len() {
            println!(
                "  WARNING: the reference's own run and the recording disagree, so this \
                 probe is set up wrong and the comparison below is not trustworthy"
            );
        }
        println!();
        println!("  cyc  OURS                 THEIRS               MICROCODE");
        let rows = ours.len().max(theirs.len());
        let mut called = false;
        for i in 0..rows {
            let o = ours.get(i);
            let t = theirs.get(i);
            let mark = if o != t && !called {
                called = true;
                "<<"
            } else if o != t {
                " <"
            } else {
                "  "
            };
            println!(
                "{mark}{i:4}  {:<20} {:<20} {}",
                o.map(show).unwrap_or_default(),
                t.map(show).unwrap_or_default(),
                mc.get(i).cloned().unwrap_or_default()
            );
        }
        return;
    }

    println!(
        "{} {}  ({} cycles recorded)",
        args.stem,
        test.name,
        test.cycles.len()
    );
    println!();
    for (i, line) in cpu.get_cycle_trace().iter().enumerate() {
        println!("{i:4} {line}");
    }
}

/// Set the reference up and run one instruction, exactly as its own test runner
/// does (`cpu_test/run_tests.rs`): the reset vector carries CS:IP, and the queue
/// has to be handed over before `reset`, which flushes it.
fn run_reference(cpu: &mut marty_core::cpu_common::CpuDispatch, test: &CpuTest) {
    let r = &test.initial_state.regs;
    cpu.set_reset_vector(CpuAddress::Segmented(r.cs, r.ip));
    if !test.initial_state.queue.is_empty() {
        cpu.set_queue_contents(&test.initial_state.queue, true);
    }
    cpu.reset();

    cpu.set_register16(Register16::AX, r.ax);
    cpu.set_register16(Register16::BX, r.bx);
    cpu.set_register16(Register16::CX, r.cx);
    cpu.set_register16(Register16::DX, r.dx);
    cpu.set_register16(Register16::SP, r.sp);
    cpu.set_register16(Register16::BP, r.bp);
    cpu.set_register16(Register16::SI, r.si);
    cpu.set_register16(Register16::DI, r.di);
    cpu.set_register16(Register16::ES, r.es);
    cpu.set_register16(Register16::SS, r.ss);
    cpu.set_register16(Register16::DS, r.ds);
    cpu.set_flags(r.flags);

    for entry in &test.initial_state.ram {
        let byte: u8 = entry[1].try_into().expect("a byte");
        cpu.bus_mut()
            .write_u8(entry[0] as usize, byte, 0)
            .expect("seed memory");
    }

    // No wait states, matching the recording's machine, and cycle tracing on.
    cpu.set_option(CpuOption::EnableWaitStates(false));
    cpu.set_option(CpuOption::TraceLoggingEnabled(true));

    loop {
        match cpu.step(false) {
            Ok(_) => {
                if cpu.in_rep() {
                    continue;
                }
                break;
            }
            Err(e) => panic!("reference CPU error: {e}"),
        }
    }
    // `step_finish` is the RNI: it fetches the next instruction's first byte,
    // and the clocks it spends belong to the span the gate measures.
    let _ = cpu.step_finish(None);
}

fn show(b: &Beat) -> String {
    match b.addr {
        Some(a) => format!("{} {} {a:05X} q{}{}", b.bus, b.t, b.q, b.qlen),
        None => format!("{} {} ..... q{}{}", b.bus, b.t, b.q, b.qlen),
    }
}

/// Where does this core first part company with the reference, and on which
/// microcode line, over the whole corpus?
///
/// **This is the worklist.** A survey says which opcode files are wrong and by
/// how much; it cannot say what the part was doing at the moment the two
/// diverged. Ranking the first divergence by the reference's microcode line
/// turns "207 files differ somewhere" into a list of routines to transcribe,
/// commonest first, and two files that fail on the same line are one fix.
fn sweep(args: &Args) {
    let stems: Vec<String> = if args.stem == "all" {
        let dir = format!("{}/8088/v2", args.vectors);
        let mut v: Vec<String> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read {dir}: {e}"))
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_suffix(".json.gz").map(|s| s.to_string())
            })
            .collect();
        v.sort();
        v
    } else {
        vec![args.stem.clone()]
    };

    let mut cpu = CpuBuilder::new()
        .with_cpu_type(CpuType::Intel8088)
        .with_trace_mode(TraceMode::CycleText)
        .build()
        .expect("build the reference CPU");

    // line -> (times it carried the first divergence, the files it happened in)
    let mut by_line: std::collections::BTreeMap<
        String,
        (usize, std::collections::BTreeSet<String>),
    > = std::collections::BTreeMap::new();
    let mut agreed = 0usize;
    let mut compared = 0usize;
    // Cases where the reference's own run does not reproduce the recording it
    // was validated against. That is the probe being set up wrong, not a
    // finding, so they are excluded and counted rather than ranked.
    let mut mismatched = 0usize;

    for stem in &stems {
        let path = format!("{}/8088/v2/{}.json.gz", args.vectors, stem);
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut json = String::new();
        if flate2::read::GzDecoder::new(file)
            .read_to_string(&mut json)
            .is_err()
        {
            continue;
        }
        let Ok(tests) = serde_json::from_str::<Vec<CpuTest>>(&json) else {
            continue;
        };

        for test in tests
            .iter()
            .filter(|t| {
                !t.cycles.is_empty()
                    && t.initial_state.queue.len() == args.queue
                    && t.bytes.first().is_some_and(|&b| is_prefix(b)) == args.prefixed
            })
            .take(args.limit)
        {
            let Some(ours) = our_beats(test) else {
                continue;
            };
            run_reference(&mut cpu, test);
            let states = cpu.get_cycle_states().clone();
            let start = window_start(&states);
            let theirs = their_beats(&states);
            if theirs.len() != test.cycles.len() {
                mismatched += 1;
                continue;
            }
            compared += 1;
            let at = (0..ours.len().max(theirs.len())).find(|&i| ours.get(i) != theirs.get(i));
            let Some(at) = at else {
                agreed += 1;
                continue;
            };
            let mc = their_microcode(cpu.get_cycle_trace(), start);
            let line = mc
                .get(at)
                .cloned()
                .unwrap_or_else(|| "(past the reference's own trace)".to_string());
            let entry = by_line.entry(line).or_default();
            entry.0 += 1;
            entry.1.insert(stem.clone());
        }
    }

    println!("first divergence from the reference, by the microcode line it happened on");
    println!(
        "  {compared} cases compared, {agreed} identical cycle for cycle ({:.2}%)",
        100.0 * agreed as f64 / compared.max(1) as f64
    );
    if mismatched > 0 {
        println!("  {mismatched} skipped: the reference's own run did not reproduce the recording");
    }
    println!();
    let mut rows: Vec<_> = by_line.into_iter().collect();
    rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (line, (n, files)) in rows.iter().take(40) {
        let mut names: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        names.truncate(8);
        println!("  {n:6}  {line:<46}  {}", names.join(" "));
    }
}

/// The prefix bytes, so a case that starts with one can be skipped the way
/// every survey on the phosphor side skips it.
fn is_prefix(b: u8) -> bool {
    matches!(b, 0x26 | 0x2E | 0x36 | 0x3E | 0xF0 | 0xF1 | 0xF2 | 0xF3)
}

/// Where the reference's own run enters the span the gate measures: its first
/// first-byte queue read.
///
/// Its `cycle_states` and its trace strings are pushed on the same cycle
/// (`cycle.rs:224` and `:230`), so one index trims both.
fn window_start(states: &[CycleState]) -> usize {
    states
        .iter()
        .position(|c| matches!(c.q_op, marty_core::cpu_common::QueueOp::First))
        .unwrap_or(0)
}

/// The reference's cycles, reduced to [`Beat`].
///
/// **Taken from its live run, not from the corpus.** The corpus is the same
/// emulator's output, but its per-cycle format carries no queue length at all:
/// the deserializer hardcodes `q_len: 0` (`cpu_validator.rs:682`). A prefetch
/// decision turns on queue length, so diffing against the corpus cannot explain
/// one. Running the reference here gives the real length, and the microcode line
/// besides. `main` checks the live run against the corpus length so that a
/// mis-set-up probe cannot pass silently.
fn their_beats(states: &[CycleState]) -> Vec<Beat> {
    states[window_start(states)..]
        .iter()
        .map(|c| Beat {
            bus: match c.b_state {
                marty_core::cpu_validator::BusState::CODE => "CODE",
                marty_core::cpu_validator::BusState::MEMR => "MEMR",
                marty_core::cpu_validator::BusState::MEMW => "MEMW",
                marty_core::cpu_validator::BusState::IOR => "IOR",
                marty_core::cpu_validator::BusState::IOW => "IOW",
                marty_core::cpu_validator::BusState::INTA => "INTA",
                marty_core::cpu_validator::BusState::HALT => "HALT",
                marty_core::cpu_validator::BusState::PASV => "PASV",
            },
            // The reference's live state builder reports an idle bus as `T1`
            // (`mod.rs:1132`), where the recording it produces says `Ti`. A real
            // T1 always carries a status, so a passive `T1` is an idle cycle.
            t: match c.t_state {
                marty_core::cpu_validator::BusCycle::T1
                    if matches!(c.b_state, marty_core::cpu_validator::BusState::PASV) =>
                {
                    "Ti"
                }
                marty_core::cpu_validator::BusCycle::Ti => "Ti",
                marty_core::cpu_validator::BusCycle::T1 => "T1",
                marty_core::cpu_validator::BusCycle::T2 => "T2",
                marty_core::cpu_validator::BusCycle::T3 => "T3",
                marty_core::cpu_validator::BusCycle::T4 => "T4",
                marty_core::cpu_validator::BusCycle::Tw => "Tw",
            },
            // Only latched on T1, and only then is it meaningful to compare.
            addr: (matches!(c.t_state, marty_core::cpu_validator::BusCycle::T1)
                && !matches!(c.b_state, marty_core::cpu_validator::BusState::PASV))
            .then_some(c.addr),
            q: match c.q_op {
                marty_core::cpu_common::QueueOp::Idle => "-",
                marty_core::cpu_common::QueueOp::First => "F",
                marty_core::cpu_common::QueueOp::Subsequent => "S",
                marty_core::cpu_common::QueueOp::Flush => "E",
            },
            qlen: c.q_len,
        })
        .collect()
}

/// This core's cycles over the same span, reduced the same way.
///
/// **No shift is applied to the queue column.** The recording reports a queue
/// operation on the cycle after it happens (`q_op` is written from
/// `last_queue_op`), and this core reports on the cycle it happens, but each
/// window is anchored on its own first-byte report, so the offset is already
/// taken out by the alignment. Shifting again is what makes every queue event
/// look one clock out, which is the ambiguity that has cost the most time here.
fn our_beats(test: &CpuTest) -> Option<Vec<Beat>> {
    use phosphor_core::{
        core::{bus::BusMaster, component::BusMasterComponent},
        cpu::i8088::{BusStatus, QueueStatus, TState, I8088},
    };
    use phosphor_cpu_validation::TracingBus20;

    let mut cpu = I8088::new();
    let mut bus = TracingBus20::new();
    bus.memory.fill(0x90);
    let r = &test.initial_state.regs;
    cpu.ax = r.ax;
    cpu.bx = r.bx;
    cpu.cx = r.cx;
    cpu.dx = r.dx;
    cpu.cs = r.cs;
    cpu.ss = r.ss;
    cpu.ds = r.ds;
    cpu.es = r.es;
    cpu.sp = r.sp;
    cpu.bp = r.bp;
    cpu.si = r.si;
    cpu.di = r.di;
    cpu.ip = r.ip;
    cpu.flags = r.flags;
    for entry in &test.initial_state.ram {
        bus.memory[(entry[0] & 0xF_FFFF) as usize] = entry[1] as u8;
    }
    cpu.load_prefetch_queue(&test.initial_state.queue);

    let mut beats: Vec<Beat> = Vec::new();
    let mut ticks = 0usize;
    let mut measuring = false;
    let mut retired = false;
    loop {
        ticks += 1;
        if ticks > 4000 {
            return None;
        }
        let was_retired = retired;
        retired |= cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
        let next = matches!(cpu.queue_status, Some((QueueStatus::First, _))) && was_retired;
        if !measuring {
            if cpu.queue_status.is_some() {
                measuring = true;
            } else {
                continue;
            }
        } else if next {
            break;
        }
        beats.push(Beat {
            // The reference reports the status pins as passive outside T1 and
            // T2, because that is when they carry the status (`get_cycle_state`
            // in `mod.rs`). Ours holds the status through T4, so it is masked
            // here rather than treated as 5000 divergences per file.
            bus: match cpu.bus.status {
                _ if !matches!(cpu.bus.t_state, TState::T1 | TState::T2) => "PASV",
                BusStatus::Code => "CODE",
                BusStatus::MemRead => "MEMR",
                BusStatus::MemWrite => "MEMW",
                BusStatus::IoRead => "IOR",
                BusStatus::IoWrite => "IOW",
                BusStatus::Inta => "INTA",
                BusStatus::Halt => "HALT",
                BusStatus::Passive => "PASV",
            },
            t: match cpu.bus.t_state {
                TState::Idle => "Ti",
                TState::T1 => "T1",
                TState::T2 => "T2",
                TState::T3 => "T3",
                TState::T4 => "T4",
                TState::Wait => "Tw",
            },
            addr: matches!(cpu.bus.t_state, TState::T1).then(|| cpu.bus.address.unwrap_or(0)),
            q: match cpu.queue_status {
                None => "-",
                Some((QueueStatus::First, _)) => "F",
                Some((QueueStatus::Subsequent, _)) => "S",
                Some((QueueStatus::Emptied, _)) => "E",
            },
            qlen: cpu.queue_len() as u32,
        });
    }
    Some(beats)
}

/// The reference's microcode line on each cycle of the span, from its own run.
///
/// Trimmed by the same index as its cycle states, so the two columns line up.
fn their_microcode(trace: &[String], start: usize) -> Vec<String> {
    trace[start.min(trace.len())..]
        .iter()
        .map(|l| {
            // The microcode column is `NNN: SRC -> DST`, where `NNN` is three
            // characters: a hex line number, or `JMP`, `RET` or `COR`. Matching
            // on that shape rather than on a field index keeps this working if
            // the reference adds a column.
            let label = l
                .split('|')
                .map(str::trim)
                .find(|f| f.as_bytes().get(3) == Some(&b':'))
                .map(|f| f.to_string())
                .unwrap_or_default();
            let comments = l.rsplit('|').next().unwrap_or("").trim().to_string();
            if comments.is_empty() {
                label
            } else {
                format!("{label}  {comments}")
            }
        })
        .collect()
}
