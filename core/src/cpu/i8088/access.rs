//! What an instruction does to its ModR/M operand, and how wide.
//!
//! [`format`](super::format) says how many bytes an instruction *is*. This says
//! what it does to the operand those bytes address: reads it, writes it, both,
//! or neither, and whether that is a byte, a word, or the four bytes of a far
//! pointer.
//!
//! A per-cycle core needs this for the same reason it needs the length table.
//! An operand read is a four-T-state MEMR bus cycle that has to happen *before*
//! the instruction computes anything, and an operand write is a MEMW cycle that
//! has to happen after. Neither can be discovered by running the instruction,
//! because running it is what they bracket.
//!
//! # How this table is kept honest
//!
//! It is a second, independent statement of something `execute.rs` already
//! knows implicitly, which is exactly the shape that drifts. So it is
//! cross-checked against the executor rather than trusted: every instruction
//! records which operand accesses it actually made, and
//! [`super::I8088::run_loaded_instruction`] asserts that against what this
//! table predicted. Under a debug build that check runs on all 3,007,000
//! per-cycle vectors, so a wrong row is a failing test rather than a latent
//! timing bug.
//!
//! The table was written from the opcode map and then corrected by that check,
//! not the other way round.

/// How wide an operand access is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Width {
    /// One byte: one bus cycle.
    Byte,
    /// Two bytes, low then high: two bus cycles, the 8088's data bus being one
    /// byte wide.
    Word,
    /// Four bytes: a far pointer in memory, offset then segment. `LES`, `LDS`,
    /// and the indirect far `CALL` and `JMP`.
    FarPointer,
}

impl Width {
    /// How many bus cycles an access of this width costs.
    pub(crate) fn bytes(self) -> u8 {
        match self {
            Width::Byte => 1,
            Width::Word => 2,
            Width::FarPointer => 4,
        }
    }
}

/// What an instruction does to the operand its ModR/M byte addresses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Access {
    pub reads: bool,
    pub writes: bool,
    pub width: Width,
}

impl Access {
    const fn new(reads: bool, writes: bool, width: Width) -> Self {
        Self {
            reads,
            writes,
            width,
        }
    }
}

const NONE: Access = Access::new(false, false, Width::Byte);
const READ_B: Access = Access::new(true, false, Width::Byte);
const READ_W: Access = Access::new(true, false, Width::Word);
const WRITE_B: Access = Access::new(false, true, Width::Byte);
const WRITE_W: Access = Access::new(false, true, Width::Word);
const RMW_B: Access = Access::new(true, true, Width::Byte);
const RMW_W: Access = Access::new(true, true, Width::Word);
const READ_FAR: Access = Access::new(true, false, Width::FarPointer);

/// How many words an instruction takes off the stack before it runs, and how
/// many it puts back after.
///
/// The stack is the other way an instruction reaches memory, and it does not
/// fit the ModR/M operand's shape at all: an interrupt pushes three words, a
/// far return pops two, and neither is addressed by a ModR/M byte. But the
/// counts are fixed by the opcode, which is enough for the pipeline to run the
/// pops before the instruction and the pushes after.
///
/// Both halves cannot be non-zero for any instruction in this set: nothing both
/// pops and pushes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StackAccess {
    /// Words read from the stack before the instruction runs.
    pub pops: u8,
    /// Words written to the stack after it has run.
    pub pushes: u8,
}

impl StackAccess {
    const fn pops(n: u8) -> Self {
        Self { pops: n, pushes: 0 }
    }
    const fn pushes(n: u8) -> Self {
        Self { pops: 0, pushes: n }
    }
}

/// What `opcode` does to the stack.
///
/// Returns nothing for the instructions whose stack use is *conditional*, which
/// a count fixed by the opcode cannot express: `INTO` pushes three words only
/// when the overflow flag is set, and `DIV`, `IDIV` and `AAM` push three only
/// when they fault. Those keep reaching the stack directly from inside the
/// executor. The pipeline handles what it can predict and leaves the rest
/// alone, rather than predicting wrongly.
pub(crate) fn stack_access(opcode: u8, modrm: u8) -> StackAccess {
    match opcode {
        // PUSH and POP of a segment register, interleaved with the ALU block.
        0x06 | 0x0E | 0x16 | 0x1E => StackAccess::pushes(1),
        0x07 | 0x0F | 0x17 | 0x1F => StackAccess::pops(1),
        // PUSH and POP with the register in the opcode.
        0x50..=0x57 => StackAccess::pushes(1),
        0x58..=0x5F => StackAccess::pops(1),
        // POP r/m16, which pops the stack and then writes the operand.
        0x8F => StackAccess::pops(1),
        // CALL far direct pushes the return segment and offset.
        0x9A => StackAccess::pushes(2),
        // PUSHF and POPF.
        0x9C => StackAccess::pushes(1),
        0x9D => StackAccess::pops(1),
        // The near returns, and their undocumented aliases one encoding below.
        0xC0..=0xC3 => StackAccess::pops(1),
        // The far returns, which pop an offset and a segment.
        0xC8..=0xCB => StackAccess::pops(2),
        // INT 3 and INT n push flags, segment and offset. INTO is not here: it
        // pushes only when the overflow flag is set.
        0xCC | 0xCD => StackAccess::pushes(3),
        // IRET pops all three back.
        0xCF => StackAccess::pops(3),
        // CALL near direct pushes the return offset.
        0xE8 => StackAccess::pushes(1),
        // The group: indirect near call pushes one, indirect far call pushes
        // two, PUSH r/m16 pushes one. The jumps push nothing.
        0xFF => match (modrm >> 3) & 7 {
            2 => StackAccess::pushes(1),
            3 => StackAccess::pushes(2),
            6 => StackAccess::pushes(1),
            _ => StackAccess::default(),
        },
        _ => StackAccess::default(),
    }
}

/// Clocks the EU spends computing an effective address, by addressing mode.
///
/// From the 8088 datasheet's EA calculation table, and it is a table of
/// *additions* rather than an arbitrary cost per mode: one component costs 5,
/// a bare displacement 6, two components 7 or 8, three 9, 11 or 12. The two
/// base-plus-index pairings differ by one clock, which is a real asymmetry in
/// the part and not a transcription error: `BX+SI` and `BP+DI` take 7 where
/// `BX+DI` and `BP+SI` take 8.
///
/// **The hardware recording confirms every one of these numbers**, through
/// `LEA`. `LEA` is the one instruction that computes an effective address and
/// runs no bus cycle at all, so its recorded span is its own two clocks plus
/// the EA and nothing else, and it separates the address calculation from
/// everything that happens on the way to memory. Over the whole `8D` file, at a
/// full queue and with no prefix, every addressing mode is uniform and every
/// one of them lands on `2 + EA` for the values above, asymmetric pairings
/// included. Nothing else in this core is confirmed that directly.
///
/// The segment override's clocks are **not** here. The recording puts them at
/// two, on the register forms as much as the memory ones, which makes them a
/// property of the prefix rather than of the address calculation; the pipeline
/// charges them in [`super::I8088::begin_execute_phase`], where a prefix on an
/// instruction with no memory operand can be charged too.
///
/// Deliberately **not** applied: the datasheet's further "add 4 for word
/// operands at odd addresses". That is an 8086 penalty, where a misaligned word
/// costs a second bus cycle on a 16-bit bus. The 8088's bus is one byte wide,
/// so every word operand is already two bus cycles whatever its alignment, and
/// the pipeline issues both. Adding it here would charge that twice.
pub(crate) fn ea_cycles(modrm: u8) -> u8 {
    let mod_bits = (modrm >> 6) & 3;
    let rm = modrm & 7;

    match (mod_bits, rm) {
        // mod=00 rm=110 is a bare 16-bit address, the one mode with a
        // displacement and nothing to add it to.
        (0, 6) => 6,
        // One register: [SI], [DI], [BX].
        (0, 4 | 5 | 7) => 5,
        // Two registers. The pairing decides which of the two costs it takes.
        (0, 0 | 3) => 7,
        (0, 1 | 2) => 8,
        // With a displacement, mod=01 or mod=10. One register plus it, where
        // rm=110 is [BP] rather than the direct-address escape.
        (1 | 2, 4..=7) => 9,
        // Two registers plus a displacement, in the same two pairings.
        (1 | 2, 0 | 3) => 11,
        (1 | 2, 1 | 2) => 12,
        // mod=11 is a register operand with no address to compute, and the
        // caller does not ask.
        _ => 0,
    }
}

/// The ModR/M `reg` field, which selects the operation inside a group opcode.
#[inline]
fn reg_of(modrm: u8) -> u8 {
    (modrm >> 3) & 7
}

/// What `opcode` does to the operand `modrm` addresses.
///
/// `modrm` is only consulted for the group opcodes, where the `reg` field
/// selects between operations that treat the operand differently: `TEST` reads
/// its operand where `NOT` reads and writes it, under the same opcode byte.
pub(crate) fn operand_access(opcode: u8, modrm: u8) -> Access {
    match opcode {
        // The ALU block. The low three bits pick the form, and bits 5:3 pick
        // the operation, of which CMP (7) is the one that does not write back.
        0x00..=0x3F => {
            let is_cmp = (opcode >> 3) & 7 == 7;
            match opcode & 7 {
                // r/m is the destination: read, operate, write back.
                0 => Access::new(true, !is_cmp, Width::Byte),
                1 => Access::new(true, !is_cmp, Width::Word),
                // The register is the destination; r/m is only a source.
                2 => READ_B,
                3 => READ_W,
                // Accumulator and immediate forms have no ModR/M byte at all
                // and never reach here.
                _ => NONE,
            }
        }

        // The immediate-to-r/m group. Same operation encoding, same CMP
        // exception. 0x83 is a word operand with a sign-extended byte
        // immediate, so it is word-wide despite its immediate.
        0x80 | 0x82 => Access::new(true, reg_of(modrm) != 7, Width::Byte),
        0x81 | 0x83 => Access::new(true, reg_of(modrm) != 7, Width::Word),

        // TEST discards its result, so it only reads.
        0x84 => READ_B,
        0x85 => READ_W,
        // XCHG swaps, so it does both.
        0x86 => RMW_B,
        0x87 => RMW_W,
        // MOV, in all four directions.
        0x88 => WRITE_B,
        0x89 => WRITE_W,
        0x8A => READ_B,
        0x8B => READ_W,
        // MOV r/m16, sreg and MOV sreg, r/m16.
        0x8C => WRITE_W,
        0x8E => READ_W,
        // LEA computes an address and never dereferences it. This is the only
        // opcode with a memory ModR/M form that touches no memory, and a table
        // that got it wrong would have the pipeline read an address the
        // instruction was never going to look at.
        0x8D => NONE,
        // POP r/m16 writes the operand, and separately pops the stack.
        0x8F => WRITE_W,

        // The direct-address accumulator moves and XLAT. None of these has a
        // ModR/M byte: the first four carry their address as a 16-bit
        // displacement and XLAT computes it from BX and AL. They reach memory
        // all the same, and until the pipeline knew that they were reaching it
        // without running a bus cycle at all, which cost them four clocks
        // apiece and left forty thousand vectors with a transaction missing
        // from the middle of the recording.
        0xA0 => READ_B,
        0xA1 => READ_W,
        0xA2 => WRITE_B,
        0xA3 => WRITE_W,
        0xD7 => READ_B,

        // LES and LDS load a segment register and a general register from four
        // consecutive bytes.
        0xC4 | 0xC5 => READ_FAR,
        // MOV r/m, imm.
        0xC6 => WRITE_B,
        0xC7 => WRITE_W,

        // Shifts and rotates read the operand, shift it, and write it back.
        0xD0 | 0xD2 => RMW_B,
        0xD1 | 0xD3 => RMW_W,

        // The 8087 escapes. The part fetches the ModR/M byte and performs the
        // memory read it describes, so that a coprocessor could see the operand
        // on the bus. This core has no 8087 and does not perform that read,
        // which is a real gap rather than a modeling choice: it is why these
        // eight files stay on the state gate's skip list.
        0xD8..=0xDF => NONE,

        // The unary group. TEST occupies two of the eight encodings and only
        // reads; NOT and NEG write back; the multiplies and divides read the
        // operand and put their results in AX and DX.
        0xF6 => match reg_of(modrm) {
            0 | 1 => READ_B,
            2 | 3 => RMW_B,
            _ => READ_B,
        },
        0xF7 => match reg_of(modrm) {
            0 | 1 => READ_W,
            2 | 3 => RMW_W,
            _ => READ_W,
        },

        // INC and DEC on a byte operand.
        0xFE => RMW_B,

        // The catch-all group: INC, DEC, the two indirect CALLs, the two
        // indirect JMPs, and PUSH.
        0xFF => match reg_of(modrm) {
            0 | 1 => RMW_W,
            // CALL far and JMP far read a far pointer out of memory; the near
            // forms read a single word.
            3 | 5 => READ_FAR,
            2 | 4 | 6 => READ_W,
            _ => NONE,
        },

        // Everything else has no ModR/M byte.
        _ => NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LEA is the whole reason this table cannot be inferred from "does the
    /// instruction have a memory operand". It has one and never reads it.
    #[test]
    fn lea_addresses_memory_without_touching_it() {
        assert_eq!(operand_access(0x8D, 0x06), NONE);
    }

    /// CMP is an ALU operation that does not write back, in both the
    /// register-source and immediate forms.
    #[test]
    fn cmp_reads_its_operand_and_does_not_write_it() {
        // 0x38 is CMP r/m8, reg8: operation 7 in the low block.
        let cmp = operand_access(0x38, 0x06);
        assert!(cmp.reads);
        assert!(!cmp.writes);
        // And the same operation inside the immediate group, chosen by reg=7.
        let cmp_imm = operand_access(0x80, 0b00_111_110);
        assert!(cmp_imm.reads);
        assert!(!cmp_imm.writes);
        // Its neighbor in the same group does write back.
        assert!(operand_access(0x80, 0b00_000_110).writes, "ADD writes back");
    }

    /// The other seven ALU operations read and write.
    #[test]
    fn the_rest_of_the_alu_block_writes_back_to_a_memory_destination() {
        for op in 0..7u8 {
            let opcode = op << 3;
            assert_eq!(operand_access(opcode, 0x06), RMW_B, "{opcode:#04X}");
            assert_eq!(operand_access(opcode | 1, 0x06), RMW_W, "{opcode:#04X}+1");
            // The forms with the register as destination only read.
            assert_eq!(operand_access(opcode | 2, 0x06), READ_B);
            assert_eq!(operand_access(opcode | 3, 0x06), READ_W);
        }
    }

    /// Inside the unary group, TEST reads and NOT and NEG read and write, under
    /// one opcode byte.
    #[test]
    fn the_unary_group_splits_on_its_reg_field() {
        assert_eq!(operand_access(0xF6, 0b00_000_110), READ_B, "TEST");
        assert_eq!(operand_access(0xF6, 0b00_001_110), READ_B, "TEST alias");
        assert_eq!(operand_access(0xF6, 0b00_010_110), RMW_B, "NOT");
        assert_eq!(operand_access(0xF6, 0b00_011_110), RMW_B, "NEG");
        assert_eq!(operand_access(0xF6, 0b00_100_110), READ_B, "MUL");
        assert_eq!(operand_access(0xF7, 0b00_110_110), READ_W, "DIV");
    }

    /// The far transfers and the far-pointer loads read four bytes, which is
    /// four bus cycles rather than one.
    #[test]
    fn far_pointers_are_four_bytes_wide() {
        assert_eq!(operand_access(0xC4, 0x06), READ_FAR, "LES");
        assert_eq!(operand_access(0xC5, 0x06), READ_FAR, "LDS");
        assert_eq!(operand_access(0xFF, 0b00_011_110), READ_FAR, "CALL far");
        assert_eq!(operand_access(0xFF, 0b00_101_110), READ_FAR, "JMP far");
        assert_eq!(Width::FarPointer.bytes(), 4);
        assert_eq!(Width::Word.bytes(), 2);
        assert_eq!(Width::Byte.bytes(), 1);
    }

    /// A MOV into memory must not read it first. Getting this wrong costs a
    /// bus cycle the hardware never runs, on one of the commonest instructions
    /// there is.
    #[test]
    fn a_store_does_not_read_its_destination() {
        for op in [0x88u8, 0x89, 0x8C, 0xC6, 0xC7, 0x8F] {
            let a = operand_access(op, 0x06);
            assert!(!a.reads, "{op:#04X} should not read");
            assert!(a.writes, "{op:#04X} should write");
        }
    }

    /// Every opcode has an answer and nothing panics, including the ones whose
    /// group has undefined members.
    #[test]
    fn every_opcode_and_reg_field_has_an_answer() {
        for opcode in 0..=0xFFu8 {
            for reg in 0..8u8 {
                let _ = operand_access(opcode, reg << 3);
            }
        }
    }

    // --- Effective address timing ---

    /// The cost tracks the number of components the EU has to add, which is
    /// what makes this a structure rather than a list.
    #[test]
    fn ea_cost_rises_with_the_number_of_components() {
        // One register.
        assert_eq!(ea_cycles(0b00_000_100), 5, "[SI]");
        assert_eq!(ea_cycles(0b00_000_111), 5, "[BX]");
        // A bare displacement, which costs one more than a register.
        assert_eq!(ea_cycles(0b00_000_110), 6, "[disp16]");
        // Two registers.
        assert_eq!(ea_cycles(0b00_000_000), 7, "[BX+SI]");
        assert_eq!(ea_cycles(0b00_000_011), 7, "[BP+DI]");
        // One register and a displacement.
        assert_eq!(ea_cycles(0b01_000_111), 9, "[BX+d8]");
        assert_eq!(ea_cycles(0b10_000_110), 9, "[BP+d16]");
        // Three components.
        assert_eq!(ea_cycles(0b01_000_000), 11, "[BX+SI+d8]");
        assert_eq!(ea_cycles(0b10_000_001), 12, "[BX+DI+d16]");
    }

    /// The two base-plus-index pairings differ by a clock. This is a real
    /// asymmetry in the part, and the sort of detail that gets flattened by
    /// someone tidying the table.
    #[test]
    fn the_two_base_plus_index_pairings_cost_differently() {
        assert_eq!(ea_cycles(0b00_000_000), 7, "[BX+SI]");
        assert_eq!(ea_cycles(0b00_000_011), 7, "[BP+DI]");
        assert_eq!(ea_cycles(0b00_000_001), 8, "[BX+DI]");
        assert_eq!(ea_cycles(0b00_000_010), 8, "[BP+SI]");
    }

    /// A register operand has no address to compute.
    #[test]
    fn a_register_operand_costs_nothing_to_address() {
        for rm in 0..8u8 {
            assert_eq!(ea_cycles(0b11_000_000 | rm), 0, "rm={rm}");
        }
    }
}
