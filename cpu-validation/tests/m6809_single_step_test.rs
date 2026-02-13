use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
use phosphor_cpu_validation::{BusOp, TestCase, TracingBus};

fn run_test_case(tc: &TestCase) {
    let mut cpu = M6809::new();
    let mut bus = TracingBus::new();

    // Load initial state
    cpu.pc = tc.initial.pc;
    cpu.s = tc.initial.s;
    cpu.u = tc.initial.u;
    cpu.a = tc.initial.a;
    cpu.b = tc.initial.b;
    cpu.dp = tc.initial.dp;
    cpu.x = tc.initial.x;
    cpu.y = tc.initial.y;
    cpu.cc = tc.initial.cc;
    for &(addr, val) in &tc.initial.ram {
        bus.memory[addr as usize] = val;
    }

    // Execute one instruction
    loop {
        if cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)) {
            break;
        }
    }

    // Assert registers
    assert_eq!(cpu.pc, tc.final_state.pc, "{}: PC", tc.name);
    assert_eq!(cpu.a, tc.final_state.a, "{}: A", tc.name);
    assert_eq!(cpu.b, tc.final_state.b, "{}: B", tc.name);
    assert_eq!(cpu.dp, tc.final_state.dp, "{}: DP", tc.name);
    assert_eq!(cpu.x, tc.final_state.x, "{}: X", tc.name);
    assert_eq!(cpu.y, tc.final_state.y, "{}: Y", tc.name);
    assert_eq!(cpu.u, tc.final_state.u, "{}: U", tc.name);
    assert_eq!(cpu.s, tc.final_state.s, "{}: S", tc.name);
    assert_eq!(cpu.cc, tc.final_state.cc, "{}: CC", tc.name);

    // Assert memory
    for &(addr, expected) in &tc.final_state.ram {
        assert_eq!(
            bus.memory[addr as usize], expected,
            "{}: RAM[0x{:04X}]",
            tc.name, addr
        );
    }

    // Assert cycle count
    assert_eq!(
        bus.cycles.len(),
        tc.cycles.len(),
        "{}: cycle count (got {} expected {})",
        tc.name,
        bus.cycles.len(),
        tc.cycles.len()
    );

    // Assert each cycle
    for (i, ((exp_addr, exp_data, exp_op), actual)) in
        tc.cycles.iter().zip(bus.cycles.iter()).enumerate()
    {
        assert_eq!(actual.addr, *exp_addr, "{}: cycle {} addr", tc.name, i);
        assert_eq!(actual.data, *exp_data, "{}: cycle {} data", tc.name, i);
        let actual_op = match actual.op {
            BusOp::Read => "read",
            BusOp::Write => "write",
        };
        assert_eq!(actual_op, exp_op.as_str(), "{}: cycle {} op", tc.name, i);
    }
}

#[test]
fn test_opcode_86_lda_imm() {
    let json = std::fs::read_to_string("test_data/m6809/86.json").unwrap_or_else(|_| {
        panic!("Missing test data. Run: cargo run -p phosphor-cpu-validation --bin gen_m6809_tests -- 0x86")
    });
    let tests: Vec<TestCase> = serde_json::from_str(&json).unwrap();
    assert!(!tests.is_empty(), "Test file is empty");
    for tc in &tests {
        run_test_case(tc);
    }
}
