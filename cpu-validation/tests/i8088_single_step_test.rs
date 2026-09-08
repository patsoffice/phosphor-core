use std::io::Read;

use rayon::prelude::*;

use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8088::I8088;
use phosphor_cpu_validation::{I8088InitialState, I8088Metadata, I8088TestCase, TracingBus20};

// ---------------------------------------------------------------------------
// Opcodes to skip (not yet implemented — Step 1.8: interrupts, I/O, flags)
// ---------------------------------------------------------------------------

/// Returns true if the given opcode file should be skipped because the
/// instruction isn't implemented yet.
///
/// Eighteen files came off this list on 2026-09-04, found by the per-cycle
/// gate rather than by this one. Adding a loader that fetches an instruction
/// before executing it made the executor's silence about them measurable: the
/// loader knew how long each instruction was and the executor consumed fewer
/// bytes, which is a disagreement this gate could never see because it only
/// checks the state an instruction leaves behind.
///
/// - `60`-`6F`: the conditional jumps sixteen above them. The suite's own
///   capture disassembles 0x60 as JO, 0x65 as JNZ, 0x6A as JP and 0x6F as
///   JNLE, each with a rel8, so "hardware-dependent aliases" understated what
///   was known about them.
/// - `F6.1`, `F7.1`: TEST, the same instruction as `F6.0` and `F7.0`. The
///   group's dispatch handled reg=0 and left reg=1 to do nothing, so its
///   immediate was never consumed.
///
/// Four more came off on the same day, found the same way once the prefetch
/// queue existed: `C0`, `C1`, `C8` and `C9` are RET and RETF, aliases of `C2`,
/// `C3`, `CA` and `CB`. They consumed their bytes but did not return, so they
/// never flushed the queue, and the recorded traces end with an `E` that this
/// core did not produce.
///
/// And eight more with M4: `E4`-`E7` and `EC`-`EF`, the `IN` and `OUT` forms.
/// Their reason was never that the instructions were unimplemented but that
/// this harness could not feed them: the suite keeps a port read's data in the
/// cycle trace rather than in `initial.ram`. It reads the trace now, through
/// [`I8088TestCase::port_reads`], which is the coverage the design doc said the
/// conversion would pay for itself with.
fn should_skip(filename: &str) -> bool {
    // Strip .json.gz suffix to get the opcode identifier
    let stem = filename.strip_suffix(".json.gz").unwrap_or(filename);

    matches!(
        stem,
        // Prefixes (no standalone execution)
        "26" | "2E" | "36" | "3E" | "F0" | "F1" | "F2" | "F3"
        // HLT (0xF4) — blocks forever in test harness (no interrupts)
        | "F4"
        // FPU ESC opcodes (0xD8-0xDF)
        | "D8" | "D9" | "DA" | "DB" | "DC" | "DD" | "DE" | "DF"
        // SALC / undocumented (0xD6)
        | "D6"
        // D0.6/D1.6/D2.6/D3.6 — undocumented SETMO/SETMOC
        | "D0.6" | "D1.6" | "D2.6" | "D3.6"
        // 0x0F — POP CS (undocumented, rarely used)
        | "0F"
    )
}

// ---------------------------------------------------------------------------
// CPU state loading and comparison
// ---------------------------------------------------------------------------

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

    // Load RAM (20-bit physical addresses)
    for &(addr, val) in &state.ram {
        bus.memory[(addr & 0xF_FFFF) as usize] = val;
    }

    // Install the prefetch queue for the half of the suite that runs from a
    // full one. This gate does not check timing, so it did not need the queue
    // while there was no queue to fill; now that there is one, leaving it empty
    // would have the BIU fetch bytes the hardware already held, which changes
    // nothing about the final state but makes the two gates model different
    // machines.
    cpu.load_prefetch_queue(&state.queue);
}

fn run_test_case(tc: &I8088TestCase, flags_mask: u16) -> Option<String> {
    let mut cpu = I8088::new();
    let mut bus = TracingBus20::new();

    load_initial_state(&mut cpu, &mut bus, &tc.initial);
    // What the part read from its ports, so that an `IN` puts the same byte in
    // the accumulator here as it did on the bench.
    bus.port_reads = tc.port_reads();

    // Execute one instruction until instruction boundary
    let mut total_ticks = 0;
    loop {
        total_ticks += 1;
        if cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)) {
            break;
        }
        // Generous, because a repeated string operation really is this long: a
        // `REP STOSB` in the recording runs to 1,193 cycles. The cap is here to
        // catch a core that never retires, not to bound an instruction.
        if total_ticks > 4000 {
            return Some(format!(
                "{}: instruction did not complete in 4000 cycles",
                tc.name
            ));
        }
    }

    // Resolve expected values: final overrides initial for changed registers
    let init = &tc.initial.regs;
    let fin = &tc.final_state.regs;

    macro_rules! expected {
        ($field:ident) => {
            fin.$field.unwrap_or(init.$field)
        };
    }

    macro_rules! check {
        ($got:expr, $exp:expr, $name:expr) => {
            if $got != $exp {
                return Some(format!(
                    "{}: {} (got 0x{:04X} exp 0x{:04X})",
                    tc.name, $name, $got, $exp
                ));
            }
        };
    }

    check!(cpu.ax, expected!(ax), "AX");
    check!(cpu.bx, expected!(bx), "BX");
    check!(cpu.cx, expected!(cx), "CX");
    check!(cpu.dx, expected!(dx), "DX");
    check!(cpu.cs, expected!(cs), "CS");
    check!(cpu.ss, expected!(ss), "SS");
    check!(cpu.ds, expected!(ds), "DS");
    check!(cpu.es, expected!(es), "ES");
    check!(cpu.sp, expected!(sp), "SP");
    check!(cpu.bp, expected!(bp), "BP");
    check!(cpu.si, expected!(si), "SI");
    check!(cpu.di, expected!(di), "DI");
    check!(cpu.ip, expected!(ip), "IP");

    // Flags: mask out undefined bits per metadata
    let got_flags = cpu.flags & flags_mask;
    let exp_flags = expected!(flags) & flags_mask;
    check!(got_flags, exp_flags, "FLAGS");

    // Detect divide error: if flags have undefined bits and SP decreased by 6,
    // the instruction fired INT 0 and pushed FLAGS/CS/IP. The pushed FLAGS on
    // the stack include "undefined" flag bits that the 8088's internal division
    // microcode sets unpredictably. Mask those bytes like we mask the register.
    let flags_push_addrs: Option<(u32, u32)> = if flags_mask != 0xFFFF {
        let exp_sp = expected!(sp);
        if init.sp.wrapping_sub(exp_sp) == 6 {
            // FLAGS were pushed at SS:(init_SP - 2), physical = SS*16 + init_SP - 2
            let base = (init.ss as u32) * 16 + init.sp.wrapping_sub(2) as u32;
            Some((base & 0xF_FFFF, (base + 1) & 0xF_FFFF))
        } else {
            None
        }
    } else {
        None
    };

    // Check memory (final ram contains only changed locations)
    for &(addr, expected) in &tc.final_state.ram {
        let phys = addr & 0xF_FFFF;
        let actual = bus.memory[phys as usize];
        if actual != expected {
            // If this is a pushed-FLAGS byte, apply the flags mask
            if let Some((lo_addr, hi_addr)) = flags_push_addrs {
                if phys == lo_addr {
                    let mask_lo = (flags_mask & 0xFF) as u8;
                    if (actual & mask_lo) == (expected & mask_lo) {
                        continue;
                    }
                } else if phys == hi_addr {
                    let mask_hi = ((flags_mask >> 8) & 0xFF) as u8;
                    if (actual & mask_hi) == (expected & mask_hi) {
                        continue;
                    }
                }
            }
            return Some(format!(
                "{}: RAM[0x{:05X}] (got 0x{:02X} exp 0x{:02X})",
                tc.name, addr, actual, expected
            ));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

#[test]
fn test_all_i8088_opcodes() {
    let test_dir = phosphor_cpu_validation::vector_dir("8088/v2");
    let test_dir = test_dir.as_path();
    if !phosphor_cpu_validation::require_test_data(
        test_dir,
        "run: git submodule update --init cpu-validation/test_data/8088",
    ) {
        return;
    }

    // Load metadata for flag masks
    let metadata_path = test_dir.join("metadata.json");
    let metadata: I8088Metadata = {
        let json = std::fs::read_to_string(&metadata_path)
            .unwrap_or_else(|e| panic!("Failed to read metadata.json: {}", e));
        serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("Failed to parse metadata.json: {}", e))
    };

    // Collect gzipped test files
    let mut entries: Vec<_> = std::fs::read_dir(test_dir)
        .expect("Failed to read test directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "gz"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    /// One opcode file's verdict. Files are independent — decompress, parse,
    /// run — so they are measured in parallel and reduced afterwards in file
    /// order, which keeps the reported counts and the first-failure list
    /// identical to a sequential run.
    struct FileOutcome {
        filename: String,
        skipped: bool,
        tests: usize,
        failed: usize,
        first_failure: Option<String>,
    }

    let outcomes: Vec<FileOutcome> = entries
        .par_iter()
        .map(|entry| {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy().into_owned();

            if should_skip(&filename_str) {
                return FileOutcome {
                    filename: filename_str,
                    skipped: true,
                    tests: 0,
                    failed: 0,
                    first_failure: None,
                };
            }

            // Determine the opcode key for metadata lookup (strip .json.gz)
            let opcode_key = filename_str
                .strip_suffix(".json.gz")
                .unwrap_or(&filename_str)
                .to_uppercase();

            // Look up flag mask (handles nested reg sub-keys for group opcodes)
            let flags_mask = metadata.flags_mask_for(&opcode_key);

            // Decompress and parse the gzipped JSON
            let gz_data = std::fs::read(entry.path())
                .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", entry.path(), e));
            let mut decoder = flate2::read::GzDecoder::new(&gz_data[..]);
            let mut json = String::new();
            decoder
                .read_to_string(&mut json)
                .unwrap_or_else(|e| panic!("Failed to decompress {:?}: {}", entry.path(), e));
            let tests: Vec<I8088TestCase> = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", entry.path(), e));

            assert!(!tests.is_empty(), "Test file {} is empty", filename_str);

            let mut failed = 0;
            let mut first_failure = None;
            for tc in &tests {
                if let Some(err) = run_test_case(tc, flags_mask) {
                    failed += 1;
                    if first_failure.is_none() {
                        first_failure = Some(err);
                    }
                }
            }

            FileOutcome {
                filename: filename_str,
                skipped: false,
                tests: tests.len(),
                failed,
                first_failure,
            }
        })
        .collect();

    let mut total_tests = 0;
    let mut total_files = 0;
    let mut skipped_files = 0;
    let mut failed_tests = 0;
    let mut failed_files = std::collections::BTreeSet::new();
    let mut first_failures: Vec<String> = Vec::new();

    for outcome in outcomes {
        if outcome.skipped {
            skipped_files += 1;
            continue;
        }
        total_tests += outcome.tests;
        total_files += 1;
        failed_tests += outcome.failed;
        if let Some(err) = outcome.first_failure {
            failed_files.insert(outcome.filename);
            if first_failures.len() < 100 {
                first_failures.push(err);
            }
        }
    }

    eprintln!(
        "\nI8088 SingleStepTests: {} passed, {} failed across {} files ({} skipped)",
        total_tests - failed_tests,
        failed_tests,
        total_files,
        skipped_files
    );

    if !first_failures.is_empty() {
        eprintln!("\nFirst failure per file ({} files):", failed_files.len());
        for err in &first_failures {
            eprintln!("  {}", err);
        }
    }

    if failed_tests > 0 {
        panic!(
            "{} tests failed across {} files (out of {} tests in {} files)",
            failed_tests,
            failed_files.len(),
            total_tests,
            total_files
        );
    }
}
