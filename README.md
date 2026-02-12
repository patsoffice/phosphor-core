# Phosphor Emulator

> Core emulation library for the Phosphor retro CPU emulator framework

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-264%20passing-brightgreen.svg)](tests/)

A modular emulator framework for retro CPUs, designed for extensibility and educational purposes. Features a trait-based architecture that allows easy addition of new CPUs, peripherals, and complete systems.

## Project Overview

**Current Focus:** Motorola 6809 CPU emulation

**Status:** 🔨 Early development (262/280 opcodes implemented, 264 tests passing)

### Features

- ✅ **Cycle-accurate execution** - Matches hardware timing exactly
- ✅ **Generic bus architecture** - Supports 8-bit, 16-bit, and 32-bit systems
- ✅ **Explicit state machine** - Transparent, debuggable instruction execution
- ✅ **Zero-cost abstractions** - Trait-based design with no runtime overhead
- ✅ **Comprehensive tests** - Integration tests for all implemented instructions
- 🚧 **Multi-CPU support** - Framework ready, implementations in progress
- 🚧 **Peripheral devices** - Modular component system with placeholders

### What Works Now

- Motorola 6809 CPU with 262 instructions (including ALU, branch, subroutine, stack, transfer, direct-page, indexed, extended, Page 2 and Page 3 ops)
- Condition code flag enum (CcFlag) for readable flag manipulation
- Initial MOS 6502 CPU support (LDA immediate implemented)
- **New:** Initial Zilog Z80 CPU support (LD A, n implemented)
- Simple 6809 system with 32KB RAM + 32KB ROM *(moving to separate crate)*
- DMA arbitration and halt signal support
- Interrupt framework (NMI, IRQ, FIRQ)
- Full test suite (264 integration tests) using direct CPU testing

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Cargo

### Build and Test

```bash
# Clone and build
git clone <repository-url>
cd phosphor-emulator
cargo build

# Run all tests
cargo test

# Expected output:
#   test test_lda_immediate ... ok
#   test test_ld_a_n ... ok
#   test test_load_accumulator_immediate ... ok
#   test test_reset ... ok
#   test test_store_accumulator_direct ... ok
#   test test_addd_immediate ... ok
#   ... (264 tests total)
#   test result: ok. 264 passed; 0 failed
```

### Try It Out

#### Direct CPU + Bus Testing (Primary Approach)

```rust
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

fn main() {
    let mut cpu = M6809::new();
    let mut bus = TestBus::new();

    // Load a simple program: LDA #$42, STA $10
    bus.load(0, &[0x86, 0x42, 0x97, 0x10]);

    // Execute for 5 cycles (enough for both instructions)
    for _ in 0..5 {
        cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    }

    // Check results
    println!("A register: 0x{:02X}", cpu.a);      // 0x42
    println!("Memory[0x10]: 0x{:02X}", bus.memory[0x10]);  // 0x42
    println!("PC: 0x{:04X}", cpu.pc);             // 0x0004
}
```

#### Machine Systems

```rust
use phosphor_machines::Simple6809System;

fn main() {
    let mut sys = Simple6809System::new();

    // Load a simple program: LDA #$42, STA $10
    sys.load_rom(0, &[0x86, 0x42, 0x97, 0x10]);

    // Execute for 5 cycles (enough for both instructions)
    for _ in 0..5 {
        sys.tick();
    }

    // Check results
    let state = sys.get_cpu_state();
    println!("A register: 0x{:02X}", state.a);      // 0x42
    println!("Memory[0x10]: 0x{:02X}", sys.read_ram(0x10));  // 0x42
    println!("PC: 0x{:04X}", state.pc);             // 0x0004
}
```

## Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| **Core Framework** | ✅ Complete | Bus trait, component system, arbitration |
| **M6809 CPU** | ⚠️ Partial | State machine working, 262 instructions |
| **M6502 CPU** | ⚠️ Partial | Initial structure, LDA imm implemented |
| **Z80 CPU** | ⚠️ Partial | Initial structure, LD A, n implemented |
| **PIA 6820** | ❌ Placeholder | Stub only |
| **Simple6809 System** | ⚠️ Moving | RAM/ROM, testing utilities *(migrating to separate crate)* |
| **Test Suite** | ✅ Complete | 264 integration tests passing using direct CPU testing |

### Implemented 6809 Instructions

Currently **262 of ~280** documented 6809 opcodes are implemented (across 3 opcode pages: 217 on page 0, 37 on page 1/0x10, 8 on page 2/0x11):

| Category | Implemented | Examples |
| --- | --- | --- |
| ALU (A) imm | 9 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA |
| ALU (A) direct | 9 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA |
| ALU (A) indexed | 9 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA |
| ALU (A) extended | 9 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA |
| ALU (B) imm | 9 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB |
| ALU (B) direct | 9 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB |
| ALU (B) indexed | 9 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB |
| ALU (B) extended | 9 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB |
| ALU (16-bit) | 12 | ADDD, SUBD, CMPX (immediate + direct + indexed + extended) |
| ALU (Unary) inherent | 13 | MUL, NEG, COM, CLR, INC, DEC, TST (A & B variants) |
| Misc inherent | 4 | NOP, SEX, ABX, DAA |
| CC manipulation | 2 | ORCC, ANDCC (immediate) |
| ALU (Unary) direct | 6 | NEG, COM, CLR, INC, DEC, TST (memory) |
| ALU (Unary) indexed | 6 | NEG, COM, CLR, INC, DEC, TST (memory) |
| ALU (Unary) extended | 6 | NEG, COM, CLR, INC, DEC, TST (memory) |
| Shift/Rotate inherent | 10 | ASL, ASR, LSR, ROL, ROR (A & B variants) |
| Shift/Rotate direct | 5 | ASL, ASR, LSR, ROL, ROR (memory) |
| Shift/Rotate indexed | 5 | ASL, ASR, LSR, ROL, ROR (memory) |
| Shift/Rotate extended | 5 | ASL, ASR, LSR, ROL, ROR (memory) |
| Load/Store imm | 5 | LDA, LDB, LDD, LDX, LDU |
| Load/Store direct | 10 | LDA, LDB, LDD, LDX, LDU, STA, STB, STD, STX, STU |
| Load/Store indexed | 10 | LDA, LDB, LDD, LDX, LDU, STA, STB, STD, STX, STU |
| Load/Store extended | 10 | LDA, LDB, LDD, LDX, LDU, STA, STB, STD, STX, STU |
| LEA | 4 | LEAX, LEAY, LEAS, LEAU |
| Branch | 16 | BRA, BRN, BHI, BLS, BCC, BCS, BNE, BEQ, BVC, BVS, BPL, BMI, BGE, BLT, BGT, BLE |
| Jump/Subroutine | 8 | BSR, JSR (direct/indexed/extended), JMP (direct/indexed/extended), RTS |
| Transfer | 2 | TFR, EXG |
| Stack | 4 | PSHS, PULS, PSHU, PULU |
| Page 2 (0x10) | 37 | CMPD, CMPY, LDY, STY, LDS, STS (imm/direct/indexed/ext), LBRN..LBLE |
| Page 3 (0x11) | 8 | CMPU, CMPS (imm/direct/indexed/extended) |

### Implemented 6502 Instructions

Currently **1 of ~151** documented 6502 opcodes are implemented:

| Category | Implemented | Examples |
| --- | --- | --- |
| Load/Store | 1 | LDA (immediate) |

### Implemented Z80 Instructions

Currently **1 of ~1582** documented Z80 opcodes are implemented:

| Category | Implemented | Examples |
| --- | --- | --- |
| Load/Store | 1 | LD A, n |

## Workspace Architecture

This project uses a **workspace structure** to clearly separate reusable components from system implementations:

### Core Crate (`phosphor-core`)

Contains all reusable components that can be used across different systems:

- CPU implementations (M6809, M6502, Z80)
- Bus and component abstractions  
- Peripheral device interfaces
- CPU state management and testing utilities

### Machines Crate (`phosphor-machines`)

Contains complete system implementations that wire core components together:

- Simple6809System (M6809 + RAM/ROM + PIA)
- Simple6502System (M6502 + flat memory)
- SimpleZ80System (Z80 + flat memory)

### Benefits

- **Clear separation**: Components vs. system-specific logic
- **Reusable core**: Multiple machines can share same CPU implementations
- **Independent development**: Core and machines can evolve separately
- **Clean testing**: CPU tests live in core crate, system tests in machines

## Architecture

### Core Modules

The emulator is organized into four main layers:

#### 1. `core/` - Bus and Component Traits ✅

- **`bus.rs`** - Generic bus interface with master arbitration and interrupt support
  - `Bus` trait with associated types for address/data widths
  - `BusMaster` enum for multi-CPU/DMA arbitration
  - `InterruptState` for interrupt signaling (NMI, IRQ, FIRQ)
- **`component.rs`** - Component lifecycle traits
  - `Component` trait for clocked devices
  - `BusMasterComponent` trait for devices needing bus access

#### 2. `cpu/` - CPU Implementations

- **`m6809/`** ✅ - Motorola 6809 (directory module, split by instruction category)
  - `mod.rs` - Struct, state machine, opcode dispatch table
  - `alu/` - ALU instructions split into `binary`, `unary`, `shift` and `word` modules
  - `branch.rs` - Branch instructions (BRA, BEQ, BNE, etc.)
  - `load_store.rs` - Load/store instructions (immediate + direct modes)
  - All 8 registers (A, B, X, Y, U, S, PC, CC)
  - Explicit state machine (Fetch, Execute, Halted)
  - Cycle-accurate multi-cycle instruction execution
- **`m6502/`** ⚠️ - MOS 6502 (directory module)
  - `mod.rs` - Struct, state machine
  - `load_store.rs` - Load/store instructions
  - Initial implementation (LDA immediate only)
- **`z80/`** ⚠️ - Zilog Z80 (directory module)
  - `mod.rs` - Struct, state machine
  - `load_store.rs` - Load/store instructions
  - Initial implementation (LD A, n only)
- **`mod.rs`** - Generic `Cpu` trait definition with `CpuStateTrait`
- **`state.rs`** ✅ - NEW: CpuStateTrait + M6809State/M6502State/Z80State structs

#### 3. `device/` - Peripheral Devices ❌

- **`pia6820.rs`** - PIA (Peripheral Interface Adapter) stub only

#### 4. `machines/` - Complete System Implementations ✅

Located in separate `phosphor-machines` crate for clean component/system separation.

- **`simple6809.rs`** - Minimal testable 6809 system
  - 32KB RAM (0x0000-0x7FFF)
  - 32KB ROM (0x8000-0xFFFF)
  - PIA6820 peripheral support
- **`simple6502.rs`** - Minimal testable 6502 system
  - 64KB flat memory space
- **`simplez80.rs`** - Minimal testable Z80 system
  - 64KB flat memory space
  - DMA arbitration support
  - Testing utilities (load_program, get_cpu_state)

## Project Structure

```text
phosphor-core/
├── Cargo.toml                      # [workspace] members = ["core", "machines"]
├── core/                           # phosphor-core crate
│   ├── Cargo.toml                  # Core crate manifest
│   ├── src/
│   │   ├── lib.rs                  # Library root, exports core, cpu, device
│   │   ├── core/                   # Core abstractions (complete)
│   │   │   ├── bus.rs              # Bus trait, BusMaster, InterruptState
│   │   │   ├── component.rs        # Component traits
│   │   │   └── mod.rs              # Module exports
│   │   ├── cpu/                    # CPU implementations
│   │   │   ├── mod.rs              # Generic Cpu trait + CpuStateTrait
│   │   │   ├── state.rs            # CpuStateTrait + state structs
│   │   │   ├── m6809/              # Working M6809 (262 opcodes)
│   │   │   │   ├── mod.rs          # Struct, state machine, dispatch
│   │   │   │   ├── alu.rs          # ALU helpers and module exports
│   │   │   │   ├── binary.rs       # Binary ops (ADD, SUB, MUL, etc.)
│   │   │   │   ├── shift.rs        # Shift/Rotate ops
│   │   │   │   ├── unary.rs        # Unary ops (NEG, COM, etc.)
│   │   │   │   └── word.rs         # 16-bit ops (ADDD, CMPX, LDD, etc.)
│   │   │   ├── branch.rs           # Branch/subroutine ops (BRA, BEQ, BSR, JSR, RTS, etc.)
│   │   │   ├── load_store.rs       # LDA, LDB, STA
│   │   │   ├── stack.rs            # Stack ops (PSHS, PULS, PSHU, PULU)
│   │   │   └── transfer.rs         # Transfer/exchange (TFR, EXG)
│   │   ├── m6502/                  # Initial implementation
│   │   │   ├── mod.rs              # Struct, state machine
│   │   │   └── load_store.rs       # LDA immediate
│   │   └── z80/                    # Initial implementation
│   │       ├── mod.rs              # Struct, state machine
│   │       └── load_store.rs       # LD A, n
│   │   └── device/                 # Peripheral devices
│   │       ├── pia6820.rs          # PIA6820 stub
│   │       └── mod.rs              # Module exports
│   └── tests/                    # Integration tests
│       ├── common/mod.rs           # Direct CPU testing harness
│       ├── m6502_basic_test.rs     # Basic 6502 tests (1 test)
│       ├── z80_basic_test.rs       # Basic Z80 tests (1 test)
│       ├── m6809_*_test.rs       # M6809 tests (14 files, 231 tests)
│       └── target/                # Build artifacts (gitignored)
└── machines/                        # phosphor-machines crate
    ├── Cargo.toml                  # Machines crate manifest
    └── src/
        ├── lib.rs                  # Exports Simple6809System, Simple6502System, SimpleZ80System
        ├── simple6809.rs           # Minimal 6809 system with RAM/ROM
        ├── simple6502.rs           # Minimal 6502 system
        └── simplez80.rs            # Minimal Z80 system
```

## How It Works

### Execution Model

The emulator uses a **cycle-accurate, state-machine-based** execution model:

```rust
// State machine in M6809
enum ExecState {
    Fetch,                          // Read next opcode
    Execute(u8, u8),                // Execute opcode at cycle N
    ExecutePage2(u8, u8),           // Execute Page 2 (0x10 prefix) opcode
    Halted { .. },                  // TSC/RDY asserted
}
```

**Execution flow for `LDA #$42`** (opcode 0x86):

```text
Cycle 0 (Fetch):  Read 0x86 from memory[PC=0] → PC=1, state=Execute(0x86, 0)
Cycle 1 (Exec 0): Read 0x42 from memory[PC=1] → A=0x42, PC=2, state=Fetch
Cycle 2 (Fetch):  Read next opcode...
```

Each `tick()` advances exactly **one CPU cycle**, matching hardware timing.

### Bus Architecture

The generic `Bus` trait enables different CPUs with zero runtime overhead:

```rust
pub trait Bus {
    type Address: Copy + Into<u64>;  // u16 for 6809, u32 for 68000
    type Data;                       // u8 or u16

    fn read(&mut self, master: BusMaster, addr: Self::Address) -> Self::Data;
    fn write(&mut self, master: BusMaster, addr: Self::Address, data: Self::Data);

    // Arbitration: CPU must check before each cycle
    fn is_halted_for(&self, master: BusMaster) -> bool;

    // Interrupt polling at instruction boundaries
    fn check_interrupts(&self, target: BusMaster) -> InterruptState;
}
```

**Key features:**

- `BusMaster` enum identifies bus requestor (CPU 0, CPU 1, DMA)
- Supports TSC/RDY/BUSREQ halt signals via `is_halted_for()`
- Generic interrupt delivery via `InterruptState` (NMI, IRQ, FIRQ)
- Associated types = compile-time polymorphism, no vtable overhead

### Component Interface

Two traits for different device types:

```rust
// Simple devices (timers, sound chips)
pub trait Component {
    fn tick(&mut self) -> bool;  // Returns true at significant events
    fn clock_divider(&self) -> u64 { 1 }  // For clock domain crossing
}

// Devices that access the bus (CPUs, DMA)
pub trait BusMasterComponent: Component {
    type Bus: Bus + ?Sized;
    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool;
}
```

This separation allows video chips to tick without bus access, while CPUs get explicit bus references.

### Using the Emulator

#### Direct CPU + Bus Testing (Primary Approach)

The project uses a **TestBus** pattern for direct CPU testing:

```rust
use phosphor_core::core::{BusMaster, BusMasterComponent};
use phosphor_core::cpu::m6809::M6809;
mod common;
use common::TestBus;

let mut cpu = M6809::new();
let mut bus = TestBus::new();

// Load code into memory
bus.load(0, &[
    0x86, 0x42,  // LDA #$42
    0x97, 0x10,  // STA $10
]);

// Execute cycle-by-cycle
for cycle in 0..5 {
    cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
    println!("Cycle {}: PC=0x{:04X}", cycle, cpu.pc);
}

// Verify results
assert_eq!(cpu.a, 0x42);
assert_eq!(bus.memory[0x10], 0x42);
assert_eq!(cpu.pc, 0x04);
```

#### Legacy SimpleSystem Approach

The `Simple6809System` still provides a complete testable environment:

```rust
use phosphor_core::machine::simple6809::Simple6809System;

let mut sys = Simple6809System::new();

// Load code into memory (0x0000 is RAM)
sys.load_rom(0, &[
    0x86, 0x42,  // LDA #$42
    0x97, 0x10,  // STA $10
]);

// Execute cycle-by-cycle
for cycle in 0..5 {
    sys.tick();
    println!("Cycle {}: PC=0x{:04X}", cycle, sys.get_cpu_state().pc);
}

// Verify results
assert_eq!(sys.get_cpu_state().a, 0x42);
assert_eq!(sys.read_ram(0x10), 0x42);
assert_eq!(sys.get_cpu_state().pc, 0x04);
```

**Output:**

```text
Cycle 0: PC=0x0001  (fetched LDA opcode)
Cycle 1: PC=0x0002  (executed LDA, loaded A)
Cycle 2: PC=0x0003  (fetched STA opcode)
Cycle 3: PC=0x0004  (fetched address)
Cycle 4: PC=0x0004  (stored A to memory, back to Fetch)
```

## Roadmap

### Phase 1: Complete 6809 CPU (Current Focus)

- [x] Arithmetic instructions (ADDA, SUBA, ADCA, SBCA, CMPA, MUL + B variants)
- [x] Logical instructions (AND, OR, EOR, BIT, COM + B variants)
- [x] Unary instructions (NEG, CLR, INC, DEC, TST + B variants)
- [x] Shift/rotate instructions (ASL, ASR, LSR, ROL, ROR + B variants)
- [x] Branch instructions (BRA, BRN, BHI, BLS, BCC, BCS, BNE, BEQ, BVC, BVS, BPL, BMI, BGE, BLT, BGT, BLE)
- [x] Jump/call instructions (BSR, JSR direct, RTS)
- [x] Stack operations (PSHS, PULS, PSHU, PULU)
- [x] Transfer/exchange (TFR, EXG)
- [x] Direct page (DP) addressing mode
- [x] All addressing modes (indexed, extended)
- [x] Condition code (CC) flag enum (CcFlag)
- [x] 16-bit operations (CMPU, CMPS — Page 3/0x11 prefix)
  - [x] LDD, LDX, LDU, STD, STX, STU, ADDD, SUBD, CMPX
  - [x] CMPD, CMPY, LDY, STY, LDS, STS (Page 2/0x10 prefix)
  - [x] CMPU, CMPS (Page 3/0x11 prefix)
- [x] Misc operations (NOP, SEX, ABX, DAA, ORCC, ANDCC)
- [x] Long conditional branches (LBRN..LBLE via Page 2/0x10 prefix)

**Progress:** 262/~280 opcodes implemented (93.6%)

### Phase 2: Core Infrastructure

- [ ] Move SimpleSystem components to separate crate
- [ ] Interrupt handling (IRQ, FIRQ, NMI)
- [ ] Reset vector fetch from 0xFFFE/0xFFFF
- [ ] CWAI and SYNC instructions
- [ ] Cycle-accurate timing validation
- [ ] Instruction disassembler
- [ ] Save state support

### Phase 3: Additional CPUs

- [ ] MOS 6502 CPU
  - [ ] 6502 addressing modes
  - [ ] 6502 instruction set
  - [ ] BCD arithmetic mode
- [ ] Zilog Z-80 CPU
  - [ ] 8-bit data bus, 16-bit address space
  - [ ] Instruction prefixes (CB, DD, ED, FD)
  - [ ] Alternate register set
- [ ] Motorola 68000 CPU
  - [ ] 32-bit address space
  - [ ] 16-bit data bus
  - [ ] Privilege levels

### Phase 4: Peripherals & Systems

- [ ] 6820 PIA (Peripheral Interface Adapter)
- [ ] 6850 ACIA (serial communication)
- [ ] 6840 PTM (timer)
- [ ] Memory mappers and bank switching
- [ ] Real system implementations:
  - [ ] Arcade boards (Williams games first)

### Phase 5: Developer Tools

- [ ] Debugger with breakpoints
- [ ] Step execution and register inspection
- [ ] Memory viewer/editor
- [ ] Disassembly viewer
- [ ] Performance profiler
- [ ] Code coverage for tested instructions

### Phase 6: Multimedia

- [ ] Video display simulation
- [ ] Sprite/tile rendering
- [ ] Sound chip emulation (AY-3-8910, SN76489)
- [ ] Audio output
- [ ] Input handling (keyboard, joystick)

## Design Decisions

### Generic Bus with Associated Types

The `Bus` trait uses associated types rather than generic parameters:

```rust
pub trait Bus {
    type Address: Copy + Into<u64>;
    type Data;
    // ...
}
```

**Why?** This allows:

- Different CPUs to define their own address/data widths
- Zero runtime overhead (no dynamic dispatch for reads/writes)
- Bus implementations to be stored as trait objects when needed
- Type safety: can't accidentally mix u16 and u32 addresses

### Explicit State Machine

CPU execution uses an explicit `ExecState` enum instead of implicit counters:

```rust
enum ExecState {
    Fetch,
    Execute(u8, u8),  // opcode, cycle
    Halted { return_state: Box<ExecState>, saved_cycle: u8 },
}
```

**Why?** This makes:

- Multi-cycle instruction execution transparent and debuggable
- Halt states (TSC, WAIT) explicit in the type system
- State transitions visible in code rather than implicit
- Easier to implement save states and debugging

### Modular Trait-Based Architecture

All major components (Bus, Cpu, Component) are traits:

**Why?** This enables:

- Testing CPUs without a full system (mock buses)
- Multiple CPU implementations behind a single interface
- Easy addition of new peripherals and systems
- Composition over inheritance (Rust idiom)

### Controlled Unsafe for Borrow Splitting

The `Simple6809System::tick()` method uses a carefully controlled `unsafe` block:

```rust
pub fn tick(&mut self) {
    let bus_ptr: *mut Self = self;
    unsafe {
        let bus = &mut *bus_ptr as &mut dyn Bus<Address = u16, Data = u8>;
        self.cpu.execute_cycle(bus, BusMaster::Cpu(0));
    }
}
```

**Why is this necessary?**

- The CPU needs `&mut self` to modify its registers
- The CPU also needs `&mut Bus` to read/write memory
- But `Simple6809System` *is* the bus (implements `Bus` trait)
- Rust's borrow checker sees this as two mutable borrows of `self`

**Why is this safe?**

- The CPU only accesses its own fields (`cpu.a`, `cpu.pc`, etc.)
- The Bus trait only accesses system fields (`ram`, `rom`, `pia`)
- These are **disjoint memory regions** - no aliasing occurs
- The raw pointer doesn't outlive the function (scoped)
- This is a known pattern for "split borrowing" structs

**Alternative approaches considered:**

- ❌ `RefCell` - Runtime borrow checking adds overhead
- ❌ Separate `System` and `Bus` structs - more boilerplate
- ❌ Interior mutability everywhere - less idiomatic
- ✅ Unsafe split borrow - zero cost, clear invariants

## Troubleshooting

### Build Issues

**Problem:** Compilation errors about trait bounds

```text
error[E0277]: the trait bound `dyn Bus<Address = u16, Data = u8>: Sized` is not satisfied
```

**Solution:** Ensure trait objects use `?Sized` bound:

```rust
impl BusMasterComponent for M6809 {
    type Bus = dyn Bus<Address = u16, Data = u8>;  // Note: trait object
}
```

**Problem:** Borrow checker errors when implementing new systems

**Solution:** Use the split-borrow pattern with controlled `unsafe` (see Design Decisions)

### Test Failures

**Problem:** Test fails with wrong PC value

```text
thread 'test_load_accumulator_immediate' panicked at 'assertion failed: `(left == right)`
  left: `1`,
 right: `2`', tests/m6809_load_store_test.rs:16:5
```

**Solution:** Check cycle count - you may need more `tick()` calls. Each instruction takes 2-4 cycles.

**Problem:** Memory doesn't contain expected value

**Solution:** Verify instruction execution order and ensure enough cycles for all instructions to complete.

### Runtime Issues

**Problem:** Infinite loop - emulator never completes

**Solution:** The 6809 doesn't auto-halt. Implement a halt instruction or limit cycle count:

```rust
for _ in 0..100 { sys.tick(); }  // Limit execution
```

## Contributing

This is an educational emulator project. We welcome contributions!

### How to Contribute

1. **Adding 6809 Instructions**

   - Add an `op_*` method in appropriate submodule (`alu/binary.rs`, `alu/word.rs`, `branch.rs`, `load_store.rs`, etc.)
   - Add dispatch entry in `src/cpu/m6809/mod.rs::execute_instruction()`
   - Implement cycle-accurate execution (use match on `cycle`)
   - Add integration test in matching `tests/m6809_*_test.rs` file using direct CPU testing

   Example (adding a method in `alu.rs`):

   ```rust
   pub(crate) fn op_anda_imm<B: Bus<Address=u16, Data=u8> + ?Sized>(
       &mut self, cycle: u8, bus: &mut B, master: BusMaster
   ) {
       match cycle {
           0 => {
               let operand = bus.read(master, self.pc);
               self.pc = self.pc.wrapping_add(1);
               self.a &= operand;
               self.set_flag(CcFlag::N, self.a & 0x80 != 0);
               self.set_flag(CcFlag::Z, self.a == 0);
               self.set_flag(CcFlag::V, false);
               self.state = ExecState::Fetch;
           }
           _ => {}
       }
   }
   ```

2. **Implementing New CPUs**

   - Create a new module directory in `src/cpu/` (e.g., `m6502/`)
   - Implement `Component`, `BusMasterComponent`, and `Cpu` traits
   - Define registers and state machine
   - Add module export in `src/cpu/mod.rs`
   - Create test system in `src/machine/`

3. **Adding Peripherals**

   - Create device in `src/device/`
   - Implement `Component` trait
   - If needs bus access, implement `BusMasterComponent`
   - Add device to appropriate system in `src/machine/`
   - Write integration tests

4. **Testing Guidelines**

   - All new instructions MUST have integration tests
   - Use direct CPU testing pattern (M6809 + TestBus) for new tests
   - Tests should verify registers, memory, PC, and condition codes
   - Use descriptive test names: `test_<instruction>_<addressing_mode>`
   - Include edge cases (zero, negative, overflow)

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Run clippy before submitting (`cargo clippy`)
- Document public APIs with rustdoc comments
- Keep `unsafe` minimal and well-documented
- Use meaningful variable names (no single letters except registers)

### Areas Needing Help

- 🔴 **High Priority:** Remaining 6809 instructions (SWI variants, RTI, CWAI, SYNC, interrupt handling)
- 🟡 **Medium Priority:** 6502 CPU implementation
- 🟡 **Medium Priority:** Z80 CPU implementation
- 🟢 **Low Priority:** Peripheral devices
- 🟢 **Low Priority:** Debugger interface

## Performance Notes

### Design Priorities

1. **Correctness** - Cycle-accurate emulation matching hardware behavior
2. **Clarity** - Readable, maintainable code for educational purposes
3. **Performance** - Fast enough for real-time emulation (future goal)

### Current Performance Characteristics

- **Zero-cost abstractions** - Generic traits compile to static dispatch
- **No heap allocations** in hot paths (instruction execution)
- **Minimal branching** - State machine uses pattern matching
- **Cache-friendly** - Flat arrays for RAM/ROM
- **No unsafe overhead** - Unsafe block is compile-time only

### Benchmarks (Future Work)

Once more instructions are implemented, we'll benchmark:

- Cycles per second on modern hardware
- Comparison to reference emulators
- Optimization opportunities

**Target:** 10MHz+ emulated speed on modern CPU (10x faster than original 1MHz 6809)

## FAQ

**Q: Why Rust for an emulator?**

A: Rust provides zero-cost abstractions, memory safety, and excellent performance - ideal for cycle-accurate emulation without sacrificing clarity. Besides, I want to improve my Rust abilities.

**Q: Can this run commercial ROMs?**

A: Not yet. Only 262 instructions are implemented. This is an educational project in early development.

**Q: Why use `unsafe` in an emulator?**

A: The split-borrow pattern is necessary for the CPU to access both itself and the bus. See Design Decisions for safety invariants.

**Q: Will this support debugger features?**

A: Yes, planned in Phase 5. The explicit state machine makes breakpoints and step execution straightforward.

**Q: Can I use this as a library?**

A: Yes! Add to `Cargo.toml`:

```toml
[dependencies]
phosphor-core = { path = "../phosphor-core" }
```

Then use the prelude: `use phosphor_core::prelude::*;`

**Q: How accurate is the timing?**

A: Cycle-accurate for implemented instructions. Each `tick()` = 1 CPU cycle = matching hardware timing.

## License

This project is licensed under the [MIT License](LICENSE).

**Note:** This is a learning/reference implementation. Not affiliated with any hardware manufacturer.

## Resources

### 6809 Documentation

- [6809 Programmer's Reference](http://www.6809.org.uk/dragon/pdf/6809.pdf) - Official Motorola datasheet
- [6809 Instruction Set](http://www.8bit-museum.de/6809_isa.html) - Complete opcode reference
- [Motorola 6809 Wikipedia](https://en.wikipedia.org/wiki/Motorola_6809) - Architecture overview
- [6809 Assembly Language](http://www.6809.org.uk/) - Programming guides

### 6502 Documentation

- [6502.org](http://www.6502.org/) - The 6502 Microprocessor Resource
- [6502 Instruction Set](http://www.6502.org/tutorials/6502opcodes.html) - Opcode reference
- [MOS Technology 6502 Wikipedia](https://en.wikipedia.org/wiki/MOS_Technology_6502) - Architecture overview

### Z80 Documentation

- [Zilog Z80 User Manual](http://www.zilog.com/docs/z80/um0080.pdf) - Official User Manual
- [Z80 Instruction Set](http://z80-heaven.wikidot.com/instructions-set) - Opcode reference
- [Zilog Z80 Wikipedia](https://en.wikipedia.org/wiki/Zilog_Z80) - Architecture overview
- [Thomas Scherrer Z80-Family](http://www.z80.info/) - Comprehensive Z80 resources

### Rust Resources

- [Rust Book - Unsafe Code](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)

### Emulator Development

- [Emulator 101](http://www.emulator101.com/) - Writing your first emulator
- [How to Write a Computer Emulator](https://fms.komkon.org/EMUL8/HOWTO.html)
- [NES Emulator Guide](https://bugzmanov.github.io/nes_ebook/) - Similar architecture principles

### Other 6809 Emulators

- [XRoar](http://www.6809.org.uk/xroar/) - Dragon/CoCo emulator (C)
- [MAME 6809 Core](https://github.com/mamedev/mame/tree/master/src/devices/cpu/m6809) - Reference implementation
