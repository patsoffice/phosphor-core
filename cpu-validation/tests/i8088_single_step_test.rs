use std::io::Read;
use std::path::Path;

use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::i8088::I8088;
use phosphor_cpu_validation::{I8088InitialState, I8088Metadata, I8088TestCase, TracingBus20};

// ---------------------------------------------------------------------------
// Opcodes to skip (not yet implemented — Step 1.8: interrupts, I/O, flags)
// ---------------------------------------------------------------------------

/// Returns true if the given opcode file should be skipped because the
/// instruction isn't implemented yet.
fn should_skip(filename: &str) -> bool {
    // Strip .json.gz suffix to get the opcode identifier
    let stem = filename.strip_suffix(".json.gz").unwrap_or(filename);

    matches!(
        stem,
        // Prefixes (no standalone execution)
        "26" | "2E" | "36" | "3E" | "F0" | "F1" | "F2" | "F3"
        // PUSHF/POPF (0x9C-0x9D) — not yet implemented
        | "9C" | "9D"
        // INT3 (0xCC), INT imm8 (0xCD), INTO (0xCE), IRET (0xCF)
        | "CC" | "CD" | "CE" | "CF"
        // IN/OUT (0xE4-0xE7, 0xEC-0xEF)
        | "E4" | "E5" | "E6" | "E7" | "EC" | "ED" | "EE" | "EF"
        // Flag control: CLC(F8) STC(F9) CLI(FA) STI(FB) CLD(FC) STD(FD) CMC(F5)
        | "F5" | "F8" | "F9" | "FA" | "FB" | "FC" | "FD"
        // HLT (0xF4) — blocks forever in our model
        | "F4"
        // FPU ESC opcodes (0xD8-0xDF)
        | "D8" | "D9" | "DA" | "DB" | "DC" | "DD" | "DE" | "DF"
        // SALC / undocumented (0xD6)
        | "D6"
        // F6.1 / F7.1 — undocumented TEST aliases (same encoding as F6.0/F7.0)
        | "F6.1" | "F7.1"
        // D0.6/D1.6/D2.6/D3.6 — undocumented SETMO/SETMOC
        | "D0.6" | "D1.6" | "D2.6" | "D3.6"
        // 0x60-0x6F aliases (8088 aliases for PUSH/POP/Jcc, hardware-dependent)
        | "60" | "61" | "62" | "63" | "64" | "65" | "66" | "67"
        | "68" | "69" | "6A" | "6B" | "6C" | "6D" | "6E" | "6F"
        // 0xC0, 0xC1 — aliases for RET
        | "C0" | "C1"
        // 0xC8, 0xC9 — aliases for RETF
        | "C8" | "C9"
        // 0x0F — POP CS (undocumented, rarely used)
        | "0F"
        // FF.7 — undefined sub-opcode
        | "FF.7"
        // AAM 0 / DIV / IDIV overflow — these trigger INT 0 (not yet implemented)
        | "D4" | "F6.6" | "F6.7" | "F7.6" | "F7.7"
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
}

fn run_test_case(tc: &I8088TestCase, flags_mask: u16) -> Option<String> {
    let mut cpu = I8088::new();
    let mut bus = TracingBus20::new();

    load_initial_state(&mut cpu, &mut bus, &tc.initial);

    // Execute one instruction until instruction boundary
    let mut total_ticks = 0;
    loop {
        total_ticks += 1;
        if cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)) {
            break;
        }
        if total_ticks > 500 {
            return Some(format!(
                "{}: instruction did not complete in 500 cycles",
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

    // Check memory (final ram contains only changed locations)
    for &(addr, expected) in &tc.final_state.ram {
        let actual = bus.memory[(addr & 0xF_FFFF) as usize];
        if actual != expected {
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
    let test_dir = Path::new("test_data/8088/v2");
    if !test_dir.exists() {
        panic!(
            "No SingleStepTests data. Run: git submodule update --init cpu-validation/test_data/8088"
        );
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
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "gz"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut total_tests = 0;
    let mut total_files = 0;
    let mut skipped_files = 0;
    let mut failed_tests = 0;
    let mut failed_files = std::collections::BTreeSet::new();
    let mut first_failures: Vec<String> = Vec::new();

    for entry in &entries {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        if should_skip(&filename_str) {
            skipped_files += 1;
            continue;
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

        let mut file_recorded = false;
        for tc in &tests {
            if let Some(err) = run_test_case(tc, flags_mask) {
                failed_tests += 1;
                if !file_recorded {
                    file_recorded = true;
                    failed_files.insert(filename_str.to_string());
                    if first_failures.len() < 100 {
                        first_failures.push(err);
                    }
                }
            }
        }

        total_tests += tests.len();
        total_files += 1;
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
