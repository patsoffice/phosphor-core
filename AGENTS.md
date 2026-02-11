# AGENTS.md

## Project: Phosphor Core (M6809/6502/Z80 Emulator Framework)

This document provides essential information for AI agents and automated tools working with the Phosphor Core codebase.

### Quick Overview

**Phosphor Core** is a modular, cycle-accurate emulator framework for retro CPUs built in Rust. It uses a trait-based architecture to support multiple CPU types (Motorola 6809, MOS 6502, Zilog Z80) with zero runtime overhead.

**Current Status:**
- M6809: 157/~280 opcodes implemented (56% complete)
- M6502: 1/~151 opcodes implemented (initial)
- Z80: 1/~1582 opcodes implemented (initial)
- 127 integration tests passing
- Focus on educational clarity and correctness over performance

### Repository Structure

```
phosphor-core/
├── Cargo.toml              # [workspace] members = ["core", "machines"]
├── core/                   # phosphor-core crate
│   ├── src/
│   │   ├── core/           # Core abstractions (Bus, Component traits)
│   │   ├── cpu/            # CPU implementations (m6809/, m6502/, z80/)
│   │   │   ├── state.rs     # CpuStateTrait + state structs
│   │   │   ├── m6809/      # M6809 implementation
│   │   │   ├── m6502/       # M6502 implementation
│   │   │   └── z80/         # Z80 implementation
│   │   └── device/         # Peripheral devices (PIA 6820 stub)
│   │   └── lib.rs         # Library exports + prelude
│   └── tests/             # Integration tests (126 total)
│       ├── common/mod.rs   # TestBus harness for direct CPU testing
│       └── m*_test.rs     # CPU-specific test files
├── machines/               # phosphor-machines crate
│   ├── src/
│   │   ├── lib.rs         # Exports Simple6809System, Simple6502System, SimpleZ80System
│   │   ├── simple6809.rs   # M6809 system implementation
│   │   ├── simple6502.rs   # M6502 system implementation
│   │   └── simplez80.rs    # Z80 system implementation
│   └── Cargo.toml         # Machines crate manifest
├── CLAUDE.md             # Development guidelines (REQUIRED READING)
├── README.md              # Project documentation
└── AGENTS.md              # This file - agent guidelines
```

### Critical Development Rules

These are **non-negotiable** development guidelines from CLAUDE.md:

#### Architecture Rules
- **CPU instructions** → `core/src/cpu/m6809/alu.rs` (ALU ops) or `load_store.rs` (load/store)
- **Opcode dispatch** → `core/src/cpu/m6809/mod.rs` `execute_instruction()`
- **Inherent-mode** → `if cycle == 0 { ... }` pattern
- **Immediate-mode** → use `alu_imm()` helper
- **Always** → transition to `ExecState::Fetch` when instruction completes

#### Flag Handling Rules
- **NEVER** use raw hex values (0x01, 0x02, etc.)
- **ALWAYS** use `CcFlag` enum for condition codes
- **MUST** document flag behavior in instruction doc comments
- **Use specific helpers:**
  - `set_flags_arithmetic()` for add/sub
  - `set_flags_logical()` for AND/OR/EOR/TST
  - `set_flags_shift()` for shift/rotate
  - V flag for shift/rotate = N XOR C (post-operation)

### Workspace Development Guidelines

#### Building Workspace
```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build --package phosphor-core
cargo build --package phosphor-machines

# Run all tests
cargo test

# Run specific test category
cargo test m6809_alu_shift_test
```

#### Cross-Crate Dependencies
- **Core crate** can depend on machines for testing (dev-dependencies)
- **Machines crate** depends on core for CPU implementations
- **Never** create circular dependencies

#### File Organization
- **Core functionality** → `core/src/` (CPU implementations, abstractions, devices)
- **System implementations** → `machines/src/` (Simple*System implementations)
- **Tests** → `core/tests/` for CPU tests, `machines/tests/` for system tests

#### Testing Requirements
- **Every new instruction** → MUST have integration tests
- **Tests go in** → `core/tests/m6809_*_test.rs` (grouped by category)
- **Must test** → both A and B register variants
- **Must include** → edge cases: zero, overflow, sign boundary (0x7F/0x80), carry propagation
- **Assertions** → use `CcFlag::X as u8`, not raw hex

### Build & Test Commands

```bash
# Build the project
cargo build

# Run all tests (required before committing)
cargo test

# Run specific test category
cargo test m6809_alu_shift_test
cargo test m6502_basic_test

# Lint and format
cargo fmt
cargo clippy
```

**All tests must pass before committing.** This is a hard requirement.

### Testing Infrastructure

#### TestBus Pattern (Modern)
The project has migrated from Simple*System to direct CPU + TestBus pattern:

```rust
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

#[test]
fn test_instruction() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();
    
    bus.load(0, &[0x86, 0x42]);  // Load instruction bytes
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0)); // Execute cycles
    
    assert_eq!(cpu.a, 0x42);  // Direct field access
    assert_eq!(cpu.pc, 2);
}
```

#### Key TestBus Differences:
- **CPU creation**: `M6809::new()` instead of `Simple6809System::new()`
- **Bus creation**: `TestBus::new()` - separate from CPU
- **Memory loading**: `bus.load(address, data)` instead of `sys.load_rom()`
- **Execution**: `cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0))` instead of `sys.tick()`
- **State access**: Direct field access (`cpu.a`, `cpu.pc`) instead of `sys.get_cpu_state()`

### CPU Implementation Patterns

#### M6809 State Machine
```rust
enum ExecState {
    Fetch,                    // Read next opcode
    Execute(u8, u8),         // Execute opcode at cycle N
    ExecutePage2(u8, u8),    // Execute Page 2 (0x10 prefix)
    Halted { .. },            // TSC/RDY asserted
}
```

#### Instruction Implementation Template
```rust
pub(crate) fn op_instruction<B: Bus<Address=u16, Data=u8> + ?Sized>(
    &mut self, cycle: u8, bus: &mut B, master: BusMaster
) {
    match cycle {
        0 => {
            // First cycle: fetch operand, set up state
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            // ... perform operation ...
            self.set_flags_arithmetic(result, operand_a, operand_b);
            self.state = ExecState::Fetch;
        }
        _ => {}
    }
}
```

### Memory Layout

#### Simple6809System (for reference)
- **RAM**: 0x0000-0x7FFF (32KB)
- **ROM**: 0x8000-0xFFFF (32KB)
- **Vectors**: Reset at 0xFFFE/0xFFFF

#### TestBus
- **Flat 64KB** address space (0x0000-0xFFFF)
- **Direct memory array access** for fast testing
- **No peripherals** - pure CPU testing

### Common Pitfalls & Solutions

#### Borrow Splitting in Systems
The `Simple6809System::tick()` uses controlled `unsafe` for borrow splitting:

```rust
pub fn tick(&mut self) {
    let bus_ptr: *mut Self = self;
    unsafe {
        let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
        self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
    }
}
```

**Why safe:** CPU and Bus access disjoint memory regions.

#### Cycle Count Issues
Tests failing with wrong PC values often need more `tick()` calls:
- LDA immediate = 2 cycles
- STA direct = 3 cycles
- Complex ops = 4+ cycles

#### Flag Assertions
Always use enum values, not magic numbers:
```rust
// ✅ Correct
assert_eq!(cpu.cc & (CcFlag::Z as u8), CcFlag::Z as u8);

// ❌ Wrong
assert_eq!(cpu.cc & 0x02, 0x02);
```

### File Organization

#### CPU Module Structure
```
src/cpu/m6809/
├── mod.rs              # Main CPU struct, state machine, opcode dispatch
├── alu.rs             # ALU helpers and module exports
├── alu/
│   ├── binary.rs       # ADD, SUB, MUL operations
│   ├── shift.rs       # ASL, ASR, LSR, ROL, ROR
│   ├── unary.rs       # NEG, COM, CLR, INC, DEC, TST
│   └── word.rs        # 16-bit operations (ADDD, SUBD, CMPX, etc.)
├── branch.rs          # BRA, BEQ, BNE, BSR, JSR, RTS
├── load_store.rs      # LDA, STA, LDB, STB with immediate/direct modes
├── stack.rs          # PSHS, PULS, PSHU, PULU
└── transfer.rs       # TFR, EXG
```

#### Test File Naming
- `tests/m6809_alu_binary_test.rs` - Binary ALU ops (ADD, SUB, MUL)
- `tests/m6809_alu_shift_test.rs` - Shift/rotate ops
- `tests/m6809_alu_unary_test.rs` - Unary ops (NEG, COM, etc.)
- `tests/m6809_branch_test.rs` - Branch and subroutine ops
- `tests/m6809_direct_test.rs` - Direct addressing mode tests
- `tests/m6502_basic_test.rs` - 6502 basic tests
- `tests/z80_basic_test.rs` - Z80 basic tests

### Commit Message Style

Follow established pattern (see `git log --oneline`):

```
refactor(test): Convert test files to TestBus harness

- Convert m6502_basic_test.rs to use M6502 + TestBus instead of Simple6502System
- Convert z80_basic_test.rs to use Z80 + TestBus instead of SimpleZ80System  
- Fix m6809_alu_shift_test.rs state reference compilation issues
- Update TestBus Bus trait implementation for missing master parameter
- Complete migration from Simple*System pattern to direct CPU + bus testing
```

### Performance Guidelines

#### Design Priorities
1. **Correctness** - Cycle-accurate hardware matching
2. **Clarity** - Educational, maintainable code
3. **Performance** - Fast enough for real-time (future goal)

#### Zero-Cost Principles
- **Generic traits** → compile-time static dispatch
- **No heap allocations** in hot paths
- **Minimal branching** - use pattern matching
- **Cache-friendly** - flat arrays for memory

### API Usage Examples

#### Direct CPU Usage
```rust
use phosphor_core::cpu::m6809::M6809;
use phosphor_core::core::{BusMaster, BusMasterComponent};

let mut cpu = M6809::new();
let mut bus = TestBus::new();
bus.load(0, &[0x86, 0x42]); // LDA #$42

// Execute 2 cycles
cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));

assert_eq!(cpu.a, 0x42);
```

#### System Usage
```rust
use phosphor_core::machine::simple6809::Simple6809System;

let mut sys = Simple6809System::new();
sys.load_rom(0, &[0x86, 0x42, 0x97, 0x10]); // LDA #$42, STA $10

// Execute full program
for _ in 0..5 {
    sys.tick();
}

assert_eq!(sys.get_cpu_state().a, 0x42);
assert_eq!(sys.read_ram(0x10), 0x42);
```

### Current Focus Areas

#### High Priority (What needs help)
- **More 6809 instructions** - JMP, extended addressing, indexed modes
- **Addressing modes** - Indexed, extended, indirect modes
- **Interrupt handling** - IRQ, FIRQ, NMI implementation

#### Medium Priority
- **6502 implementation** - Complete instruction set and addressing modes
- **Z80 implementation** - CB/DD/ED/FD prefixes and alternate registers

#### Low Priority
- **Peripheral devices** - PIA 6820, ACIA 6850, PTM 6840
- **Debugger interface** - Breakpoints, step execution, memory viewer

### Safety Guidelines

#### Controlled Unsafe Usage
- **Split-borrow pattern** in system tick() methods
- **Well-documented invariants** - disjoint memory access
- **Scoped lifetime** - no pointer escape
- **Alternative approaches** considered and rejected for performance

#### Memory Safety
- **No data races** - single-threaded CPU execution
- **Clear ownership** - CPU owns registers, Bus owns memory
- **Trait isolation** - components can't access internals

### Resources for Agents

#### Essential Reading
1. **CLAUDE.md** - Development guidelines (PRIMARY SOURCE)
2. **README.md** - Full project documentation
3. **src/cpu/m6809/mod.rs** - Current instruction dispatch table
4. **tests/common/mod.rs** - TestBus implementation

#### Documentation Links
- [6809 Programmer's Reference](http://www.6809.org.uk/dragon/pdf/6809.pdf)
- [6809 Instruction Set](http://www.8bit-museum.de/6809_isa.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

#### Verification Commands
```bash
# Check project health
cargo test && cargo fmt && cargo clippy

# Check specific CPU implementation
cargo test m6809 -- --nocapture

# Count implemented instructions
grep -r "fn op_" src/cpu/m6809/ | wc -l

# Check test coverage
find tests/ -name "*test.rs" -exec wc -l {} + | tail -1
```

---

**NOTE:** This document complements CLAUDE.md. For detailed implementation rules, always defer to CLAUDE.md as the authoritative source. This AGENTS.md file provides the context and patterns needed for effective automated assistance.