//! What the hardware recording says an instruction costs, asked directly.
//!
//! These are surveys rather than checks, in the same sense as
//! `i8088_multiply_timing.rs`: none of them asserts anything, and each exists to
//! answer one question about the part that Intel's Table 1-16 either does not
//! answer or answers wrongly. They are `#[ignore]`d, so `cargo test` runs none
//! of them; run one by name when a timing row is in question.
//!
//! ```text
//! PHOSPHOR_REQUIRE_VECTORS=1 cargo test --release -p phosphor-cpu-validation \
//!     --test i8088_transfer_timing -- --ignored --nocapture
//! ```
//!
//! # The measurement, and why it can be trusted
//!
//! Every survey restricts itself to the cases that begin with a **full prefetch
//! queue** and carry **no prefix**. That is what makes the numbers comparable:
//! the instruction's own bytes are already in hand, so nothing in the span is
//! waiting for a fetch that a differently-scheduled BIU might have issued
//! earlier. Under that restriction the recorded span from an instruction's
//! First Byte to the next one's *is* the instruction's clock count, and for
//! most of the instruction set it agrees with Table 1-16 exactly, to the cycle,
//! over thousands of cases.
//!
//! Where it does not agree, these surveys are the evidence. Each prints a
//! histogram, so a row that is uniform says so with its case count beside it,
//! and a row that is data-dependent shows two groups rather than an average.
//!
//! # What they established
//!
//! - **The control transfers cost three to eight clocks less than the manual
//!   says**, uniformly per row, once the queue flush and reload are accounted
//!   separately. [`control_transfer_survey`] and [`required_eu_clocks`].
//! - **A fetched byte reaches the EU on the cycle after T4**, never on T4
//!   itself. [`empty_queue_traces`] shows it three ways: an instruction fetched
//!   into an empty queue has its opcode latched on T3 and read two cycles
//!   later.
//! - **A segment override costs two clocks**, on the register forms as much as
//!   the memory ones, so it belongs to the prefix and not to the address
//!   calculation. [`required_ea_clocks`].
//! - **The effective-address table is right, including its asymmetric
//!   pairings**, confirmed through `LEA`, the one instruction that computes an
//!   address and runs no bus cycle. [`required_ea_clocks`].

use std::collections::BTreeMap;
use std::io::Read;

use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8088::{I8088, QueueStatus};
use phosphor_cpu_validation::{I8088TestCase, QueueOp, TracingBus20};

fn load(stem: &str) -> Option<Vec<I8088TestCase>> {
    let dir = phosphor_cpu_validation::vector_dir("8088/v2");
    if !phosphor_cpu_validation::require_test_data(&dir, "vectors") {
        return None;
    }
    let path = dir.join(format!("{stem}.json.gz"));
    let gz = std::fs::read(&path).ok()?;
    let mut json = String::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_string(&mut json)
        .expect("decompresses");
    Some(serde_json::from_str(&json).expect("parses"))
}

fn is_prefix(b: u8) -> bool {
    matches!(b, 0x26 | 0x2E | 0x36 | 0x3E | 0xF0 | 0xF2 | 0xF3)
}

/// Replay one case through this core and return the span it takes, measured
/// exactly as the gate measures it: from the cycle the queue status lines
/// report a First Byte to the cycle they report the next instruction's.
///
/// A smaller copy of `i8088_cycle_test`'s replay, which is private to that test
/// binary. It exists here so a single opcode's residual can be looked at
/// directly, which is the difference between "this row is two clocks out on
/// every case" and "this row is right and something else is wrong".
fn replay(tc: &I8088TestCase) -> Option<usize> {
    let mut cpu = I8088::new();
    let mut bus = TracingBus20::new();
    bus.memory.fill(0x90);

    let r = &tc.initial.regs;
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
    for &(addr, val) in &tc.initial.ram {
        bus.memory[(addr & 0xF_FFFF) as usize] = val;
    }
    cpu.load_prefetch_queue(&tc.initial.queue);

    let mut ticks = 0usize;
    let mut measuring = false;
    let mut retired = false;
    let mut elapsed = 0usize;
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
        } else {
            elapsed += 1;
        }
    }
    Some(elapsed + 1)
}

/// How far this core is from the recording on one opcode file, as a histogram
/// of signed differences over the cases that begin with a full queue.
fn residuals(stem: &str) {
    let Some(tests) = load(stem) else { return };
    let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some(ours) = replay(tc) else { continue };
        *hist
            .entry(ours as i64 - tc.cycles.len() as i64)
            .or_default() += 1;
    }
    let total: usize = hist.values().sum();
    let mut modes: Vec<(i64, usize)> = hist.into_iter().collect();
    modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    let top: Vec<String> = modes
        .iter()
        .take(5)
        .map(|(d, n)| format!("{d:+}:{n}"))
        .collect();
    eprintln!(
        "  {stem}: {total} cases, ours minus hardware {}",
        top.join(" ")
    );
}

#[test]
#[ignore = "survey, not a check: how far each row is from the recording"]
fn row_residuals() {
    eprintln!("\nresiduals against the recording, full queue, no prefix");
    for stem in [
        "70", "E0", "E1", "E2", "E3", "E8", "E9", "EA", "EB", "9A", "C2", "C3", "CA", "CB", "CC",
        "CD", "CE", "CF", "A0", "A1", "A2", "A3", "D7", "98", "99", "9E", "9F", "27", "37", "C4",
        "C5", "F8", "FF.2", "FF.3", "FF.4", "FF.5", "8B", "01", "50", "58", "90",
    ] {
        residuals(stem);
    }
}

/// Histogram of recorded span lengths, split by taken and not taken, over the
/// unprefixed cases that began with a full queue.
fn survey(stem: &str, split_reg: bool) {
    let Some(tests) = load(stem) else { return };
    // key: (taken, reg field or 8, register operand?)
    let mut groups: BTreeMap<(bool, u8, bool), BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let taken = tc
            .cycles
            .iter()
            .any(|c| matches!(c.queue_op(), Some((QueueOp::Emptied, _))));
        let (reg, is_reg_operand) = if split_reg {
            match tc.bytes.get(1) {
                Some(&modrm) => ((modrm >> 3) & 7, modrm >> 6 == 3),
                None => (8, false),
            }
        } else {
            (8, false)
        };
        *groups
            .entry((taken, reg, is_reg_operand))
            .or_default()
            .entry(tc.cycles.len())
            .or_default() += 1;
    }

    eprintln!("\n{stem}: recorded span lengths, full queue, no prefix");
    for ((taken, reg, is_reg), hist) in &groups {
        let total: usize = hist.values().sum();
        let mut modes: Vec<(usize, usize)> = hist.iter().map(|(k, v)| (*k, *v)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(6)
            .map(|(len, n)| format!("{len}:{n}"))
            .collect();
        let label = if *reg == 8 {
            String::new()
        } else {
            format!(" reg={reg} {}", if *is_reg { "r" } else { "mem" })
        };
        eprintln!(
            "  {}{label}: {total} cases, lengths {}",
            if *taken { "taken    " } else { "not taken" },
            top.join(" ")
        );
    }
}

/// Dump the first prefetched, unprefixed case of an opcode file, cycle by
/// cycle, so the shape of the transfer is visible rather than inferred.
fn dump(stem: &str, want_taken: bool) {
    let Some(tests) = load(stem) else { return };
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let taken = tc
            .cycles
            .iter()
            .any(|c| matches!(c.queue_op(), Some((QueueOp::Emptied, _))));
        if taken != want_taken {
            continue;
        }
        eprintln!("\n{stem} {} ({} cycles)", tc.name, tc.cycles.len());
        for (i, c) in tc.cycles.iter().enumerate() {
            let q = match c.queue_op() {
                Some((op, b)) => format!("{op:?}:{b:02X}"),
                None => "-".to_string(),
            };
            let addr = match c.address() {
                Some(a) => format!("{a:05X}"),
                None => "     ".to_string(),
            };
            eprintln!(
                "  {i:3} {:?} {:?} {addr} data={:02X} q={q}",
                c.status(),
                c.t_state(),
                c.6
            );
        }
        return;
    }
}

/// Dump the first case of an opcode file that begins with an empty queue.
fn dump_empty(stem: &str) {
    let Some(tests) = load(stem) else { return };
    for tc in &tests {
        if tc.cycles.is_empty() || !tc.initial.queue.is_empty() {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        eprintln!(
            "\n{stem} {} ({} cycles, empty queue)",
            tc.name,
            tc.cycles.len()
        );
        for (i, c) in tc.cycles.iter().enumerate() {
            let q = match c.queue_op() {
                Some((op, b)) => format!("{op:?}:{b:02X}"),
                None => "-".to_string(),
            };
            let addr = match c.address() {
                Some(a) => format!("{a:05X}"),
                None => "     ".to_string(),
            };
            eprintln!(
                "  {i:3} {:?} {:?} {addr} data={:02X} q={q}",
                c.status(),
                c.t_state(),
                c.6
            );
        }
        return;
    }
}

/// The modal recorded span for the simplest population there is: a full queue,
/// no prefix, and a register operand where there is a ModR/M byte at all. If
/// Table 1-16's clock counts describe anything directly observable, it is this.
fn modal_span(stems: &[&str]) {
    eprintln!("\nmodal recorded span, full queue, no prefix, register operand");
    for stem in stems {
        let Some(tests) = load(stem) else { continue };
        let mut hist: BTreeMap<usize, usize> = BTreeMap::new();
        let mut bytes = 0usize;
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            if let Some(&modrm) = tc.bytes.get(1)
                && modrm >> 6 != 3
                && *stem != "05"
                && *stem != "04"
            {
                continue;
            }
            bytes = tc.bytes.len();
            *hist.entry(tc.cycles.len()).or_default() += 1;
        }
        let mut modes: Vec<(usize, usize)> = hist.into_iter().collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(len, n)| format!("{len}:{n}"))
            .collect();
        eprintln!("  {stem} ({bytes} bytes): {}", top.join(" "));
    }
}

#[test]
#[ignore = "survey, not a check: the recorded span of the simplest forms there are"]
fn modal_spans() {
    modal_span(&[
        "90", "40", "48", "50", "58", "04", "05", "00", "01", "02", "03", "88", "8A", "8C", "8E",
        "B0", "B8", "F8", "F9", "FA", "FB", "FC", "FD", "F5", "98", "99", "9E", "9F", "9C", "9D",
        "D7", "27", "2F", "37", "3F", "D4", "D5", "C4", "C5",
    ]);
}

#[test]
#[ignore = "survey, not a check: when a fetched byte reaches the EU"]
fn empty_queue_traces() {
    dump_empty("90");
    dump_empty("40");
    dump_empty("81.0");
    dump_empty("B8");
}

#[test]
#[ignore = "survey, not a check: where a transfer flushes and how it reloads"]
fn control_transfer_traces() {
    dump("EA", true);
    dump("9A", true);
    dump("EB", true);
    dump("70", true);
    dump("70", false);
    dump("E8", true);
    dump("C3", true);
}

/// This core's effective-address clocks, copied so the survey can subtract
/// them. See `core/src/cpu/i8088/access.rs`, which is the real table.
fn ea_cycles(modrm: u8, segment_override: bool) -> usize {
    let base = match ((modrm >> 6) & 3, modrm & 7) {
        (0, 6) => 6,
        (0, 4 | 5 | 7) => 5,
        (0, 0 | 3) => 7,
        (0, 1 | 2) => 8,
        (1 | 2, 4..=7) => 9,
        (1 | 2, 0 | 3) => 11,
        (1 | 2, 1 | 2) => 12,
        _ => 0,
    };
    base + if segment_override { 2 } else { 0 }
}

/// What the EU time would have to be for this core's span to equal the
/// hardware's, given everything the pipeline spends structurally: the bus
/// cycles, the effective address, and, for a taken transfer, the queue flush
/// and the reload of the first byte at the target.
///
/// `stack` is the number of stack *bytes* the pipeline moves; `operand` the
/// number of ModR/M operand bytes it reads.
fn required_eu(stem: &str, transfers: bool, stack: usize, operand: usize) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(bool, bool), BTreeMap<i64, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let taken = tc
            .cycles
            .iter()
            .any(|c| matches!(c.queue_op(), Some((QueueOp::Emptied, _))));
        let (ea, is_mem) = match tc.bytes.get(1) {
            Some(&modrm) if operand > 0 || stem.starts_with("FF") => {
                if modrm >> 6 == 3 {
                    (0, false)
                } else {
                    (ea_cycles(modrm, false), true)
                }
            }
            _ => (0, false),
        };
        let bus = 4 * (stack + if is_mem { operand } else { 0 });
        let reload = if taken && transfers { 7 } else { 0 };
        let eu = tc.cycles.len() as i64 - (reload + bus + ea) as i64;
        *groups
            .entry((taken, is_mem))
            .or_default()
            .entry(eu)
            .or_default() += 1;
    }

    eprintln!("\n{stem}: required EU clocks");
    for ((taken, is_mem), hist) in &groups {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(k, v)| (*k, *v)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let total: usize = hist.values().sum();
        let top: Vec<String> = modes
            .iter()
            .take(5)
            .map(|(eu, n)| format!("{eu}:{n}"))
            .collect();
        eprintln!(
            "  {} {}: {total} cases, eu {}",
            if *taken { "taken    " } else { "not taken" },
            if *is_mem { "mem" } else { "reg" },
            top.join(" ")
        );
    }
}

/// The same question asked per addressing mode: what would the effective
/// address have to cost for the required EU time to come out the same as the
/// register form's? Any row that disagrees is an EA row this core has wrong.
fn required_ea(stem: &str, operand: usize) {
    required_ea_inner(stem, operand, false);
    required_ea_inner(stem, operand, true);
}

fn required_ea_inner(stem: &str, operand: usize, prefixed: bool) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(u8, u8), BTreeMap<i64, usize>> = BTreeMap::new();
    let mut reg_form: BTreeMap<i64, usize> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        // Either no prefix at all, or exactly one segment override.
        let has_prefix = tc.bytes.first().is_some_and(|&b| is_prefix(b));
        if has_prefix != prefixed {
            continue;
        }
        if prefixed
            && (!matches!(tc.bytes.first(), Some(0x26 | 0x2E | 0x36 | 0x3E))
                || tc.bytes.get(1).is_some_and(|&b| is_prefix(b)))
        {
            continue;
        }
        let Some(&modrm) = tc.bytes.get(if prefixed { 2 } else { 1 }) else {
            continue;
        };
        let (m, rm) = (modrm >> 6, modrm & 7);
        if m == 3 {
            *reg_form.entry(tc.cycles.len() as i64).or_default() += 1;
            continue;
        }
        // span - bus - eu_reg = what the EA and the extra memory-form EU time
        // add up to.
        *groups
            .entry((m, rm))
            .or_default()
            .entry(tc.cycles.len() as i64 - (4 * operand) as i64)
            .or_default() += 1;
    }
    let reg = reg_form.keys().next().copied().unwrap_or(0);
    eprintln!(
        "\n{stem}{}: span minus bus time, by addressing mode (register form is {reg})",
        if prefixed {
            " with a segment override"
        } else {
            ""
        }
    );
    for ((m, rm), hist) in &groups {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(k, v)| (*k, *v)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(3)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!(
            "  mod={m} rm={rm}  ours_ea={:2}  {}",
            ea_cycles(*m << 6 | *rm, false),
            top.join(" ")
        );
    }
}

#[test]
#[ignore = "survey, not a check: what the effective address and a prefix cost"]
fn required_ea_clocks() {
    required_ea("8B", 2);
    required_ea("03", 2);
    // LEA is the discriminator: it computes an effective address and runs no
    // bus cycle at all, so its span is its EU time plus the EA and nothing
    // else. If the extra clock the memory forms above need belongs to the
    // address calculation, LEA pays it too; if it belongs to reaching memory,
    // LEA does not.
    required_ea("8D", 0);
    // And a store, to see whether the write path carries an offset of its own.
    required_ea("89", 2);
}

/// The same measurement for an opcode with no ModR/M byte at all, where the
/// operand's address is in the instruction: the direct-address `MOV`s and
/// `XLAT`. `bus` is the number of operand bytes the instruction moves.
fn required_eu_flat(stem: &str, bus: usize) {
    let Some(tests) = load(stem) else { return };
    let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        *hist
            .entry(tc.cycles.len() as i64 - (4 * bus) as i64)
            .or_default() += 1;
    }
    let mut modes: Vec<(i64, usize)> = hist.into_iter().collect();
    modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    let top: Vec<String> = modes
        .iter()
        .take(4)
        .map(|(v, n)| format!("{v}:{n}"))
        .collect();
    eprintln!("  {stem}: eu {}", top.join(" "));
}

/// Split an opcode's recorded spans by a property of the initial state, to see
/// whether a two-valued row is the microcode branching on it.
fn split_by(stem: &str, label: &str, key: fn(&I8088TestCase) -> bool) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<bool, BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        *groups
            .entry(key(tc))
            .or_default()
            .entry(tc.cycles.len())
            .or_default() += 1;
    }
    eprintln!("\n{stem} by {label}");
    for (k, hist) in &groups {
        let mut modes: Vec<(usize, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!("  {k:5}: {}", top.join(" "));
    }
}

#[test]
#[ignore = "survey, not a check: which microcode branch a two-valued row takes"]
fn data_dependent_rows() {
    // CWD sign-extends AX into DX, and the recording gives it two costs.
    split_by("99", "AX negative", |tc| tc.initial.regs.ax & 0x8000 != 0);
    // AAA and AAS adjust when the low nibble is above 9 or the auxiliary carry
    // is set, which is the condition the microcode branches on.
    for stem in ["37", "3F"] {
        split_by(stem, "the adjust condition", |tc| {
            (tc.initial.regs.ax & 0x0F) > 9 || tc.initial.regs.flags & 0x10 != 0
        });
    }
    // And DAA and DAS, which have the same shape and did not split at all.
    for stem in ["27", "2F"] {
        split_by(stem, "the adjust condition", |tc| {
            (tc.initial.regs.ax & 0x0F) > 9 || tc.initial.regs.flags & 0x10 != 0
        });
    }
}

#[test]
#[ignore = "survey, not a check: EU clocks for the opcodes with no ModR/M byte"]
fn required_eu_without_a_modrm_byte() {
    eprintln!("\nrequired EU clocks, opcodes with no ModR/M byte");
    required_eu_flat("A0", 1);
    required_eu_flat("A1", 2);
    required_eu_flat("A2", 1);
    required_eu_flat("A3", 2);
    required_eu_flat("D7", 1);
    required_eu_flat("98", 0);
    required_eu_flat("99", 0);
    required_eu_flat("9E", 0);
    required_eu_flat("9F", 0);
    required_eu_flat("27", 0);
    required_eu_flat("2F", 0);
    required_eu_flat("37", 0);
    required_eu_flat("3F", 0);
}

#[test]
#[ignore = "survey, not a check: the EU clocks each control transfer needs"]
fn required_eu_clocks() {
    required_eu("70", true, 0, 0);
    required_eu("E0", true, 0, 0);
    required_eu("E1", true, 0, 0);
    required_eu("E2", true, 0, 0);
    required_eu("E3", true, 0, 0);
    required_eu("E8", true, 2, 0);
    required_eu("E9", true, 0, 0);
    required_eu("EA", true, 0, 0);
    required_eu("EB", true, 0, 0);
    required_eu("9A", true, 4, 0);
    required_eu("C2", true, 2, 0);
    required_eu("C3", true, 2, 0);
    required_eu("CA", true, 4, 0);
    required_eu("CB", true, 4, 0);
    required_eu("CC", true, 6, 0);
    required_eu("CD", true, 6, 0);
    required_eu("CE", true, 6, 0);
    required_eu("CF", true, 6, 0);
    // Controls: ordinary memory-operand instructions whose EU time is already
    // modeled and whose gate rate is decent. If their required EU is uniform
    // across addressing modes, the effective-address table explains the whole
    // spread and any residual spread elsewhere is that instruction's own.
    required_eu("03", false, 0, 2);
    required_eu("01", false, 0, 4);
    required_eu("8B", false, 0, 2);
    // The far-pointer loads, which read four bytes and write two registers.
    required_eu("C4", false, 0, 4);
    required_eu("C5", false, 0, 4);
    required_eu("FF.2", true, 2, 2);
    required_eu("FF.3", true, 4, 4);
    required_eu("FF.4", true, 0, 2);
    required_eu("FF.5", true, 0, 4);
}

#[test]
#[ignore = "survey, not a check: recorded spans, taken against not taken"]
fn control_transfer_survey() {
    for stem in [
        "70", "7F", "E0", "E1", "E2", "E3", "E8", "E9", "EA", "EB", "9A", "C0", "C1", "C2", "C3",
        "C8", "C9", "CA", "CB", "CC", "CD", "CE", "CF", "FF.2", "FF.3", "FF.4", "FF.5",
    ] {
        survey(stem, false);
    }
}
