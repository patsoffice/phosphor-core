# phosphor-core

CPU implementations, Bus trait, and peripheral devices. Zero external dependencies.

## CPU Architecture Rules

- Instructions go in `src/cpu/<cpu>/alu.rs` (ALU ops) or `load_store.rs` (load/store)
- Opcode dispatch entries go in `src/cpu/<cpu>/mod.rs` `execute_instruction()`
- Inherent-mode instructions use `if cycle == 0 { ... }` pattern
- Immediate-mode instructions use `alu_imm()` helper
- Always transition to `ExecState::Fetch` when instruction completes

## Flag Conventions

- Use `CcFlag` enum, never raw hex values (0x01, 0x02, etc.)
- All instruction doc comments must document flag behavior
- Use `set_flags_arithmetic()` for add/sub, `set_flags_logical()` for AND/OR/EOR/TST, `set_flags_shift()` for shift/rotate
- V flag for shift/rotate = N XOR C (post-operation)

## CPU-Specific Notes

- M6800 follows identical patterns to M6809 (same flag helpers, addressing mode helpers, state machine)
- M6800 has no DP register (direct mode always page 0), no Y/U registers, no multi-byte opcode prefixes

## Testing

- Tests go in `tests/<cpu>_*_test.rs`, grouped by category (e.g., `m6809_alu_binary_test.rs`)
- Use direct CPU + TestBus pattern, not Simple*System:

```rust
let mut cpu = M6809::new();
let mut bus = TestBus::new();
bus.load(0, &[0x86, 0x42]);  // LDA #$42
cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
assert_eq!(cpu.a, 0x42);
```

## Gotchas

- Tests failing with wrong PC values often need more `tick()` calls (each cycle = one tick)
- The borrow-splitting `unsafe` in system `tick()` methods is sound because CPU and Bus access disjoint memory
