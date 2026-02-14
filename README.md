# Phosphor Emulator

> Core emulation library for the Phosphor retro CPU emulator framework

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-561%20passing-brightgreen.svg)](core/tests/)

A modular emulator framework for retro CPUs, designed for extensibility and educational purposes. Features a trait-based architecture that allows easy addition of new CPUs, peripherals, and complete systems.

**Current Focus:** Joust (1982) arcade board emulation — M6809 CPU (285 opcodes), M6800 CPU (66 opcodes), MC6821 PIA, Williams SC1 blitter, CMOS RAM, Machine trait for frontend abstraction. 561 tests passing, 266,000 cross-validated test vectors across 266 M6809 opcodes

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Cargo
- SDL2 (`brew install sdl2` on macOS)

### Build and Test

```bash
# Clone and build
git clone <repository-url>
cd phosphor-emulator
cargo build

# Run all tests
cargo test

# Expected output:
#   test result: ok. 561 passed; 0 failed
```

### Running the Emulator

```bash
# MAME-style rompath (directory containing joust.zip)
cargo run --package phosphor-frontend -- joust /path/to/roms --scale 3

# Direct ZIP file
cargo run --package phosphor-frontend -- joust /path/to/joust.zip --scale 3

# Extracted ROM directory (backward compatible)
cargo run --package phosphor-frontend -- joust /path/to/extracted/roms --scale 3
```

ROMs are matched by CRC32 checksum, so any MAME ROM naming convention works. All three Joust label variants are supported: Green (parent), Yellow, and Red.

**Controls:**

| Key              | Action      |
| ---------------- | ----------- |
| Arrow Left/Right | P1 Move     |
| Space            | P1 Flap     |
| 1                | P1 Start    |
| A/D              | P2 Move     |
| W                | P2 Flap     |
| 2                | P2 Start    |
| 5                | Insert Coin |
| Escape           | Quit        |

> `.cargo/config.toml` sets the Homebrew library path for aarch64-apple-darwin automatically, so no manual `LIBRARY_PATH` is needed.

## Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| **Core Framework** | Complete | Bus trait, Machine trait, component system, arbitration |
| **M6809 CPU** | Complete | 285 opcodes, cycle-accurate, all addressing modes |
| **M6800 CPU** | In Progress | 66 opcodes: inherent, immediate ALU, shifts/rotates, addressing mode helpers |
| **M6502 CPU** | Partial | Initial structure, LDA immediate only |
| **Z80 CPU** | Partial | Initial structure, LD A, n only |
| **MC6821 PIA** | Complete | Full register set, interrupts, edge detection, control lines |
| **Williams SC1 Blitter** | Complete | DMA block copy/fill, mask, shift, foreground-only modes |
| **CMOS RAM** | Complete | 1KB battery-backed RAM with save/load persistence |
| **ROM Loader** | Complete | MAME ZIP support, CRC32-based ROM matching, multi-variant support |
| **Joust System** | Complete | Williams board: CPU + video RAM + PIAs + blitter + CMOS + ROM |
| **Machine Trait** | Complete | Frontend-agnostic interface: display, input, render, reset |
| **CPU Validation** | Complete | M6809: 266K vectors vs elmerucr/MC6809 (100%), M6800: 192K vectors vs mame4all (99.998%) |
| **Test Suite** | Complete | 561 tests across core, devices, and machine integration |

### 6809 Instructions

285 opcodes across 3 pages (238 page 0, 38 page 1/0x10, 9 page 2/0x11):

| Category | Count | Details |
| --- | --- | --- |
| ALU (A register) | 9 | ADDA, SUBA, CMPA, SBCA, ADCA, ANDA, BITA, EORA, ORA — imm/direct/indexed/extended |
| ALU (B register) | 9 | ADDB, SUBB, CMPB, SBCB, ADCB, ANDB, BITB, EORB, ORB — imm/direct/indexed/extended |
| ALU (16-bit) | 3 | ADDD, SUBD, CMPX — imm/direct/indexed/extended |
| Unary (inherent) | 13 | NEG, COM, CLR, INC, DEC, TST (A & B), MUL |
| Unary (memory) | 6 | NEG, COM, CLR, INC, DEC, TST — direct/indexed/extended |
| Shift/Rotate (inherent) | 10 | ASL, ASR, LSR, ROL, ROR (A & B) |
| Shift/Rotate (memory) | 5 | ASL, ASR, LSR, ROL, ROR — direct/indexed/extended |
| Load/Store | 5 imm + 10 per mode | LDA, LDB, LDD, LDX, LDU, STA, STB, STD, STX, STU |
| LEA | 4 | LEAX, LEAY, LEAS, LEAU |
| Branch | 16 | BRA, BRN, BHI, BLS, BCC, BCS, BNE, BEQ, BVC, BVS, BPL, BMI, BGE, BLT, BGT, BLE |
| Jump/Subroutine | 10 | BSR, LBRA, LBSR, JSR, JMP, RTS |
| Transfer/Stack | 6 | TFR, EXG, PSHS, PULS, PSHU, PULU |
| Interrupt | 4 | SWI, RTI, CWAI, SYNC |
| Misc inherent | 6 | NOP, SEX, ABX, DAA, ORCC, ANDCC |
| Page 2 (0x10) | 38 | Long branches, CMPD, CMPY, LDY, STY, LDS, STS, SWI2 |
| Page 3 (0x11) | 9 | CMPU, CMPS, SWI3 |
| Undocumented | 15 | Aliases for compatibility |

## Workspace Architecture

This project uses a **workspace structure** to separate reusable components from system implementations:

### Core Crate (`phosphor-core`)

Contains all reusable components — zero external dependencies:

- CPU implementations (M6800, M6809, M6502, Z80)
- Bus and component abstractions
- Machine trait (frontend-agnostic display/input/render interface)
- Peripheral devices (MC6821 PIA, Williams SC1 blitter, CMOS RAM)

### Machines Crate (`phosphor-machines`)

Complete system implementations that wire core components together:

- **JoustSystem** — Williams arcade board (M6809 + 48KB video RAM + two PIAs + blitter + CMOS + 12KB ROM)
- Simple6800System (M6800 + RAM/ROM)
- Simple6809System (M6809 + RAM/ROM + PIA)
- Simple6502System (M6502 + flat memory)
- SimpleZ80System (Z80 + flat memory)

### Frontend Crate (`phosphor-frontend`)

SDL2-based windowed frontend — external dependencies: SDL2, zip:

- **Machine-agnostic** — operates entirely through the `Machine` trait, no hardware-specific knowledge
- **ROM path resolution** — loads from MAME ZIP files, rompath directories, or extracted loose files
- SDL2 window with GPU-scaled texture rendering (VSync frame timing)
- Keyboard input mapping built automatically from `Machine::input_map()`
- Adding a new machine requires only a new match arm in `main.rs`

### CPU Validation Crate (`phosphor-cpu-validation`)

[SingleStepTests](https://github.com/SingleStepTests/65x02)-style test infrastructure for validating CPU implementations against randomized test vectors with cycle-by-cycle bus traces. Cross-validates against independent reference emulators to catch flag, timing, and behavioral bugs.

- **M6809** — 266 opcodes, 266,000 test vectors, cross-validated against [elmerucr/MC6809](https://github.com/elmerucr/MC6809). See [cpu-validation/README_6809.md](cpu-validation/README_6809.md).
- **M6800** — 192 opcodes, 192,000 test vectors, cross-validated against [mame4all](https://github.com/mamedev/mame) M6800. See [cpu-validation/README_6800.md](cpu-validation/README_6800.md).

### Cross-Validation (`cross-validation/`)

C++ harnesses that validate phosphor-core's test vectors against independent reference emulators. Compares registers, memory, and cycle counts.

- **M6809** — 266,000/266,000 tests pass (100%)
- **M6800** — 191,996/192,000 tests pass (99.998%)

## Project Structure

```text
phosphor-core/
├── Cargo.toml                      # [workspace] members = ["core", "machines", "cpu-validation", "frontend"]
├── .cargo/config.toml              # Homebrew library path for aarch64-apple-darwin
├── core/                           # phosphor-core crate
│   ├── Cargo.toml                  # Core crate manifest (zero dependencies)
│   ├── src/
│   │   ├── lib.rs                  # Library root, exports core, cpu, device
│   │   ├── core/                   # Core abstractions (complete)
│   │   │   ├── bus.rs              # Bus trait, BusMaster, InterruptState
│   │   │   ├── component.rs        # Component traits
│   │   │   ├── machine.rs          # Machine trait, InputButton (frontend interface)
│   │   │   └── mod.rs              # Module exports
│   │   ├── cpu/                    # CPU implementations
│   │   │   ├── mod.rs              # Generic Cpu trait + CpuStateTrait
│   │   │   ├── state.rs            # CpuStateTrait + state structs
│   │   │   ├── m6800/              # M6800 CPU (66 opcodes, in progress)
│   │   │   │   ├── mod.rs          # Struct, state machine, dispatch
│   │   │   │   ├── alu.rs          # ALU helpers and module exports
│   │   │   │   ├── alu/
│   │   │   │   │   ├── binary.rs   # ADD, SUB, CMP, AND, OR, EOR, BIT, ADC, SBC
│   │   │   │   │   ├── shift.rs    # ASL, ASR, LSR, ROL, ROR
│   │   │   │   │   └── unary.rs    # NEG, COM, CLR, INC, DEC, TST
│   │   │   │   ├── branch.rs       # Branch/subroutine ops
│   │   │   │   ├── load_store.rs   # Load/store ops
│   │   │   │   └── stack.rs        # Stack ops + interrupts
│   │   │   ├── m6809/              # Working M6809 (285 opcodes)
│   │   │   │   ├── mod.rs          # Struct, state machine, dispatch
│   │   │   │   ├── alu.rs          # ALU helpers and module exports
│   │   │   │   ├── binary.rs       # Binary ops (ADD, SUB, MUL, etc.)
│   │   │   │   ├── shift.rs        # Shift/Rotate ops
│   │   │   │   ├── unary.rs        # Unary ops (NEG, COM, etc.)
│   │   │   │   └── word.rs         # 16-bit ops (ADDD, CMPX, LDD, etc.)
│   │   │   ├── branch.rs           # Branch/subroutine ops
│   │   │   ├── load_store.rs       # LDA, LDB, STA
│   │   │   ├── stack.rs            # Stack ops + interrupts (SWI/RTI/CWAI/SYNC)
│   │   │   └── transfer.rs         # Transfer/exchange (TFR, EXG)
│   │   ├── m6502/                  # Initial implementation
│   │   │   ├── mod.rs              # Struct, state machine
│   │   │   └── load_store.rs       # LDA immediate
│   │   └── z80/                    # Initial implementation
│   │       ├── mod.rs              # Struct, state machine
│   │       └── load_store.rs       # LD A, n
│   │   └── device/                 # Peripheral devices
│   │       ├── pia6820.rs          # MC6821 PIA (full: registers, interrupts, edge detection)
│   │       ├── williams_blitter.rs # Williams SC1 DMA blitter (copy/fill/shift/mask)
│   │       ├── cmos_ram.rs         # 1KB battery-backed CMOS RAM
│   │       └── mod.rs              # Module exports
│   └── tests/                      # Integration tests (561 tests)
│       ├── common/mod.rs           # TestBus harness
│       ├── m6800_*_test.rs         # M6800 tests (3 files, 164 tests)
│       ├── m6809_*_test.rs         # M6809 tests (16 files)
│       ├── pia6820_test.rs         # MC6821 PIA tests (28 tests)
│       ├── williams_blitter_test.rs # Blitter tests (39 tests)
│       ├── m6502_basic_test.rs     # Basic 6502 tests
│       └── z80_basic_test.rs       # Basic Z80 tests
├── machines/                       # phosphor-machines crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                  # Exports system types
│   │   ├── joust.rs                # Joust arcade board (Williams 2nd-gen)
│   │   ├── rom_loader.rs           # ROM loading with CRC32 matching, multi-variant support
│   │   ├── simple6800.rs           # M6800 + RAM/ROM
│   │   ├── simple6809.rs           # M6809 + RAM/ROM
│   │   ├── simple6502.rs           # M6502 + flat memory
│   │   └── simplez80.rs            # Z80 + flat memory
│   └── tests/
│       └── joust_test.rs           # Joust system integration tests (39 tests)
├── cpu-validation/                 # phosphor-cpu-validation crate
│   ├── Cargo.toml                  # Deps: phosphor-core, serde, rand
│   ├── README_6809.md              # M6809 cross-validation details
│   ├── README_6800.md              # M6800 cross-validation details & MAME differences
│   ├── src/
│   │   ├── lib.rs                  # TracingBus + JSON types
│   │   └── bin/
│   │       ├── gen_m6809_tests.rs  # M6809 test vector generator
│   │       └── gen_m6800_tests.rs  # M6800 test vector generator
│   ├── tests/
│   │   ├── m6809_single_step_test.rs  # Validates M6809 against JSON
│   │   └── m6800_single_step_test.rs  # Validates M6800 against JSON
│   └── test_data/
│       ├── m6809/                  # Generated M6809 test vectors (266 files)
│       └── m6800/                  # Generated M6800 test vectors (192 files)
├── frontend/                       # phosphor-frontend crate (SDL2 frontend)
│   ├── Cargo.toml                  # Deps: phosphor-core, phosphor-machines, sdl2, zip
│   └── src/
│       ├── main.rs                 # CLI args, machine selection, ROM loading
│       ├── rom_path.rs             # ROM path resolution (ZIP, rompath, directory)
│       ├── emulator.rs             # Main loop: tick, render, input, frame timing
│       ├── video.rs                # SDL window/texture setup, framebuffer blit
│       └── input.rs                # Keyboard → Machine::set_input() mapping
└── cross-validation/               # C++ reference validation
    ├── Makefile
    ├── validate.cpp                # M6809 harness using elmerucr/MC6809
    ├── validate_m6800.cpp          # M6800 harness using mame4all
    ├── mc6809/                     # Git submodule: elmerucr/MC6809
    ├── m6800/                      # mame4all M6800 CPU core + shim
    └── include/nlohmann/json.hpp   # Single-header JSON parser
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
    ExecutePage3(u8, u8),           // Execute Page 3 (0x11 prefix) opcode
    Halted { .. },                  // TSC/RDY asserted
    Interrupt(u8),                  // Hardware interrupt response sequence
    WaitForInterrupt,               // CWAI wait state
    SyncWait,                       // SYNC wait state
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

**Output:**

```text
Cycle 0: PC=0x0001  (fetched LDA opcode)
Cycle 1: PC=0x0002  (executed LDA, loaded A)
Cycle 2: PC=0x0003  (fetched STA opcode)
Cycle 3: PC=0x0004  (fetched address)
Cycle 4: PC=0x0004  (stored A to memory, back to Fetch)
```

## Roadmap

### Phase 1: M6809 CPU ✅

285 opcodes implemented (280 documented + 15 undocumented aliases), integration tests passing. Complete hardware interrupt handling (NMI, FIRQ, IRQ), CWAI, SYNC. Cycle-accurate timing validated against independent reference emulator.

### Phase 2: Core Infrastructure

- [x] Interrupt handling (IRQ, FIRQ, NMI)
- [x] CWAI and SYNC instructions
- [x] Move SimpleSystem components to separate crate
- [x] Cycle-accurate timing validation (M6809: 266K tests vs elmerucr/MC6809, M6800: 192K tests vs mame4all)
- [ ] Reset vector fetch from 0xFFFE/0xFFFF
- [ ] Instruction disassembler
- [ ] Save state support

### Phase 3: Additional CPUs

- [ ] Motorola 6800 CPU (in progress — 66/~197 opcodes: inherent, immediate ALU, shifts)
- [ ] MOS 6502 CPU (addressing modes, instruction set, BCD arithmetic)
- [ ] Zilog Z80 CPU (instruction prefixes, alternate register set)
- [ ] Motorola 68000 CPU (32-bit address space, 16-bit data bus)

### Phase 4: Peripherals & Systems

- [x] MC6821 PIA (full register set, interrupts, edge detection)
- [x] Williams SC1 blitter (DMA copy/fill, mask, shift, foreground-only)
- [x] CMOS RAM (1KB battery-backed, save/load persistence)
- [x] ROM loader (MAME ZIP support, CRC32-based matching, multi-variant ROM sets)
- [x] Machine trait (frontend-agnostic display/input/render interface)
- [x] Joust arcade board (Williams 2nd-gen: M6809 + video RAM + PIAs + blitter + CMOS + ROM)
- [ ] 6850 ACIA (serial communication)
- [ ] 6840 PTM (timer)
- [ ] Memory mappers and bank switching

### Phase 5: Frontend & Developer Tools

- [x] SDL2 frontend (renders any Machine impl, keyboard input, VSync timing)
- [ ] Debugger with breakpoints and step execution
- [ ] Memory viewer/editor
- [ ] Disassembly viewer
- [ ] Performance profiler

### Phase 6: Multimedia

- [ ] Sound chip emulation (AY-3-8910, SN76489)
- [ ] Additional arcade boards (Robotron, Defender)

## License

This project is licensed under the [MIT License](LICENSE).

This is a learning/reference implementation. Not affiliated with any hardware manufacturer.

## Resources

### Motorola 6800

- [MC6800 Programming Manual](http://www.bitsavers.org/components/motorola/6800/Motorola_M6800_Programming_Reference_Manual_M68PRM_D_Nov76.pdf) - Official Motorola programming reference
- [MC6800 Datasheet](http://www.bitsavers.org/components/motorola/6800/MC6800_8-Bit_Microprocessing_Unit.pdf) - Instruction timing and pinout
- [Motorola 6800 Wikipedia](https://en.wikipedia.org/wiki/Motorola_6800) - Architecture overview
- [MAME 6800 Core](https://github.com/mamedev/mame/tree/master/src/devices/cpu/m6800) - Reference implementation

### Motorola 6809

- [6809 Programmer's Reference](http://www.6809.org.uk/dragon/pdf/6809.pdf) - Official Motorola datasheet
- [6809 Instruction Set](http://www.8bit-museum.de/6809_isa.html) - Complete opcode reference
- [Motorola 6809 Wikipedia](https://en.wikipedia.org/wiki/Motorola_6809) - Architecture overview
- [elmerucr/MC6809](https://github.com/elmerucr/MC6809) - Independent 6809 reference emulator (used for cross-validation)
- [XRoar](http://www.6809.org.uk/xroar/) - Dragon/CoCo emulator (C)
- [MAME 6809 Core](https://github.com/mamedev/mame/tree/master/src/devices/cpu/m6809) - Reference implementation

See [CONTRIBUTING.md](CONTRIBUTING.md) for design decisions, troubleshooting, and contribution guidelines.
