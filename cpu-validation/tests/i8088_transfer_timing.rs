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
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Whether the opcode's second byte is a ModR/M byte, so a survey knows whether
/// it has an addressing mode to group by at all. The stack instructions and the
/// I/O forms do not.
fn format_has_modrm(opcode: u8) -> bool {
    !matches!(
        opcode,
        0x04 | 0x05 | 0x0C | 0x0D | 0x14 | 0x15 | 0x1C | 0x1D
            | 0x24 | 0x25 | 0x2C | 0x2D | 0x34 | 0x35 | 0x3C | 0x3D
            | 0x06 | 0x07 | 0x0E | 0x0F | 0x16 | 0x17 | 0x1E | 0x1F
            | 0x27 | 0x2F | 0x37 | 0x3F
            | 0x40..=0x5F
            | 0x90..=0x9F
            | 0xA0..=0xAF
            | 0xB0..=0xBF
            | 0xC2 | 0xC3 | 0xCA | 0xCB | 0xCC..=0xCF
            | 0xD4..=0xD7
            | 0xE0..=0xEF
            | 0xF4 | 0xF5 | 0xF8..=0xFD
    )
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

/// Replay a case and report the cycle on which this core drives T1 of its
/// first *data* bus cycle, counted from the start of the measured span. The
/// recording carries the same quantity directly, so the two can be compared
/// per addressing mode: where the operand access starts is the question the
/// residual is really asking.
fn replay_operand_start(tc: &I8088TestCase) -> Option<usize> {
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
            return None;
        } else {
            elapsed += 1;
        }
        let data_cycle = matches!(
            cpu.bus.status,
            phosphor_core::cpu::i8088::BusStatus::MemRead
                | phosphor_core::cpu::i8088::BusStatus::MemWrite
        );
        if data_cycle && cpu.bus.address.is_some() {
            return Some(elapsed);
        }
    }
}

/// Replay a case and report its first *code* fetch: which cycle of the span
/// drove T1 of it, and what address it latched.
///
/// The counterpart of [`replay_operand_start`] for the other kind of bus cycle,
/// and the one that measures the loader rather than the operand path. The
/// bus-cycle comparison fails on nearly every file with the whole fetch stream
/// shifted by one position, which is either this core starting a fetch the part
/// had already finished or the part starting one this core has not reached. The
/// address says which.
fn replay_first_code(tc: &I8088TestCase) -> Option<(usize, u32)> {
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
            return None;
        } else {
            elapsed += 1;
        }
        if cpu.bus.status == phosphor_core::cpu::i8088::BusStatus::Code
            && let Some(addr) = cpu.bus.address
        {
            return Some((elapsed, addr));
        }
    }
}

/// The same quantity out of the recording: the first cycle that latches an
/// address for an instruction fetch.
fn recorded_first_code(tc: &I8088TestCase) -> Option<(usize, u32)> {
    tc.cycles.iter().enumerate().find_map(|(i, c)| {
        match (
            c.address(),
            c.status() == phosphor_cpu_validation::BusStatus::CODE,
        ) {
            (Some(addr), true) => Some((i, addr)),
            _ => None,
        }
    })
}

/// The same quantity out of the recording: the index of the first cycle that
/// latches an address for a data read or write.
fn recorded_operand_start(tc: &I8088TestCase) -> Option<usize> {
    tc.cycles.iter().position(|c| {
        c.address().is_some()
            && matches!(
                c.status(),
                phosphor_cpu_validation::BusStatus::MEMR | phosphor_cpu_validation::BusStatus::MEMW
            )
    })
}

/// Where the operand's bus cycle starts, ours against the recording, by mode.
fn operand_start_by_mode(stem: &str) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(u8, u8), BTreeMap<(usize, usize), usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some(&modrm) = tc.bytes.get(1) else {
            continue;
        };
        if modrm >> 6 == 3 {
            continue;
        }
        let (Some(ours), Some(theirs)) = (replay_operand_start(tc), recorded_operand_start(tc))
        else {
            continue;
        };
        *groups
            .entry((modrm >> 6, modrm & 7))
            .or_default()
            .entry((ours, theirs))
            .or_default() += 1;
    }
    eprintln!("\n{stem}: first data bus cycle, (ours, hardware), by mode");
    for ((m, rm), hist) in &groups {
        let mut modes: Vec<((usize, usize), usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(3)
            .map(|((o, t), n)| format!("({o},{t}):{n}"))
            .collect();
        eprintln!(
            "  mod={m} rm={rm}  ea={:2}  {}",
            ea_cycles(m << 6 | rm, false),
            top.join(" ")
        );
    }
}

/// Where the loader's first instruction fetch lands, ours against the
/// recording.
///
/// The answer is that on a full queue it lands exactly right: cycle 2 and the
/// same address, on every case of every file tried. So the bus-cycle order's
/// shortfall is not the loader starting its first fetch on the wrong clock, and
/// the EU-before-BIU ordering inside `execute_cycle` is not the cause either.
///
/// **Full-queue cases only, and that restriction is the point.** The first
/// version of this also grouped the empty-queue cases and printed a confident
/// `(0, 3, -1)` for them, which measured nothing: for a case that starts with
/// an empty queue the recorded trace window opens partway through the opcode's
/// own fetch, so the T1 that carries its address is outside the trace and the
/// first address-carrying cycle in the recording is already the *second* fetch.
/// Our index counts from the First Byte and theirs from the start of the trace,
/// which are different origins, and the `-1` is the missing opcode fetch rather
/// than a disagreement. Comparing the two needs the whole ordered list, which
/// is what `i8088_cycle_test` already does.
fn code_start_by_queue_length(stem: &str) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<usize, BTreeMap<(i64, i64, i64), usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let (Some((ours, our_addr)), Some((theirs, their_addr))) =
            (replay_first_code(tc), recorded_first_code(tc))
        else {
            continue;
        };
        *groups
            .entry(tc.initial.queue.len())
            .or_default()
            .entry((
                ours as i64,
                theirs as i64,
                our_addr as i64 - their_addr as i64,
            ))
            .or_default() += 1;
    }
    eprintln!("\n{stem}: first code fetch (our cycle, their cycle, address delta)");
    for (queued, hist) in &groups {
        let mut modes: Vec<((i64, i64, i64), usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let total: usize = hist.values().sum();
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|((o, t, d), n)| format!("({o},{t},{d:+}):{n}"))
            .collect();
        eprintln!("  queue {queued}: {total} cases  {}", top.join(" "));
    }
}

/// One case, cycle by cycle, ours beside the recording.
///
/// The instrument for "the part ran a bus cycle here and we did not". A
/// bus-cycle list says *which* transaction is missing; this says what both
/// machines were doing on the cycle it should have started, and it carries our
/// queue depth, which is what tells an idle BIU apart from a full queue. Those
/// are very different bugs and the transaction list cannot separate them.
///
/// `want_mem` picks a memory-operand form, which is where the differences are.
fn dump_side_by_side(stem: &str, want_mem: bool, queued: usize) {
    let Some(tests) = load(stem) else { return };
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != queued {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        // An opcode with no ModR/M byte has no addressing mode to filter on,
        // and the stack instructions are exactly that shape.
        if let Some(&modrm) = tc.bytes.get(1)
            && format_has_modrm(tc.bytes[0])
            && (modrm >> 6 != 3) != want_mem
        {
            continue;
        }

        // Replay, recording one line per cycle over the same span the gate
        // measures.
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

        let mut mine: Vec<String> = Vec::new();
        let mut ticks = 0usize;
        let mut measuring = false;
        let mut retired = false;
        loop {
            ticks += 1;
            if ticks > 4000 {
                break;
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
            let q = match cpu.queue_status {
                Some((op, b)) => format!("{op:?}:{b:02X}"),
                None => "-".to_string(),
            };
            let addr = match cpu.bus.address {
                Some(a) => format!("{a:05X}"),
                None => "     ".to_string(),
            };
            mine.push(format!(
                "{:?} {:?} {addr} q={q} len={}",
                cpu.bus.status,
                cpu.bus.t_state,
                cpu.queue_len()
            ));
        }

        eprintln!(
            "\n{stem} {} ({} cycles hardware, {} ours)",
            tc.name,
            tc.cycles.len(),
            mine.len()
        );
        eprintln!("  {:<38}  HARDWARE", "OURS");
        let rows = mine.len().max(tc.cycles.len());
        for i in 0..rows {
            let ours = mine.get(i).cloned().unwrap_or_default();
            let theirs = match tc.cycles.get(i) {
                Some(c) => {
                    let q = match c.queue_op() {
                        Some((op, b)) => format!("{op:?}:{b:02X}"),
                        None => "-".to_string(),
                    };
                    let addr = match c.address() {
                        Some(a) => format!("{a:05X}"),
                        None => "     ".to_string(),
                    };
                    format!("{:?} {:?} {addr} q={q}", c.status(), c.t_state())
                }
                None => String::new(),
            };
            eprintln!("  {i:3} {ours:<38}  {theirs}");
        }
        return;
    }
}

#[test]
#[ignore = "survey, not a check: one case, ours beside the recording"]
fn side_by_side() {
    // The indirect near call, whose memory forms run a clock long and whose
    // register forms run a clock short, uniformly and in opposite directions.
    dump_side_by_side("FF.2", true, 4);
    dump_side_by_side("FF.2", false, 4);
    // The read-modify-write form that runs a clock long and still fits one
    // fewer prefetch than the part, which additive time cannot explain.
    dump_side_by_side("F7.3", true, 4);
    // And a read-only memory form, which runs a clock short.
    dump_side_by_side("F7.4", true, 4);
    // The four-byte *register* form the loader's clock does not reach. It
    // drains a full queue exactly, so the refill behind it may be what fixes
    // the span rather than the microcode.
    dump_side_by_side("81.0", false, 4);
    // The discriminator. Also four bytes and also a register form, but
    // documented five clocks against `81`'s four. If the span is the refill's
    // it matches `81.0`; if it is the microcode's it is a clock longer.
    dump_side_by_side("F7.0", false, 4);
    dump_side_by_side("C7.0", false, 4);
    // And the pair that refuses the same clock. `9A` and `EA` are five bytes,
    // so from a full queue they drain it and then wait for their last byte
    // themselves. Delaying the refill by one made both `+1` on every case while
    // it made the four-byte forms exact, and at the first pop the BIU cannot
    // tell them apart. Whatever separates them is in these two traces.
    dump_side_by_side("9A", false, 4);
    dump_side_by_side("EA", false, 4);
    // The stack pair, from a full queue. `POP` is exact cycle for cycle since
    // its microcode was transcribed; `PUSH` is the one that still runs short.
    dump_side_by_side("58", false, 4);
    dump_side_by_side("50", false, 4);
    // `MOV r/m, imm`, both directions of the operand. Its routine writes but
    // never reads, so it is the one transcribed group whose address is
    // computed and not loaded, and the only one whose immediate the loader
    // defers without a read to defer it behind.
    dump_side_by_side("C7", false, 4);
    dump_side_by_side("C7", true, 4);
}

/// Replay a case and count the code fetches this core starts before its first
/// data bus cycle, over the gate's span.
fn replay_fetches_before_data(tc: &I8088TestCase) -> Option<usize> {
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
    let mut fetches = 0usize;
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
            return None;
        }
        if cpu.bus.address.is_some() {
            match cpu.bus.status {
                phosphor_core::cpu::i8088::BusStatus::Code => fetches += 1,
                phosphor_core::cpu::i8088::BusStatus::MemRead
                | phosphor_core::cpu::i8088::BusStatus::MemWrite => return Some(fetches),
                _ => {}
            }
        }
    }
}

/// The same count out of the recording, with the allowance the gate makes: a
/// case that begins with an empty queue opens on the T2 of a fetch already in
/// flight, whose T1 and therefore whose address is outside the trace, so the
/// recording cannot show it and one has to be added back.
fn recorded_fetches_before_data(tc: &I8088TestCase) -> Option<usize> {
    use phosphor_cpu_validation::{BusStatus, TState};
    let opens_mid_fetch = tc
        .cycles
        .first()
        .is_some_and(|c| c.t_state() != TState::T1 && c.status() == BusStatus::CODE);
    let mut fetches = usize::from(opens_mid_fetch);
    for c in &tc.cycles {
        if c.address().is_none() {
            continue;
        }
        match c.status() {
            BusStatus::CODE => fetches += 1,
            BusStatus::MEMR | BusStatus::MEMW => return Some(fetches),
            _ => {}
        }
    }
    None
}

/// How long after a code fetch completes does the EU take the byte, when the
/// queue was empty and the EU was waiting for it?
///
/// The recording talking to itself: no replay, so this cannot be answered
/// wrongly by this core being wrong. **The queue-status lines are reported one
/// T-state late and the bus columns beside them are not**, which the suite
/// documents and which the gate has since confirmed from the other end, so
/// every read row here is corrected by one. Reading both off the same row is
/// comparing two origins, and doing that put the floor at two and cost an
/// experiment.
///
/// The split is the hypothesis. `9A` and `EA` are five bytes: from a full queue
/// they drain it and then wait, and the byte they wait for continues the
/// instruction, arriving as a **Subsequent**. `ADD BP, imm16` and `TEST AX,
/// imm16` are four bytes: they drain the queue exactly and the byte behind them
/// is the next instruction's opcode, a **First**. Both take seven clocks on the
/// part against this core's six, though their rows differ by one. If the
/// boundary costs a clock that continuing does not, it is the difference
/// between those two columns, and nothing else here can express it.
#[test]
#[ignore = "survey, not a check: when the EU takes a byte it was waiting for"]
fn queue_delivery_latency() {
    use phosphor_cpu_validation::{BusStatus, TState};

    // (the case's initial queue, what the byte was read as) -> latency histogram
    let mut hist: BTreeMap<(&'static str, &'static str), BTreeMap<usize, usize>> = BTreeMap::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        for tc in &tests {
            if tc.cycles.is_empty() {
                continue;
            }
            let queue = match tc.initial.queue.len() {
                0 => "empty",
                4 => "full ",
                _ => "part ",
            };
            // The EU's reads, at the T-state they actually happened on rather
            // than the one they were reported on.
            let reads: Vec<(usize, QueueOp)> = tc
                .cycles
                .iter()
                .enumerate()
                .filter_map(|(i, c)| match c.queue_op().map(|(op, _)| op) {
                    Some(op @ (QueueOp::First | QueueOp::Subsequent)) => Some((i.max(1) - 1, op)),
                    _ => None,
                })
                .collect();

            // A fetch whose T1 is outside the window cannot be located, so only
            // those that begin inside it are counted. T4 is three rows on: the
            // suite's cases incur no wait states.
            let mut depth = tc.initial.queue.len() as i32;
            let mut waited: Vec<usize> = Vec::new();
            for i in 0..tc.cycles.len() {
                if reads.iter().any(|&(r, _)| r == i) {
                    depth -= 1;
                }
                if tc.cycles[i].queue_op().map(|(op, _)| op) == Some(QueueOp::Emptied) {
                    depth = 0;
                }
                let began = i.checked_sub(3).map(|t1| &tc.cycles[t1]);
                if began.is_some_and(|c| c.status() == BusStatus::CODE && c.t_state() == TState::T1)
                {
                    // Empty entering this T-state, so the EU is waiting on this
                    // byte rather than reading its way through a queue.
                    if depth <= 0 {
                        waited.push(i);
                    }
                    depth += 1;
                }
            }
            for t4 in waited {
                if let Some(&(row, op)) = reads.iter().find(|&&(r, _)| r > t4) {
                    let kind = match op {
                        QueueOp::First => "First     ",
                        _ => "Subsequent",
                    };
                    *hist
                        .entry((queue, kind))
                        .or_default()
                        .entry(row - t4)
                        .or_default() += 1;
                }
            }
        }
    }

    eprintln!("\nT-states from a code fetch's T4 to the EU taking that byte,");
    eprintln!("over fetches the EU was waiting on, whole corpus, reads corrected");
    eprintln!("for the one-T-state queue-status reporting delay\n");
    eprintln!("  initial  read as       | 1        2        3        4+       cases");
    for ((queue, kind), h) in &hist {
        let total: usize = h.values().sum();
        let at = |d: usize| -> String {
            let n: usize = h
                .iter()
                .filter(|&(&k, _)| if d == 4 { k >= 4 } else { k == d })
                .map(|(_, n)| *n)
                .sum();
            format!("{:.1}%", 100.0 * n as f64 / total as f64)
        };
        eprintln!(
            "  {queue}    {kind}   | {:<8} {:<8} {:<8} {:<8} {total}",
            at(1),
            at(2),
            at(3),
            at(4),
        );
    }
}

/// Replay a case and report the T-states between successive bus cycles, tagged
/// by kind, over the gate's span.
fn replay_bus_gaps(tc: &I8088TestCase) -> Option<Vec<(char, usize)>> {
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

    let mut starts: Vec<(char, usize)> = Vec::new();
    let mut measuring = false;
    let mut retired = false;
    for i in 0..4000usize {
        let was_retired = retired;
        retired |= cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
        let next = matches!(cpu.queue_status, Some((QueueStatus::First, _))) && was_retired;
        if !measuring {
            if cpu.queue_status.is_none() {
                continue;
            }
            measuring = true;
        } else if next {
            let first = starts.first().map(|&(_, t)| t).unwrap_or(0);
            return Some(
                starts
                    .iter()
                    .scan(first, |prev, &(k, t)| {
                        let gap = t - *prev;
                        *prev = t;
                        Some((k, gap))
                    })
                    .collect(),
            );
        }
        if cpu.bus.address.is_some() {
            let kind = match cpu.bus.status {
                phosphor_core::cpu::i8088::BusStatus::Code => 'F',
                phosphor_core::cpu::i8088::BusStatus::MemRead => 'R',
                phosphor_core::cpu::i8088::BusStatus::MemWrite => 'W',
                _ => 'O',
            };
            starts.push((kind, i));
        }
    }
    None
}

/// Where do this core's bus cycles start, against the part's, measured as the
/// T-states between one and the next?
///
/// **Gaps between bus events only**, so the queue-status reporting delay cannot
/// enter: both columns are read off the same kind of signal. The bus-cycle gate
/// compares the *sequence* of transactions and so says nothing about when they
/// happen; this says exactly that, and it is what the prefetch decision point
/// has to be settled against.
///
/// `F` is a code fetch, `R` and `W` the operand's, `O` an I/O cycle. A chained
/// fetch shows as a gap of 4; anything longer is the BIU having waited.
#[test]
#[ignore = "survey, not a check: when our bus cycles start against the part's"]
fn fetch_gap_diff() {
    let mut rows: Vec<(String, String, String, usize, usize)> = Vec::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        let mut pairs: BTreeMap<(String, String), usize> = BTreeMap::new();
        let mut cases = 0usize;
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let theirs: Vec<(char, usize)> = {
                let starts: Vec<(char, usize)> = tc
                    .cycles
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        c.address().map(|_| {
                            let k = match c.status() {
                                phosphor_cpu_validation::BusStatus::CODE => 'F',
                                phosphor_cpu_validation::BusStatus::MEMR => 'R',
                                phosphor_cpu_validation::BusStatus::MEMW => 'W',
                                _ => 'O',
                            };
                            (k, i)
                        })
                    })
                    .collect();
                let first = starts.first().map(|&(_, t)| t).unwrap_or(0);
                let mut prev = first;
                starts
                    .iter()
                    .map(|&(k, t)| {
                        let gap = t - prev;
                        prev = t;
                        (k, gap)
                    })
                    .collect()
            };
            let Some(ours) = replay_bus_gaps(tc) else {
                continue;
            };
            cases += 1;
            if ours == theirs {
                continue;
            }
            let render = |g: &[(char, usize)]| {
                g.iter()
                    .map(|(k, n)| format!("{k}{n}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            *pairs.entry((render(&ours), render(&theirs))).or_default() += 1;
        }
        let wrong: usize = pairs.values().sum();
        if wrong == 0 {
            continue;
        }
        let ((ours, theirs), n) = pairs
            .into_iter()
            .max_by_key(|&(_, n)| n)
            .expect("non-empty");
        let _ = cases;
        rows.push((stem, ours, theirs, n, wrong));
    }

    rows.sort_by_key(|r| std::cmp::Reverse(r.4));
    let total: usize = rows.iter().map(|r| r.4).sum();
    eprintln!("\nwhen our bus cycles start, against the part's, as gaps between them");
    eprintln!(
        "  {} files differ somewhere, {total} cases in all",
        rows.len()
    );
    eprintln!("\n  file     ours                      theirs");
    for (stem, ours, theirs, n, wrong) in rows.iter().take(30) {
        eprintln!("  {stem:7}  {ours:<24}  {theirs:<24}  {n} of {wrong}");
    }
}

/// How long after the ModR/M byte does the part read the displacement, per
/// opcode?
///
/// [`loader_read_gap_diff`] says this gap is four T-states across 148 files,
/// and then that `8F` wants six and `8C` wants seven. So it is a table and not
/// a constant, and this is the table: the gap measured directly, for every
/// opcode that has one, with the share of cases agreeing so a row that is
/// really two groups cannot pass as uniform.
///
/// Restricted to memory forms carrying a displacement, a full queue and no
/// prefix, so the byte is already in hand and the gap is decode rather than a
/// wait on a fetch.
#[test]
#[ignore = "survey, not a check: the ModR/M to displacement gap, per opcode"]
fn modrm_to_displacement_gap() {
    // Keyed by addressing mode and pooled over every opcode that has one: the
    // per-opcode cut showed 4, 6 and 7 in thirds on almost every file, which is
    // the signature of a split this key does not carry.
    let mut by_mode: BTreeMap<(u8, u8), BTreeMap<usize, usize>> = BTreeMap::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        let mut gaps: BTreeMap<usize, usize> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let Some(&modrm) = tc.bytes.get(1) else {
                continue;
            };
            if !format_has_modrm(tc.bytes[0]) {
                continue;
            }
            // A displacement is what the gap runs to. Without one the next read
            // is an immediate or the next instruction, which is a different
            // question.
            let disp = match (modrm >> 6, modrm & 7) {
                (0, 6) | (2, _) => 2,
                (1, _) => 1,
                _ => 0,
            };
            if disp == 0 {
                continue;
            }
            let reads: Vec<usize> = tc
                .cycles
                .iter()
                .enumerate()
                .filter_map(|(i, c)| match c.queue_op().map(|(op, _)| op) {
                    Some(QueueOp::First | QueueOp::Subsequent) => Some(i),
                    _ => None,
                })
                .collect();
            if reads.len() < 3 {
                continue;
            }
            let gap = reads[2] - reads[1];
            *gaps.entry(gap).or_default() += 1;
            *by_mode
                .entry((modrm >> 6, modrm & 7))
                .or_default()
                .entry(gap)
                .or_default() += 1;
        }
        let _ = gaps;
    }

    eprintln!("\nT-states from the ModR/M byte to the displacement, by addressing");
    eprintln!("mode, pooled over every opcode, full queue and no prefix\n");
    eprintln!("  mode      gap   cases          spread");
    for ((m, rm), gaps) in &by_mode {
        let cases: usize = gaps.values().sum();
        let (&modal, &n) = gaps.iter().max_by_key(|&(_, n)| *n).expect("non-empty");
        let mut spread: Vec<(usize, usize)> = gaps.iter().map(|(g, n)| (*g, *n)).collect();
        spread.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        spread.truncate(3);
        let s: Vec<String> = spread.iter().map(|(g, n)| format!("{g}:{n}")).collect();
        eprintln!(
            "  mod={m} rm={rm}  {modal:<4}  {n} of {cases:<6}  {}{}",
            s.join(" "),
            if n == cases { "   uniform" } else { "" }
        );
    }
}

/// Replay a case and report the same gaps [`loader_read_pattern`] takes from
/// the recording: the T-states between successive reads of this instruction's
/// bytes.
///
/// **Gaps, and not positions, is the whole point.** The recorded queue-status
/// lines are reported one T-state late and the bus columns beside them are not,
/// so any measurement mixing a read row with a bus row has to guess at that
/// offset, and two of this epic's dead ends are exactly that guess made
/// differently. A constant delay cancels in a difference between two reads, so
/// this comparison has no offset to get wrong.
fn replay_read_gaps(tc: &I8088TestCase) -> Option<Vec<usize>> {
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

    let mut reads: Vec<usize> = Vec::new();
    let mut ticks = 0usize;
    let mut measuring = false;
    for i in 0..4000 {
        ticks += 1;
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
        let read = matches!(
            cpu.queue_status,
            Some((QueueStatus::First | QueueStatus::Subsequent, _))
        );
        if !measuring {
            if !read {
                continue;
            }
            measuring = true;
        }
        if read {
            reads.push(i);
            if reads.len() > tc.bytes.len() {
                break;
            }
        }
    }
    if ticks >= 4000 || reads.len() < tc.bytes.len() {
        return None;
    }
    Some(
        reads[..tc.bytes.len()]
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect(),
    )
}

/// Where does this core read an instruction's bytes at a different rhythm from
/// the part, and at which byte?
///
/// The decisive question left on the four-byte forms is *which* clock is
/// missing: the one where the fetcher sets out, or the one where the loader
/// takes the byte home. Positions cannot answer it without assuming the
/// reporting offset, and assuming it wrongly is what produced two confident
/// dead ends. Gaps can, because the offset cancels.
///
/// A difference in an early gap is decode: this core is reading a byte it
/// already holds at the wrong rhythm. A difference in the *last* gap, the one
/// that follows a queue the instruction drained, is the refill: the byte came
/// home at a different time or was taken up at a different time. The two are
/// different repairs and this says which is needed, per opcode, over the whole
/// corpus.
#[test]
#[ignore = "survey, not a check: our read rhythm against the part's, per opcode"]
fn loader_read_gap_diff() {
    let mut rows: Vec<(String, String, String, usize, usize)> = Vec::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        let mut pairs: BTreeMap<(String, String), usize> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 || tc.bytes.len() < 2 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let theirs: Vec<usize> = {
                let reads: Vec<usize> = tc
                    .cycles
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| match c.queue_op().map(|(op, _)| op) {
                        Some(QueueOp::First | QueueOp::Subsequent) => Some(i),
                        _ => None,
                    })
                    .collect();
                if reads.len() < tc.bytes.len() {
                    continue;
                }
                reads[..tc.bytes.len()]
                    .windows(2)
                    .map(|w| w[1] - w[0])
                    .collect()
            };
            let Some(ours) = replay_read_gaps(tc) else {
                continue;
            };
            if ours == theirs {
                continue;
            }
            let render = |g: &[usize]| {
                g.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            *pairs.entry((render(&ours), render(&theirs))).or_default() += 1;
        }
        let wrong: usize = pairs.values().sum();
        if wrong == 0 {
            continue;
        }
        let ((ours, theirs), n) = pairs
            .into_iter()
            .max_by_key(|&(_, n)| n)
            .expect("non-empty");
        rows.push((stem, ours, theirs, n, wrong));
    }

    rows.sort_by_key(|r| std::cmp::Reverse(r.4));
    let total: usize = rows.iter().map(|r| r.4).sum();
    eprintln!("\nour read rhythm against the part's, full queue and no prefix");
    eprintln!(
        "  {} files differ somewhere, {total} cases in all",
        rows.len()
    );
    eprintln!("\n  file     ours            theirs          commonest of the file's wrong");
    for (stem, ours, theirs, n, wrong) in &rows {
        eprintln!("  {stem:7}  {ours:<15} {theirs:<15} {n} of {wrong}");
    }
}

/// How fast does the part's loader pull an instruction's bytes out of the
/// queue, byte by byte?
///
/// This core's loader takes one byte per T-state whenever the queue has one.
/// The part's does not. `EA` reads its opcode and then **stalls a whole T-state
/// before taking the next byte**; `F7` stalls after its ModR/M byte instead;
/// `81` does not stall at all. Three traces, three different patterns, and this
/// core runs all three flat out.
///
/// That is not a detail of three opcodes. The stall decides when the queue
/// drains, which decides when the refill behind it lands, which decides where
/// the span ends. It is why a four-byte register form is a clock short while
/// the five-byte far transfers beside it are exact, and why every constant
/// tried against that gap either moved nothing or broke the other one.
///
/// Reported as the gaps between successive reads of one instruction, so `1 1 1`
/// is flat out and `1 2 1` is a stall in the middle. Restricted to the full
/// queue and no prefix, where every byte is in hand before the instruction
/// starts and nothing in the pattern can be waiting on a fetch.
#[test]
#[ignore = "survey, not a check: where the part's loader stalls, per opcode"]
fn loader_read_pattern() {
    let mut rows: Vec<(String, String, usize, usize)> = Vec::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        // Split register forms from memory ones. A memory form defers its
        // immediate and waits on fetches, so its gaps are not all decode and its
        // patterns scatter; pooling the two hides the register pattern under
        // whichever memory one happens to be commonest.
        let mut patterns: BTreeMap<(bool, String), usize> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let register_form = match tc.bytes.get(1) {
                Some(&modrm) if format_has_modrm(tc.bytes[0]) => modrm >> 6 == 3,
                _ => false,
            };
            let reads: Vec<usize> = tc
                .cycles
                .iter()
                .enumerate()
                .filter_map(|(i, c)| match c.queue_op().map(|(op, _)| op) {
                    Some(QueueOp::First | QueueOp::Subsequent) => Some(i),
                    _ => None,
                })
                .collect();
            // Only the bytes of this instruction: a read past its length
            // belongs to whatever the part started next.
            if reads.len() < tc.bytes.len() {
                continue;
            }
            let gaps: Vec<String> = reads[..tc.bytes.len()]
                .windows(2)
                .map(|w| (w[1] - w[0]).to_string())
                .collect();
            *patterns.entry((register_form, gaps.join(" "))).or_default() += 1;
        }
        for reg in [false, true] {
            let group: Vec<(&String, usize)> = patterns
                .iter()
                .filter(|((r, _), _)| *r == reg)
                .map(|((_, p), n)| (p, *n))
                .collect();
            let cases: usize = group.iter().map(|&(_, n)| n).sum();
            if cases == 0 {
                continue;
            }
            let (pattern, n) = group
                .iter()
                .max_by_key(|&&(_, n)| n)
                .map(|&(p, n)| (p.clone(), n))
                .expect("non-empty");
            let label = if reg {
                format!("{stem} reg")
            } else {
                stem.clone()
            };
            rows.push((label, pattern, n, cases));
        }
    }

    eprintln!("\ngaps between successive reads of one instruction's bytes,");
    eprintln!("full queue and no prefix, modal pattern per opcode file\n");
    let flat = |p: &str| p.split_whitespace().all(|g| g == "1");
    let stalled: Vec<&(String, String, usize, usize)> =
        rows.iter().filter(|(_, p, _, _)| !flat(p)).collect();
    eprintln!(
        "  {} of {} files stall somewhere; {} run flat out",
        stalled.len(),
        rows.len(),
        rows.len() - stalled.len()
    );
    eprintln!("\n  file    pattern              share");
    for (stem, pattern, n, cases) in stalled {
        eprintln!(
            "  {stem:6}  {pattern:<20} {n} of {cases} ({:.1}%)",
            100.0 * *n as f64 / *cases as f64
        );
    }
}

/// Does the BIU-yields hypothesis hold over the population, or only over the
/// one trace it came from?
///
/// Two questions at once, because the second is what says whether the first is
/// the whole story:
///
/// 1. **How many empty-queue cases start one more code fetch before their first
///    data cycle than the part does?** That is the hypothesis stated as a
///    countable thing.
/// 2. **What is the residual on empty-queue cases that touch no memory at
///    all?** Those cannot be affected by an arbitration rule between the EU and
///    the BIU, so whatever is wrong with them is a second cause, and the
///    empty-queue half is more than half wrong.
#[test]
#[ignore = "survey, not a check: does the BIU-yields hypothesis hold at scale"]
fn empty_queue_population() {
    let mut gap: BTreeMap<i64, usize> = BTreeMap::new();
    let mut residual_by_kind: BTreeMap<bool, BTreeMap<i64, usize>> = BTreeMap::new();
    let mut files = 0usize;
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        files += 1;
        for tc in &tests {
            if tc.cycles.is_empty() || !tc.initial.queue.is_empty() {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let Some(ours) = replay(tc) else { continue };
            let touches_memory = recorded_fetches_before_data(tc).is_some();
            *residual_by_kind
                .entry(touches_memory)
                .or_default()
                .entry(ours as i64 - tc.cycles.len() as i64)
                .or_default() += 1;
            if let (Some(a), Some(b)) = (
                replay_fetches_before_data(tc),
                recorded_fetches_before_data(tc),
            ) {
                *gap.entry(a as i64 - b as i64).or_default() += 1;
            }
        }
    }

    eprintln!("\nempty queue, no prefix, over {files} files");
    for (touches, hist) in &residual_by_kind {
        let total: usize = hist.values().sum();
        let exact = hist.get(&0).copied().unwrap_or(0);
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(6)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!(
            "  {}: {exact} of {total} exact ({:.2}%)  {}",
            if *touches {
                "touches memory   "
            } else {
                "touches no memory"
            },
            100.0 * exact as f64 / total as f64,
            top.join(" ")
        );
    }
    let total: usize = gap.values().sum();
    eprintln!("\n  code fetches before the first data cycle, ours minus the part's:");
    for (d, n) in &gap {
        eprintln!("    {d:+}: {n}  ({:.1}%)", 100.0 * *n as f64 / total as f64);
    }
}

/// The gap grouped by what plausibly decides it: how long the address phase
/// runs, and how many bytes the instruction is.
///
/// The population count says the over-fetching is real for half the cases,
/// absent for 42.5% and reversed for 6.3%, which is not one rule. If what
/// separates them is the size of the window the address phase leaves for
/// fetching, then the effective address's own cost and the instruction's length
/// are the keys, and each group here is uniform.
#[test]
#[ignore = "survey, not a check: what separates the over-fetching cases"]
fn empty_queue_fetch_gap_by_shape() {
    // A load, a store, a read-modify-write and one with an immediate, so the
    // operand's direction and the instruction's length both vary.
    for stem in ["8B", "89", "01", "81.0"] {
        let Some(tests) = load(stem) else { continue };
        let mut groups: BTreeMap<(u8, u8), BTreeMap<i64, usize>> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || !tc.initial.queue.is_empty() {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let Some(&modrm) = tc.bytes.get(1) else {
                continue;
            };
            if modrm >> 6 == 3 {
                continue;
            }
            let (Some(a), Some(b)) = (
                replay_fetches_before_data(tc),
                recorded_fetches_before_data(tc),
            ) else {
                continue;
            };
            *groups
                .entry((modrm >> 6, modrm & 7))
                .or_default()
                .entry(a as i64 - b as i64)
                .or_default() += 1;
        }
        eprintln!("\n{stem}: fetches before the first data cycle, ours minus the part's, by mode");
        for ((m, rm), hist) in &groups {
            let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
            modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            let spread = modes.len() > 1;
            let top: Vec<String> = modes
                .iter()
                .take(3)
                .map(|(d, n)| format!("{d:+}:{n}"))
                .collect();
            eprintln!(
                "  mod={m} rm={rm}  ea={:2}  {}{}",
                ea_cycles(m << 6 | rm, false),
                top.join(" "),
                if spread { "   <-- SPREAD" } else { "" }
            );
        }
    }
}

/// The same instrument pointed at the **empty-queue** half, the largest
/// unexplained population in the epic: 44.92% on cycle count against the
/// prefetched half's 84.55%, about 828,000 vectors.
///
/// The one earlier attempt compared our span index against the recording's raw
/// trace index, which are different origins for a case that starts with an
/// empty queue, and produced a confident and meaningless answer. This does not
/// have that problem: both columns start on the cycle the opening First Byte is
/// read, because that is where the recorded trace begins and where this replay
/// starts measuring.
///
/// A memory form first, then the stack pair, so the queue and the operand path
/// are varied against each other.
#[test]
#[ignore = "survey, not a check: where an empty-queue case diverges"]
fn side_by_side_from_an_empty_queue() {
    dump_side_by_side("8B", true, 0);
    // Every push is +1 and every pop is -1 from an empty queue, uniformly over
    // all 5000 cases of sixteen opcode files, where the full-queue population
    // wants the opposite for the pushes. A row cannot be both, so the stack
    // path has an empty-queue error of its own.
    dump_side_by_side("50", false, 0);
    dump_side_by_side("58", false, 0);
    // And the four-byte register form that is `-1` on every full-queue case and
    // *exact* on every empty-queue one. The same instruction and the same
    // boundary, so whatever the part does differently is visible here beside
    // the full-queue trace in [`side_by_side`].
    dump_side_by_side("81.0", false, 0);
    // The port pair, which is the largest uniform block left in this
    // population: every `IN` is -1 and every `OUT` is +1, on all 5000 cases of
    // each of eight files. Symmetric, so it is where the port cycle sits rather
    // than what it costs.
    dump_side_by_side("E4", false, 0);
    dump_side_by_side("E6", false, 0);
}

#[test]
#[ignore = "survey, not a check: where the loader's first fetch lands"]
fn code_fetch_start() {
    // A one-byte opcode, a two-byte one with an immediate, a memory form, and a
    // long one, so the instruction's own length is varied against the queue's.
    code_start_by_queue_length("90");
    code_start_by_queue_length("04");
    code_start_by_queue_length("8B");
    code_start_by_queue_length("81.0");
}

#[test]
#[ignore = "survey, not a check: where the operand's bus cycle starts"]
fn operand_start() {
    operand_start_by_mode("8B");
    operand_start_by_mode("89");
    // And the forms that carry an immediate as well as a displacement, where
    // the part fetches the immediate after the operand read and this core
    // fetches it before.
    operand_start_by_mode("81.0");
    operand_start_by_mode("C7");
}

/// How far this core is from the recording on one opcode file, as a histogram
/// of signed differences over the cases that begin with a full queue.
///
/// Empty when the file holds no case that begins with a full queue and no
/// prefix, which is the population every survey here restricts itself to.
fn residual_histogram(stem: &str) -> BTreeMap<i64, usize> {
    let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
    let Some(tests) = load(stem) else { return hist };
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
    hist
}

/// Every opcode file the suite ships, by stem, in the order the directory
/// lists them.
fn every_opcode_file() -> Vec<String> {
    let dir = phosphor_cpu_validation::vector_dir("8088/v2");
    if !phosphor_cpu_validation::require_test_data(&dir, "vectors") {
        return Vec::new();
    }
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("read")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "gz"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries
        .iter()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".json.gz").unwrap_or(&name).to_string()
        })
        .collect()
}

/// Replay `f` over every file at once, returning the results in input order.
///
/// **These instruments are dominated by loading, not by replaying.** Each of
/// the 323 files is a gzip stream that has to be decompressed and parsed as
/// JSON before a single T-state runs, and that work is per file and shares
/// nothing. Spreading it over the machine takes the whole-corpus survey from
/// about seventy seconds to a few, which matters because it is the instrument
/// run between every change.
///
/// Results come back in input order however the work was scheduled, so a
/// failure list stays diffable between runs, the same guarantee the harness's
/// registry sweeps make. `PHOSPHOR_TEST_THREADS=1` forces it back to sequential
/// for bisecting.
fn map_files<R, F>(stems: &[String], f: F) -> Vec<R>
where
    R: Send,
    F: Fn(&str) -> R + Sync,
{
    let threads = survey_threads().min(stems.len().max(1));
    if threads <= 1 {
        return stems.iter().map(|s| f(s)).collect();
    }

    // Files differ in size by a factor of twenty, so the work is handed out one
    // at a time rather than in contiguous blocks: a thread that draws the small
    // files comes back for more instead of finishing early.
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let next = &next;
                let f = &f;
                scope.spawn(move || {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(stem) = stems.get(i) else { break };
                        mine.push((i, f(stem)));
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a survey thread panicked"))
            .collect()
    });
    done.sort_by_key(|&(i, _)| i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// How many threads a whole-corpus sweep may use.
fn survey_threads() -> usize {
    match std::env::var("PHOSPHOR_TEST_THREADS") {
        Ok(v) => v.parse().unwrap_or(1).max(1),
        Err(_) => std::thread::available_parallelism().map_or(1, |n| n.get()),
    }
}

/// The same residual, split by addressing mode, so that a memory-operand
/// instruction's error can be attributed to a component of its address rather
/// than to the instruction.
fn residuals_by_mode(stem: &str) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(u8, u8), BTreeMap<i64, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some(&modrm) = tc.bytes.get(1) else {
            continue;
        };
        let Some(ours) = replay(tc) else { continue };
        *groups
            .entry((modrm >> 6, modrm & 7))
            .or_default()
            .entry(ours as i64 - tc.cycles.len() as i64)
            .or_default() += 1;
    }
    eprintln!("\n{stem}: residual by addressing mode");
    for ((m, rm), hist) in &groups {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(3)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!("  mod={m} rm={rm}  {}", top.join(" "));
    }
}

#[test]
#[ignore = "survey, not a check: which addressing modes carry the residual"]
fn memory_residual_by_mode() {
    // The two worst rows in the sweep, wrong on every case: the far transfers
    // through a memory pointer, which read a four-byte pointer and then flush.
    residuals_by_mode("FF.5");
    residuals_by_mode("FF.3");
    // A load, a store, a read-modify-write, and one with an immediate, so that
    // the operand's direction and the instruction's length are both varied.
    residuals_by_mode("8B");
    residuals_by_mode("89");
    residuals_by_mode("01");
    residuals_by_mode("81.0");
    // The two halves of the one-clock memory tail, and the store that refutes
    // the tidy explanation of the second.
    //
    // `NEG` in memory runs a clock long, and the side-by-side trace says why:
    // the part reads the next instruction's First Byte on the same cycle as the
    // final write's T4. `MOV [mem], reg` ends in a write too and is `+0` on
    // most of its cases, so the overlap is conditional on something. If that
    // something is the addressing mode, it is visible here.
    residuals_by_mode("F7.3");
    residuals_by_mode("F7.4");
    residuals_by_mode("88");
    // The rest of the family the ranked sweep gives the same shape to. Checking
    // them rather than assuming is the whole discipline: `F7.3` and `F7.4` are
    // uniform across all 24 memory modes, and a row that is uniform is a wrong
    // constant, but a row that only *looks* like them in the aggregate could be
    // two modes cancelling.
    for stem in [
        "F6.2", "F6.3", "F7.2", "F6.4", "F6.5", "F6.6", "F6.7", "F7.5", "F7.6", "F7.7",
    ] {
        residuals_by_mode(stem);
    }
    // And the CMP-immediate rows, which do not write back and carry the same
    // `-1` as the multiplies. `81.7` is the odd one: the sweep says it is wrong
    // on its register forms too, unlike the other three.
    for stem in ["80.7", "81.7", "82.7", "83.7"] {
        residuals_by_mode(stem);
    }
    // The coprocessor escapes, which this core does not perform the operand
    // read for at all. The recording shows the part reading a *word* and
    // discarding it, which is what lets a coprocessor snoop the bus. Their
    // residual is two-valued, -11 and -10, and if that is the even-address
    // rounding then it separates by addressing mode.
    residuals_by_mode("D8");
    residuals_by_mode("D9");
    // The next rows down the ranked sweep.
    for stem in ["8C", "8F", "C6", "C7", "FF.2", "FF.4", "D6"] {
        residuals_by_mode(stem);
    }
}

/// Does the suite record an interrupt acknowledge anywhere, or an asserted
/// INTR or NMI pin? M4 asks for INTA cycles, and whether the vectors can check
/// them decides whether they are modeled against a recording or against the
/// manual.
#[test]
#[ignore = "survey, not a check: what the suite records about interrupts"]
fn interrupt_pins_and_acknowledge() {
    let dir = phosphor_cpu_validation::vector_dir("8088/v2");
    if !phosphor_cpu_validation::require_test_data(&dir, "vectors") {
        return;
    }
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("read")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "gz"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut files_with_inta = Vec::new();
    let mut files_with_intr = Vec::new();
    let mut files_with_nmi = Vec::new();
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name.strip_suffix(".json.gz").unwrap_or(&name).to_string();
        let Some(tests) = load(&stem) else { continue };
        for tc in &tests {
            for c in &tc.cycles {
                if c.status() == phosphor_cpu_validation::BusStatus::INTA
                    && !files_with_inta.contains(&stem)
                {
                    files_with_inta.push(stem.clone());
                }
                if c.0 & 2 != 0 && !files_with_intr.contains(&stem) {
                    files_with_intr.push(stem.clone());
                }
                if c.0 & 4 != 0 && !files_with_nmi.contains(&stem) {
                    files_with_nmi.push(stem.clone());
                }
            }
        }
    }
    eprintln!("\nfiles whose traces contain an INTA cycle: {files_with_inta:?}");
    eprintln!("files with INTR asserted on any cycle:      {files_with_intr:?}");
    eprintln!("files with NMI asserted on any cycle:       {files_with_nmi:?}");
}

/// The signed multiply's operands, pulled out of one recorded case: the
/// multiplicand from the ModR/M byte's `r/m` field and the multiplier from the
/// accumulator, both sign-extended so the byte and word forms can share a
/// survey.
fn signed_multiply_operands(tc: &I8088TestCase, word: bool) -> Option<(i32, i32)> {
    let modrm = *tc.bytes.get(1)?;
    if modrm >> 6 != 3 {
        return None;
    }
    let r = &tc.initial.regs;
    let multiplicand = if word {
        i32::from(match modrm & 7 {
            0 => r.ax,
            1 => r.cx,
            2 => r.dx,
            3 => r.bx,
            4 => r.sp,
            5 => r.bp,
            6 => r.si,
            _ => r.di,
        } as i16)
    } else {
        let regs = [
            r.ax as u8,
            r.cx as u8,
            r.dx as u8,
            r.bx as u8,
            (r.ax >> 8) as u8,
            (r.cx >> 8) as u8,
            (r.dx >> 8) as u8,
            (r.bx >> 8) as u8,
        ];
        i32::from(regs[(modrm & 7) as usize] as i8)
    };
    let multiplier = if word {
        i32::from(r.ax as i16)
    } else {
        i32::from(r.ax as u8 as i8)
    };
    Some((multiplicand, multiplier))
}

/// `IMUL`, grouped the way its microcode is shaped: the signs it has to correct
/// for, and the set bits of the multiplier its loop walks.
///
/// The span has `popcount(|multiplier|)` subtracted off, so what is printed is
/// the part of the cost the loop does not explain. A sign combination whose
/// remainder is a single value is fully explained; one that splits has a term
/// left in it, which is the shape `AAM` and `DIV` were in before the quotient's
/// low bit was found.
///
/// `key` is the candidate for that term. Each is a guess at a branch the
/// microcode takes, and a right one makes every group uniform.
fn signed_multiply_shape_by(stem: &str, word: bool, label: &str, key: fn(i32, i32, bool) -> bool) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(bool, bool, bool), BTreeMap<i64, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some((multiplicand, multiplier)) = signed_multiply_operands(tc, word) else {
            continue;
        };
        let bits = multiplier.unsigned_abs().count_ones() as i64;
        *groups
            .entry((
                multiplicand < 0,
                multiplier < 0,
                key(multiplicand, multiplier, word),
            ))
            .or_default()
            .entry(tc.cycles.len() as i64 - bits)
            .or_default() += 1;
    }
    eprintln!("\n{stem}: span less popcount(|multiplier|), by signs and {label}");
    for ((mcand, mplier, k), hist) in &groups {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let spread = modes.len() > 1;
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!(
            "  mcand{} mplier{} {k:5}: {}{}",
            if *mcand { "-" } else { "+" },
            if *mplier { "-" } else { "+" },
            top.join(" "),
            if spread { "   <-- SPREAD" } else { "" }
        );
    }
}

#[test]
#[ignore = "survey, not a check: what shape IMUL's timing has"]
fn signed_multiply_shape() {
    for (stem, word) in [("F6.5", false), ("F7.5", true)] {
        // Nothing: the four sign combinations on their own, which is where the
        // previous pass stopped.
        signed_multiply_shape_by(stem, word, "nothing", |_, _, _| false);
        // `MUL`'s own extra term: the product's upper half is zero, which is
        // the branch that sets carry and overflow.
        signed_multiply_shape_by(
            stem,
            word,
            "the product's upper half is zero",
            |a, b, word| {
                let shift = if word { 16 } else { 8 };
                (a * b) >> shift == 0
            },
        );
        // The signed form of the same test: the upper half is the sign
        // extension of the lower, which is what `IMUL` sets its flags on.
        signed_multiply_shape_by(
            stem,
            word,
            "the product sign-extends into its upper half",
            |a, b, word| {
                let shift = if word { 16 } else { 8 };
                let p = a * b;
                (p >> shift) == (p << (32 - shift)) >> 31
            },
        );
        // A multiplier of zero never enters the loop at all.
        signed_multiply_shape_by(stem, word, "the multiplier is zero", |_, b, _| b == 0);
    }
}

/// `IDIV`, asked the question `DIV` was asked: does the recorded span follow
/// the `CORD` loop's compared subtracts and the quotient's low bit, once the
/// operands are made positive the way `PREIDIV` makes them?
///
/// Printed as the span less those two terms, grouped by the two signs, so a
/// group that is one value is fully explained and the four values are the
/// sign-correction costs. The faulting cases are grouped separately: `CORD`
/// leaves for `INT 0` before the loop, so nothing about the loop applies.
fn signed_divide_shape(stem: &str, word: bool) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<(bool, bool), BTreeMap<i64, Vec<String>>> = BTreeMap::new();
    let mut late_faults: BTreeMap<(bool, bool), BTreeMap<i64, Vec<String>>> = BTreeMap::new();
    let mut faults: BTreeMap<(bool, bool), BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some((divisor, _)) = signed_multiply_operands(tc, word) else {
            continue;
        };
        let r = &tc.initial.regs;
        let dividend = if word {
            ((i64::from(r.dx) << 16) | i64::from(r.ax)) as i32 as i64
        } else {
            i64::from(r.ax as i16)
        };
        let divisor = i64::from(divisor);
        // `CORD` checks before it loops, on the magnitudes `PREIDIV` leaves it,
        // and leaves for `INT 0` at once when the quotient would not fit the
        // *unsigned* width. That is the fault `DIV` has, and it costs the same
        // whatever caused it.
        let width = if word { 16 } else { 8 };
        if divisor == 0 || dividend.unsigned_abs() >> width >= divisor.unsigned_abs() {
            *faults
                .entry((dividend < 0, divisor < 0))
                .or_default()
                .entry(tc.cycles.len())
                .or_default() += 1;
            continue;
        }
        // The loop runs on the magnitudes, which is what PREIDIV leaves it.
        let compared = i64::from(cord_compared(
            dividend.unsigned_abs() as u32,
            divisor.unsigned_abs() as u32,
            word,
        ));
        let magnitude = dividend.unsigned_abs() / divisor.unsigned_abs();
        let odd = magnitude & 1 != 0;
        let residual = tc.cycles.len() as i64 - compared - 2 * i64::from(odd);
        // The signed range is narrower than the unsigned one by a bit, so a
        // quotient between the two runs the whole loop and only then faults.
        // Those are a second population, not outliers.
        if magnitude > (1 << (width - 1)) - 1 {
            late_faults
                .entry((dividend < 0, divisor < 0))
                .or_default()
                .entry(residual)
                .or_default()
                .push(tc.name.clone());
            continue;
        }
        groups
            .entry((dividend < 0, divisor < 0))
            .or_default()
            .entry(residual)
            .or_default()
            .push(tc.name.clone());
    }
    eprintln!("\n{stem}: span less compared subtracts and the quotient's low bit, by signs");
    for ((dividend, divisor), hist) in &groups {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, b.len())).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let spread = modes.len() > 1;
        let top: Vec<String> = modes
            .iter()
            .take(5)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!(
            "  dividend{} divisor{}: {}{}",
            if *dividend { "-" } else { "+" },
            if *divisor { "-" } else { "+" },
            top.join(" "),
            if spread { "   <-- SPREAD" } else { "" }
        );
        // Name the cases in any group of one or two, which is what an outlier
        // looks like here: the rule holds over hundreds and a handful sit well
        // off it, and the only way to tell a broken rule from a miscategorized
        // case is to read the case.
        for (value, names) in hist.iter().filter(|(_, names)| names.len() <= 2) {
            eprintln!("      {value}: {}", names.join(", "));
        }
    }
    eprintln!("  and the quotients that fit unsigned but not signed:");
    for ((dividend, divisor), hist) in &late_faults {
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, b.len())).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let spread = modes.len() > 1;
        let top: Vec<String> = modes
            .iter()
            .take(5)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!(
            "    dividend{} divisor{}: {}{}",
            if *dividend { "-" } else { "+" },
            if *divisor { "-" } else { "+" },
            top.join(" "),
            if spread { "   <-- SPREAD" } else { "" }
        );
    }
    for ((dividend, divisor), hist) in &faults {
        let mut modes: Vec<(usize, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!(
            "  fault, dividend{} divisor{}: spans {}",
            if *dividend { "-" } else { "+" },
            if *divisor { "-" } else { "+" },
            top.join(" ")
        );
    }
}

#[test]
#[ignore = "survey, not a check: what shape IDIV's timing has"]
fn signed_divide_shape_survey() {
    signed_divide_shape("F6.7", false);
    signed_divide_shape("F7.7", true);
}

/// The residual for the *repeated* string operations, which every other survey
/// here filters out along with the rest of the prefixed cases.
///
/// Grouped by how many iterations the recording ran, because a per-iteration
/// error and a fixed one look the same on a single count and completely
/// different across a range: a row that is one clock out per iteration shows a
/// residual that grows with the count, and a wrong `REP` setup shows the same
/// residual at every count.
#[test]
#[ignore = "survey, not a check: the repeated string operations"]
fn rep_residuals() {
    eprintln!("\nrepeated string operations, residual by iteration count");
    for stem in ["A4", "A5", "A6", "A7", "AA", "AB", "AC", "AD", "AE", "AF"] {
        let Some(tests) = load(stem) else { continue };
        let mut groups: BTreeMap<u16, BTreeMap<i64, usize>> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            // Exactly one REP prefix and nothing else in front of the opcode.
            if !matches!(tc.bytes.first(), Some(0xF2 | 0xF3))
                || tc.bytes.get(1).is_some_and(|&b| is_prefix(b))
            {
                continue;
            }
            let Some(ours) = replay(tc) else { continue };
            // How many iterations ran: CX before, less CX after.
            let before = tc.initial.regs.cx;
            let after = tc.final_state.regs.cx.unwrap_or(before);
            *groups
                .entry(before.wrapping_sub(after))
                .or_default()
                .entry(ours as i64 - tc.cycles.len() as i64)
                .or_default() += 1;
        }
        let shown: Vec<String> = groups
            .iter()
            .take(6)
            .map(|(iterations, hist)| {
                let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
                modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
                let (d, n) = modes[0];
                format!("{iterations} iters {d:+} ({n})")
            })
            .collect();
        eprintln!("  {stem}: {}", shown.join("  "));
    }
}

/// Every row's residual, ranked by how many cases it gets wrong.
///
/// **This sweeps all 310 opcode files rather than a hand-kept list**, and that
/// is the whole point of it. The list this replaced held 78 stems, which were
/// the rows somebody had been working on when they added them, so "most of the
/// rows I looked at read +0" said nothing at all about the 232 nobody had
/// looked at. An aggregate with no per-row breakdown and a per-row breakdown
/// with no coverage are the same mistake twice.
///
/// The population is the clean one: a full queue and no prefix, where the
/// recorded span is the instruction's own clock count and nothing is waiting on
/// a differently-scheduled fetch. So a row that is wrong here is a *timing row*
/// that is wrong, and what this measures is how much of the gate's residual the
/// table accounts for, as against the prefetcher's scheduling, which this
/// population cannot see.
/// One opcode file's standing against the recording, for the ranked sweep.
struct RowResidual {
    stem: String,
    /// Cases compared: a full queue, no prefix, and a trace to compare against.
    cases: usize,
    /// How many of those this core gets wrong, which is what the ranking is by.
    wrong: usize,
    /// The signed differences, commonest first.
    modes: Vec<(i64, usize)>,
}

/// Which population a case belongs to, for the whole-corpus gap map.
///
/// The gate reports two populations, "empty queue" and "prefetched", and the
/// row meter reports one, "full queue and no prefix". Between them they leave
/// two whole populations nobody has ever measured: the partially-filled queues,
/// and every prefixed case in the suite. This names all of them.
fn population_of(tc: &I8088TestCase) -> (&'static str, &'static str) {
    let queue = match tc.initial.queue.len() {
        0 => "empty",
        4 => "full ",
        _ => "part ",
    };
    let prefix = match tc.bytes.first() {
        Some(0x26 | 0x2E | 0x36 | 0x3E) => "segment",
        Some(0xF2 | 0xF3) => "rep    ",
        Some(0xF0) => "lock   ",
        _ => "none   ",
    };
    (queue, prefix)
}

/// The whole corpus, bucketed by population, so that no part of it can be
/// unmeasured by accident.
///
/// Every percentage this epic has quoted describes a slice: the row meter sees
/// full-queue unprefixed cases, the gate splits empty against prefetched and
/// folds prefixes into both. A cause that lives only in the prefixed cases, or
/// only in the partially-filled queues, is invisible to all of it. This is the
/// map that says where the remaining error actually is.
#[test]
#[ignore = "survey, not a check: the whole corpus, bucketed by population"]
fn gap_map() {
    // (queue, prefix, touches memory) -> residual histogram
    type Key = (&'static str, &'static str, bool);
    let mut buckets: BTreeMap<Key, BTreeMap<i64, usize>> = BTreeMap::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        for tc in &tests {
            if tc.cycles.is_empty() {
                continue;
            }
            let Some(ours) = replay(tc) else { continue };
            let (queue, prefix) = population_of(tc);
            let touches = recorded_operand_start(tc).is_some();
            *buckets
                .entry((queue, prefix, touches))
                .or_default()
                .entry(ours as i64 - tc.cycles.len() as i64)
                .or_default() += 1;
        }
    }

    let grand: usize = buckets.values().flat_map(|h| h.values()).sum();
    let grand_exact: usize = buckets
        .values()
        .map(|h| h.get(&0).copied().unwrap_or(0))
        .sum();
    eprintln!("\nwhole corpus, by population");
    eprintln!(
        "  {grand_exact} of {grand} exact ({:.2}%)\n",
        100.0 * grand_exact as f64 / grand as f64
    );
    eprintln!("  queue prefix   memory     cases    exact   share of all error");
    /// One printed line: the population, its case and exact counts, and the
    /// residuals it carries, commonest first.
    type Row = (Key, usize, usize, Vec<(i64, usize)>);
    let mut rows: Vec<Row> = Vec::new();
    for (key, hist) in &buckets {
        let cases: usize = hist.values().sum();
        let exact = hist.get(&0).copied().unwrap_or(0);
        let mut modes: Vec<(i64, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        rows.push((*key, cases, exact, modes));
    }
    rows.sort_by_key(|(_, cases, exact, _)| std::cmp::Reverse(cases - exact));
    let all_error = grand - grand_exact;
    for ((queue, prefix, touches), cases, exact, modes) in &rows {
        let wrong = cases - exact;
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!(
            "  {queue} {prefix} {}  {cases:7}  {:6.2}%  {:5.1}%   {}",
            if *touches { "yes" } else { "no " },
            100.0 * *exact as f64 / *cases as f64,
            100.0 * wrong as f64 / all_error as f64,
            top.join(" ")
        );
    }
}

/// The segment-override population, which no row meter has ever looked at.
///
/// `row_residuals` filters every prefixed case out, and the gate folds them
/// into its two queue populations, so an override-only cause is invisible to
/// both. The whole-corpus map says they carry **48.8% of all remaining error**,
/// and the full-queue half of that is the tractable part: the same instructions
/// without an override are at 95% and 98%.
///
/// Ranked by cases wrong, over full-queue cases carrying exactly one override.
/// Rank every opcode's residual within one population.
///
/// The populations are the gap map's: an opcode can be exact in one and wrong
/// in another, and a ranking that mixes them hides which.
fn rank_rows(label: &str, keep: fn(&I8088TestCase) -> bool) {
    let mut rows: Vec<RowResidual> = Vec::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || !keep(tc) {
                continue;
            }
            let Some(ours) = replay(tc) else { continue };
            *hist
                .entry(ours as i64 - tc.cycles.len() as i64)
                .or_default() += 1;
        }
        let cases: usize = hist.values().sum();
        if cases == 0 {
            continue;
        }
        let exact = hist.get(&0).copied().unwrap_or(0);
        let mut modes: Vec<(i64, usize)> = hist.into_iter().collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        rows.push(RowResidual {
            stem,
            cases,
            wrong: cases - exact,
            modes,
        });
    }
    let cases: usize = rows.iter().map(|r| r.cases).sum();
    let wrong: usize = rows.iter().map(|r| r.wrong).sum();
    eprintln!(
        "\n{label}: {} of {cases} exact ({:.2}%)",
        cases - wrong,
        100.0 * (cases - wrong) as f64 / cases as f64
    );
    rows.sort_by_key(|r| std::cmp::Reverse(r.wrong));
    for RowResidual {
        stem,
        cases: total,
        wrong,
        modes,
    } in rows.iter().take(24)
    {
        if *wrong == 0 {
            break;
        }
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!(
            "  {stem:6} {wrong:5} of {total:5} wrong   {}",
            top.join(" ")
        );
    }
}

/// The empty-queue populations, ranked per opcode.
///
/// The gap map puts 12.4% of all remaining error in cases that start with an
/// empty queue, carry no prefix and touch no memory at all, 87,924 of them at
/// exactly +2. Nothing about operand access or arbitration can reach those, so
/// whatever it is belongs to the loader.
#[test]
#[ignore = "survey, not a check: the empty-queue rows, per opcode"]
fn empty_queue_rows() {
    rank_rows("empty queue, no prefix, no memory", |tc| {
        tc.initial.queue.is_empty()
            && !tc.bytes.first().is_some_and(|&b| is_prefix(b))
            && recorded_operand_start(tc).is_none()
    });
    rank_rows("empty queue, no prefix, touches memory", |tc| {
        tc.initial.queue.is_empty()
            && !tc.bytes.first().is_some_and(|&b| is_prefix(b))
            && recorded_operand_start(tc).is_some()
    });
}

/// The two populations no ranking has ever covered: an empty queue with a
/// segment override, which the gap map puts at 26.3% of all remaining error and
/// 6.34% exact, and the repeated string operations.
#[test]
#[ignore = "survey, not a check: the last unranked populations"]
fn remaining_population_rows() {
    rank_rows("empty queue, segment override", |tc| {
        tc.initial.queue.is_empty()
            && matches!(tc.bytes.first(), Some(0x26 | 0x2E | 0x36 | 0x3E))
            && !tc.bytes.get(1).is_some_and(|&b| is_prefix(b))
    });
    rank_rows("full queue, segment override, no memory", |tc| {
        tc.initial.queue.len() == 4
            && matches!(tc.bytes.first(), Some(0x26 | 0x2E | 0x36 | 0x3E))
            && !tc.bytes.get(1).is_some_and(|&b| is_prefix(b))
            && recorded_operand_start(tc).is_none()
    });
    rank_rows("repeated string operations", |tc| {
        matches!(tc.bytes.first(), Some(0xF2 | 0xF3))
    });
}

#[test]
#[ignore = "survey, not a check: what a segment override costs, per row"]
fn segment_override_rows() {
    let mut rows: Vec<RowResidual> = Vec::new();
    for stem in every_opcode_file() {
        let Some(tests) = load(&stem) else { continue };
        let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            // Exactly one override and nothing else in front of the opcode.
            if !matches!(tc.bytes.first(), Some(0x26 | 0x2E | 0x36 | 0x3E))
                || tc.bytes.get(1).is_some_and(|&b| is_prefix(b))
            {
                continue;
            }
            let Some(ours) = replay(tc) else { continue };
            *hist
                .entry(ours as i64 - tc.cycles.len() as i64)
                .or_default() += 1;
        }
        let cases: usize = hist.values().sum();
        if cases == 0 {
            continue;
        }
        let exact = hist.get(&0).copied().unwrap_or(0);
        let mut modes: Vec<(i64, usize)> = hist.into_iter().collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        rows.push(RowResidual {
            stem,
            cases,
            wrong: cases - exact,
            modes,
        });
    }

    let cases: usize = rows.iter().map(|r| r.cases).sum();
    let wrong: usize = rows.iter().map(|r| r.wrong).sum();
    let clean = rows.iter().filter(|r| r.wrong == 0).count();
    eprintln!("\nsegment override, full queue, one prefix, every file");
    eprintln!("  {} files, {clean} of them +0 on every case", rows.len());
    eprintln!(
        "  {} of {cases} cases exact ({:.2}%)\n",
        cases - wrong,
        100.0 * (cases - wrong) as f64 / cases as f64
    );
    rows.sort_by_key(|r| std::cmp::Reverse(r.wrong));
    for RowResidual {
        stem,
        cases: total,
        wrong,
        modes,
    } in &rows
    {
        if *wrong == 0 {
            break;
        }
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!(
            "  {stem:6} {wrong:5} of {total:5} wrong   {}",
            top.join(" ")
        );
    }
    // The clean ones matter as much: whether the eight-bit-immediate forms are
    // exact is what says the sixteen-bit ones are wrong for their immediate
    // rather than for having no ModR/M byte.
    let clean: Vec<&str> = rows
        .iter()
        .filter(|r| r.wrong == 0)
        .map(|r| r.stem.as_str())
        .collect();
    eprintln!("\n  exact on every case: {}", clean.join(" "));
}

#[test]
#[ignore = "survey, not a check: how far each row is from the recording"]
fn row_residuals() {
    let files = every_opcode_file();
    let mut rows: Vec<RowResidual> = map_files(&files, |stem| {
        let hist = residual_histogram(stem);
        let cases: usize = hist.values().sum();
        if cases == 0 {
            return None;
        }
        let exact = hist.get(&0).copied().unwrap_or(0);
        let mut modes: Vec<(i64, usize)> = hist.into_iter().collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        Some(RowResidual {
            stem: stem.to_string(),
            cases,
            wrong: cases - exact,
            modes,
        })
    })
    .into_iter()
    .flatten()
    .collect();

    let cases: usize = rows.iter().map(|r| r.cases).sum();
    let wrong: usize = rows.iter().map(|r| r.wrong).sum();
    let clean = rows.iter().filter(|r| r.wrong == 0).count();

    eprintln!("\nresiduals against the recording, full queue, no prefix, every file");
    eprintln!("  {} files, {clean} of them +0 on every case", rows.len());
    eprintln!(
        "  {} of {cases} cases exact ({:.2}%)",
        cases - wrong,
        100.0 * (cases - wrong) as f64 / cases as f64
    );
    eprintln!("\n  worst first, by cases wrong:");
    rows.sort_by_key(|r| std::cmp::Reverse(r.wrong));
    for RowResidual {
        stem,
        cases: total,
        wrong,
        modes,
    } in &rows
    {
        if *wrong == 0 {
            break;
        }
        let top: Vec<String> = modes
            .iter()
            .take(5)
            .map(|(d, n)| format!("{d:+}:{n}"))
            .collect();
        eprintln!(
            "  {stem:6} {wrong:5} of {total:5} wrong   {}",
            top.join(" ")
        );
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

/// `AAM` divides AL by its immediate through the same `CORD` loop `DIV` uses,
/// and `AAD` multiplies AH by its immediate through the same loop `MUL` uses.
/// If that is right, their spans follow the same two rules: the number of
/// *compared* subtracts for the divide, and the number of set bits in the
/// multiplier for the multiply.
///
/// Prints the span grouped by the key, so a rule that holds shows up as one
/// span per group rather than as a correlation.
fn ascii_adjust_survey(stem: &str, key: fn(u8, u8) -> u32, label: &str) {
    let Some(tests) = load(stem) else { return };
    let mut groups: BTreeMap<u32, BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some(&imm) = tc.bytes.get(1) else {
            continue;
        };
        let ax = tc.initial.regs.ax;
        *groups
            .entry(key(ax as u8, imm))
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
        eprintln!("  {k:3}: {}", top.join(" "));
    }
}

/// The compared subtracts an 8-bit long division makes, which is what `DIV`'s
/// cost was found to follow: the pass that jumps straight to the subtract costs
/// the same as the pass that does not subtract at all, and only the pass that
/// compares first and then subtracts is longer.
fn compared_subtracts(dividend: u16, divisor: u8) -> u32 {
    if divisor == 0 {
        return 0;
    }
    let divisor = u32::from(divisor);
    let mut a = u32::from(dividend >> 8);
    let mut c = u32::from(dividend & 0xFF);
    let mut qbit = 0u32;
    let mut compared = 0;
    for _ in 0..8 {
        let carry_out = a & 0x80 != 0;
        a = ((a << 1) & 0xFF) | u32::from(c & 0x80 != 0);
        c = ((c << 1) & 0xFF) | qbit;
        if carry_out {
            a = a.wrapping_sub(divisor) & 0xFF;
            qbit = 1;
        } else if a >= divisor {
            compared += 1;
            a -= divisor;
            qbit = 1;
        } else {
            qbit = 0;
        }
    }
    compared
}

#[test]
#[ignore = "survey, not a check: do AAM and AAD follow the divide and multiply loops"]
fn ascii_adjust_loops() {
    // AAM divides AL by the immediate, so the dividend is AL with a zero high
    // half.
    ascii_adjust_survey(
        "D4",
        |al, imm| compared_subtracts(u16::from(al), imm),
        "compared subtracts dividing AL",
    );
    // AAD's multiplier: AH is the candidate, the immediate the other.
    ascii_adjust_survey("D5", |_, imm| imm.count_ones(), "set bits in the immediate");
}

/// AAM's compared-subtract groups are not uniform: each splits in two, two
/// clocks apart. This asks what separates them.
#[test]
#[ignore = "survey, not a check: what splits AAM's groups"]
fn aam_residual() {
    let Some(tests) = load("D4") else { return };
    let mut groups: BTreeMap<(u32, bool, bool, bool), BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        let Some(&imm) = tc.bytes.get(1) else {
            continue;
        };
        if imm == 0 {
            continue;
        }
        let al = tc.initial.regs.ax as u8;
        let quotient = al / imm;
        let key = (
            compared_subtracts(u16::from(al), imm),
            quotient & 1 != 0,
            al.is_multiple_of(imm),
            quotient.count_ones() > 1,
        );
        *groups
            .entry(key)
            .or_default()
            .entry(tc.cycles.len())
            .or_default() += 1;
    }
    eprintln!("\nD4 by (compared, odd quotient, zero remainder, multi-bit quotient)");
    for (k, hist) in &groups {
        let mut modes: Vec<(usize, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(3)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!("  {k:?}: {}", top.join(" "));
    }
}

/// `AAM`'s groups split on the low bit of its quotient, two clocks apart. `DIV`
/// has a residual of up to two clocks that was never explained, and the two
/// share the same `CORD` loop, so this asks the same question of `DIV`.
///
/// Register operands only, so no effective address or operand read is in the
/// span, and non-faulting cases only.
#[test]
#[ignore = "survey, not a check: does DIV's residual split the way AAM's does"]
fn divide_residual() {
    for (stem, word) in [("F6.6", false), ("F7.6", true)] {
        let Some(tests) = load(stem) else { continue };
        let mut groups: BTreeMap<(u32, bool), BTreeMap<usize, usize>> = BTreeMap::new();
        for tc in &tests {
            if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
                continue;
            }
            if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
                continue;
            }
            let Some(&modrm) = tc.bytes.get(1) else {
                continue;
            };
            if modrm >> 6 != 3 {
                continue;
            }
            let r = &tc.initial.regs;
            let divisor: u32 = if word {
                match modrm & 7 {
                    0 => r.ax,
                    1 => r.cx,
                    2 => r.dx,
                    3 => r.bx,
                    4 => r.sp,
                    5 => r.bp,
                    6 => r.si,
                    _ => r.di,
                }
                .into()
            } else {
                let regs = [
                    r.ax as u8,
                    r.cx as u8,
                    r.dx as u8,
                    r.bx as u8,
                    (r.ax >> 8) as u8,
                    (r.cx >> 8) as u8,
                    (r.dx >> 8) as u8,
                    (r.bx >> 8) as u8,
                ];
                regs[(modrm & 7) as usize].into()
            };
            let dividend: u32 = if word {
                (u32::from(r.dx) << 16) | u32::from(r.ax)
            } else {
                r.ax.into()
            };
            let limit = if word { 0xFFFF } else { 0xFF };
            if divisor == 0 || dividend / divisor > limit {
                // The divide error, which never enters the loop. Grouped under
                // a compared count of 99 so it is visible rather than skipped.
                *groups
                    .entry((99, divisor == 0))
                    .or_default()
                    .entry(tc.cycles.len())
                    .or_default() += 1;
                continue;
            }
            let quotient = dividend / divisor;
            *groups
                .entry((cord_compared(dividend, divisor, word), quotient & 1 != 0))
                .or_default()
                .entry(tc.cycles.len())
                .or_default() += 1;
        }
        eprintln!("\n{stem} by (compared subtracts, odd quotient)");
        for (k, hist) in &groups {
            let mut modes: Vec<(usize, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
            modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            let top: Vec<String> = modes
                .iter()
                .take(3)
                .map(|(v, n)| format!("{v}:{n}"))
                .collect();
            eprintln!("  {k:?}: {}", top.join(" "));
        }
    }
}

/// The compared subtracts of the long division at either width, the same walk
/// `timing::divide_cycles` makes.
fn cord_compared(dividend: u32, divisor: u32, word: bool) -> u32 {
    let width = if word { 16 } else { 8 };
    let mask = (1u32 << width) - 1;
    let top = 1u32 << (width - 1);
    let mut a = (dividend >> width) & mask;
    let mut c = dividend & mask;
    let mut qbit = 0u32;
    let mut compared = 0;
    for _ in 0..width {
        let carry_out = (a & top) != 0;
        a = ((a << 1) & mask) | u32::from((c & top) != 0);
        c = ((c << 1) & mask) | qbit;
        if carry_out {
            a = a.wrapping_sub(divisor) & mask;
            qbit = 1;
        } else if a >= divisor {
            compared += 1;
            a -= divisor;
            qbit = 1;
        } else {
            qbit = 0;
        }
    }
    compared
}

/// AAD again, keyed on AH rather than on the immediate.
#[test]
#[ignore = "survey, not a check: which operand AAD's multiply loop tests"]
fn aad_multiplier() {
    let Some(tests) = load("D5") else { return };
    let mut groups: BTreeMap<u32, BTreeMap<usize, usize>> = BTreeMap::new();
    for tc in &tests {
        if tc.cycles.is_empty() || tc.initial.queue.len() != 4 {
            continue;
        }
        if tc.bytes.first().is_some_and(|&b| is_prefix(b)) {
            continue;
        }
        *groups
            .entry((tc.initial.regs.ax >> 8).count_ones())
            .or_default()
            .entry(tc.cycles.len())
            .or_default() += 1;
    }
    eprintln!("\nD5 by set bits in AH");
    for (k, hist) in &groups {
        let mut modes: Vec<(usize, usize)> = hist.iter().map(|(a, b)| (*a, *b)).collect();
        modes.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let top: Vec<String> = modes
            .iter()
            .take(4)
            .map(|(v, n)| format!("{v}:{n}"))
            .collect();
        eprintln!("  {k:3}: {}", top.join(" "));
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
    dump("E4", false);
    dump("E7", false);
    dump("CC", true);
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
    // SALC, which Intel never documented: it sets AL to 0xFF when carry is set
    // and to zero when it is not, and the recording gives it two costs one
    // clock apart. If that is the same branch, this splits it.
    split_by("D6", "carry set", |tc| tc.initial.regs.flags & 1 != 0);
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
