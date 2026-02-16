# CPU Implementation Guidelines

## File Organization (per CPU)

- `mod.rs` - State machine, opcode dispatch table via `execute_instruction()`
- `alu.rs` - Flag helpers, addressing mode helpers, module re-exports
- `alu/binary.rs` - ADD, SUB, CMP, ADC, SBC, AND, ORA, EOR, BIT
- `alu/unary.rs` - NEG, COM, CLR, INC, DEC, TST
- `alu/shift.rs` - ASL, ASR, LSR, ROL, ROR
- `branch.rs` - Conditional/unconditional branches, JSR, JMP, RTS
- `load_store.rs` - LD*, ST*, LEA, transfers
- `stack.rs` - PUSH, PULL, SWI, RTI, interrupt handling

## Adding a New Instruction

1. Implement the operation in the appropriate `alu/*.rs` or `load_store.rs` file
2. Add the opcode dispatch entry in `mod.rs` `execute_instruction()`
3. Add integration tests in `tests/<cpu>_*_test.rs`
4. Update the CPU's `README.md` opcode count

## README Maintenance

- Each CPU directory has its own README.md documenting architecture, instruction set, and resources
- Update when adding instructions, addressing modes, or changing opcode counts
