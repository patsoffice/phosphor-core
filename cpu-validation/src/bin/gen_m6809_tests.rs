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

fn generate_opcode_86(rng: &mut impl Rng) -> Vec<TestCase> {
    let mut tests = Vec::with_capacity(NUM_TESTS);

    for _ in 0..NUM_TESTS {
        let mut cpu = M6809::new();
        let mut bus = TracingBus::new();

        // Randomize all registers
        cpu.a = rng.r#gen();
        cpu.b = rng.r#gen();
        cpu.dp = rng.r#gen();
        cpu.x = rng.r#gen();
        cpu.y = rng.r#gen();
        cpu.u = rng.r#gen();
        cpu.s = rng.r#gen();
        cpu.cc = rng.r#gen();
        // PC needs room for opcode + 1 operand byte
        cpu.pc = rng.gen_range(0x0000..=0xFFFE);

        // Place opcode and random immediate operand
        let pc = cpu.pc;
        let operand: u8 = rng.r#gen();
        bus.memory[pc as usize] = 0x86;
        bus.memory[pc.wrapping_add(1) as usize] = operand;

        // Snapshot pre-execution memory
        let pre_memory = bus.memory;

        // Snapshot initial CPU state
        let mut initial = snapshot_cpu(&cpu);

        // Execute one instruction
        loop {
            if cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)) {
                break;
            }
        }

        // Snapshot final CPU state
        let mut final_state = snapshot_cpu(&cpu);

        // Collect all accessed addresses
        let addresses: BTreeSet<u16> = bus.cycles.iter().map(|c| c.addr).collect();

        // Build ram fields from pre/post memory
        initial.ram = build_ram(&pre_memory, &addresses);
        final_state.ram = build_ram(&bus.memory, &addresses);

        // Build cycles array
        let cycles: Vec<(u16, u8, String)> = bus
            .cycles
            .iter()
            .map(|c| {
                let op = match c.op {
                    BusOp::Read => "read".to_string(),
                    BusOp::Write => "write".to_string(),
                };
                (c.addr, c.data, op)
            })
            .collect();

        // Build name from instruction bytes
        let name = format!("{:02x} {:02x}", 0x86, operand);

        tests.push(TestCase {
            name,
            initial,
            final_state,
            cycles,
        });
    }

    tests
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: gen_m6809_tests <opcode_hex>");
        eprintln!("Example: gen_m6809_tests 0x86");
        std::process::exit(1);
    }

    let opcode_str = args[1].trim_start_matches("0x").trim_start_matches("0X");
    let opcode = u8::from_str_radix(opcode_str, 16).unwrap_or_else(|_| {
        eprintln!("Invalid hex opcode: {}", args[1]);
        std::process::exit(1);
    });

    let mut rng = rand::thread_rng();

    let tests = match opcode {
        0x86 => generate_opcode_86(&mut rng),
        _ => {
            eprintln!("Opcode 0x{:02X} not yet supported", opcode);
            std::process::exit(1);
        }
    };

    // Write output
    let out_dir = Path::new("test_data/m6809");
    fs::create_dir_all(out_dir).expect("Failed to create output directory");

    let out_path = out_dir.join(format!("{:02x}.json", opcode));
    let json = serde_json::to_string_pretty(&tests).expect("Failed to serialize test cases");
    fs::write(&out_path, json).expect("Failed to write output file");

    println!(
        "Generated {} test cases for opcode 0x{:02X} -> {}",
        tests.len(),
        opcode,
        out_path.display()
    );
}
