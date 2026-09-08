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
}
