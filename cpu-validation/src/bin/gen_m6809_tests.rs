use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
use phosphor_cpu_validation::{BusOp, CpuState, TestCase, TracingBus};
use rand::Rng;

const NUM_TESTS: usize = 1000;

fn snapshot_cpu(cpu: &M6809) -> CpuState {
    CpuState {
        pc: cpu.pc,
        s: cpu.s,
        u: cpu.u,
        a: cpu.a,
        b: cpu.b,
        dp: cpu.dp,
        x: cpu.x,
        y: cpu.y,
        cc: cpu.cc,
        ram: Vec::new(),
    }
}

fn build_ram(memory: &[u8; 0x10000], addresses: &BTreeSet<u16>) -> Vec<(u16, u8)> {
    addresses
        .iter()
        .map(|&addr| (addr, memory[addr as usize]))
        .collect()
}

/// Returns instruction byte count for supported page-1 opcodes, or None if unsupported.
fn opcode_size(opcode: u8) -> Option<u8> {
    match opcode {
        // --- Inherent (size 1) ---
        0x12 | 0x19 | 0x1D | 0x3A | 0x3D => Some(1), // NOP, DAA, SEX, ABX, MUL
        0x40 | 0x43 | 0x44 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4A | 0x4C
        | 0x4D | 0x4F => Some(1), // A-register inherent (excl undocumented 0x41,0x45,0x4B)
        0x50 | 0x53 | 0x54 | 0x56 | 0x57 | 0x58 | 0x59 | 0x5A | 0x5C
        | 0x5D | 0x5F => Some(1), // B-register inherent (excl undocumented 0x51,0x55,0x5B)

        // --- 8-bit immediate (size 2) ---
        0x1A | 0x1C => Some(2), // ORCC, ANDCC
        0x80 | 0x81 | 0x82 | 0x84 | 0x85 | 0x86 | 0x88 | 0x89 | 0x8A | 0x8B => Some(2), // A-ALU imm
        0xC0 | 0xC1 | 0xC2 | 0xC4 | 0xC5 | 0xC6 | 0xC8 | 0xC9 | 0xCA | 0xCB => Some(2), // B-ALU imm

        // --- 16-bit immediate (size 3) ---
        0x83 | 0x8C | 0x8E => Some(3), // SUBD, CMPX, LDX imm16
        0xC3 | 0xCC | 0xCE => Some(3), // ADDD, LDD, LDU imm16

        // --- Short branch (size 2) ---
        0x20..=0x2F => Some(2),

        // --- Direct mode (size 2) ---
        0x00 | 0x03 | 0x04 | 0x06..=0x0A | 0x0C..=0x0F => Some(2), // direct unary/shift/JMP/CLR (excl undocumented 0x01,0x05,0x0B)
        0x90..=0x9C | 0x9E | 0x9F => Some(2), // A-side direct ALU (excl 0x9D JSR)
        0xD0..=0xDF => Some(2),               // B-side direct ALU

        // --- Extended mode (size 3) ---
        0x70 | 0x73 | 0x74 | 0x76..=0x7A | 0x7C..=0x7F => Some(3), // extended unary/shift/JMP/CLR (excl undocumented 0x71,0x75,0x7B)
        0xB0..=0xBC | 0xBE | 0xBF => Some(3), // A-side extended ALU (excl 0xBD JSR)
        0xF0..=0xFF => Some(3),               // B-side extended ALU

        _ => None,
    }
}

/// Generate NUM_TESTS randomized test vectors for a single opcode.
fn generate_opcode(rng: &mut impl Rng, opcode: u8, instr_size: u8) -> Vec<TestCase> {
    let mut tests = Vec::with_capacity(NUM_TESTS);
    let max_pc = (0x10000u32 - instr_size as u32) as u16;

    for _ in 0..NUM_TESTS {
        let mut cpu = M6809::new();
        let mut bus = TracingBus::new();

        // Fill entire 64KB with random data
        rng.fill(&mut bus.memory[..]);

        // Randomize all registers
        cpu.a = rng.r#gen();
        cpu.b = rng.r#gen();
        cpu.dp = rng.r#gen();
        cpu.x = rng.r#gen();
        cpu.y = rng.r#gen();
        cpu.u = rng.r#gen();
        cpu.s = rng.r#gen();
        cpu.cc = rng.r#gen();
        cpu.pc = rng.gen_range(0..=max_pc);

        // Place opcode byte; operand bytes are already random from the fill
        let pc = cpu.pc;
        bus.memory[pc as usize] = opcode;

        // Snapshot pre-execution memory
        let pre_memory = bus.memory;

        // Snapshot initial CPU state
        let mut initial = snapshot_cpu(&cpu);

        // Execute one instruction, detecting internal cycles
        let mut all_cycles: Vec<(u16, u8, BusOp)> = Vec::new();
        loop {
            let before = bus.cycles.len();
            let done = cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
            if bus.cycles.len() > before {
                // Bus activity occurred — copy new entries
                for c in &bus.cycles[before..] {
                    all_cycles.push((c.addr, c.data, c.op));
                }
            } else {
                // No bus activity — internal cycle
                all_cycles.push((0xFFFF, 0, BusOp::Internal));
            }
            if done {
                break;
            }
        }

        // Snapshot final CPU state
        let mut final_state = snapshot_cpu(&cpu);

        // Collect all accessed addresses (skip internal cycle sentinel 0xFFFF)
        let addresses: BTreeSet<u16> = all_cycles
            .iter()
            .filter(|(_, _, op)| *op != BusOp::Internal)
            .map(|&(addr, _, _)| addr)
            .collect();

        // Build ram fields from pre/post memory
        initial.ram = build_ram(&pre_memory, &addresses);
        final_state.ram = build_ram(&bus.memory, &addresses);

        // Build cycles array
        let cycles: Vec<(u16, u8, String)> = all_cycles
            .iter()
            .map(|&(addr, data, op)| {
                let op_str = match op {
                    BusOp::Read => "read".to_string(),
                    BusOp::Write => "write".to_string(),
                    BusOp::Internal => "internal".to_string(),
                };
                (addr, data, op_str)
            })
            .collect();

        // Build name from instruction bytes at PC
        let name = (0..instr_size as u16)
            .map(|i| format!("{:02x}", pre_memory[pc.wrapping_add(i) as usize]))
            .collect::<Vec<_>>()
            .join(" ");

        tests.push(TestCase {
            name,
            initial,
            final_state,
            cycles,
        });
    }

    tests
}

fn generate_and_write(rng: &mut impl Rng, opcode: u8, instr_size: u8, out_dir: &Path) {
    let tests = generate_opcode(rng, opcode, instr_size);
    let out_path = out_dir.join(format!("{:02x}.json", opcode));
    let json = serde_json::to_string_pretty(&tests).expect("Failed to serialize test cases");
    fs::write(&out_path, json).expect("Failed to write output file");
    println!(
        "Generated {} tests for 0x{:02X} -> {}",
        tests.len(),
        opcode,
        out_path.display()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: gen_m6809_tests <opcode_hex | all>");
        eprintln!("Examples:");
        eprintln!("  gen_m6809_tests 0x86");
        eprintln!("  gen_m6809_tests all");
        std::process::exit(1);
    }

    let out_dir = Path::new("test_data/m6809");
    fs::create_dir_all(out_dir).expect("Failed to create output directory");

    let mut rng = rand::thread_rng();

    if args[1] == "all" {
        let mut count = 0;
        for opcode in 0x00..=0xFFu8 {
            if let Some(size) = opcode_size(opcode) {
                generate_and_write(&mut rng, opcode, size, out_dir);
                count += 1;
            }
        }
        println!("Generated tests for {} opcodes", count);
    } else {
        let opcode_str = args[1].trim_start_matches("0x").trim_start_matches("0X");
        let opcode = u8::from_str_radix(opcode_str, 16).unwrap_or_else(|_| {
            eprintln!("Invalid hex opcode: {}", args[1]);
            std::process::exit(1);
        });
        let size = opcode_size(opcode).unwrap_or_else(|| {
            eprintln!("Opcode 0x{:02X} not supported for test generation", opcode);
            std::process::exit(1);
        });
        generate_and_write(&mut rng, opcode, size, out_dir);
    }
}
