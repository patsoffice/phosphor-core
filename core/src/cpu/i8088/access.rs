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
            // PUSH r/m16, and its undocumented alias one encoding above.
            6 | 7 => StackAccess::pushes(1),
            _ => StackAccess::default(),
        },
        _ => StackAccess::default(),
    }
}

/// What one iteration of a string operation touches.
///
/// The fourth way an instruction reaches memory, and the one the ModR/M operand
/// table cannot describe at all: the addresses come from SI and DI rather than
/// from an encoding, there can be two of them, and a `REP` prefix runs the
/// whole thing again with both moved on. The source is `[DS:SI]`, with the
/// segment overridable; the destination is `[ES:DI]`, and ES is not.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct StringAccess {
    /// `MOVS`, `CMPS` and `LODS` read `[DS:SI]`.
    pub reads_source: bool,
    /// `CMPS` and `SCAS` read `[ES:DI]`.
    pub reads_dest: bool,
    /// `MOVS` and `STOS` write `[ES:DI]`.
    pub writes_dest: bool,
    /// One byte or two, from the opcode's low bit.
    pub width: u8,
}

/// What `opcode`'s string operation touches, or `None` if it is not one.
pub(crate) fn string_access(opcode: u8) -> Option<StringAccess> {
    let width = if opcode & 1 == 0 { 1 } else { 2 };
    let (reads_source, reads_dest, writes_dest) = match opcode {
        // MOVS: read the source, write the destination.
        0xA4 | 0xA5 => (true, false, true),
        // CMPS: read both and compare.
        0xA6 | 0xA7 => (true, true, false),
        // STOS: write the accumulator to the destination.
        0xAA | 0xAB => (false, false, true),
        // LODS: read the source into the accumulator.
        0xAC | 0xAD => (true, false, false),
        // SCAS: read the destination and compare it against the accumulator.
        0xAE | 0xAF => (false, true, false),
        _ => return None,
    };
    Some(StringAccess {
        reads_source,
        reads_dest,
        writes_dest,
        width,
    })
}

/// What `opcode` does to an I/O port, and how wide.
///
/// The third way an instruction reaches the outside world, after the ModR/M
/// operand and the stack, and the only one that is not memory: `IN` and `OUT`
/// drive IOR and IOW rather than MEMR and MEMW, and address sixteen bits of
/// port space rather than twenty of memory. A word port access is two cycles at
/// consecutive port numbers, for the same reason a word memory access is: the
/// 8088's data bus is one byte wide.
///
/// The port itself is not here, because it is not a property of the opcode
/// alone: `E4` through `E7` carry it as an immediate byte and `EC` through `EF`
/// take it from DX. [`super::I8088::port_of`] resolves that.
pub(crate) fn port_access(opcode: u8) -> Option<Access> {
    match opcode {
        // IN accumulator, immed8 and IN accumulator, DX.
        0xE4 | 0xEC => Some(READ_B),
        0xE5 | 0xED => Some(READ_W),
        // OUT immed8, accumulator and OUT DX, accumulator.
        0xE6 | 0xEE => Some(WRITE_B),
        0xE7 | 0xEF => Some(WRITE_W),
        _ => None,
    }
}

/// Clocks the effective-address microcode spends **after** the displacement has
/// been read.
///
/// The part's address calculation is in two halves with the displacement fetch
/// between them, because how long that fetch takes depends on the queue and
/// cannot be known in advance. The first half is the address arithmetic on the
/// register components, and this core already spends it: it is
/// [`super::timing::loader_stall`]'s `BeforeDisplacement`, whose per-mode values
/// are the published `pre_disp_cost` plus the jump into the routine, mode for
/// mode. This is the second half.
///
/// The values are the published table's, and its oddity is real: **an eight-bit
/// displacement costs one more to finish than a sixteen-bit one**, because of an
/// extra jump at microcode line 0x1de on that path.
///
/// What this replaces was fitted rather than read. It took Table 1-16's `+EA`,
/// rounded it up to an even number of clocks for any operand that reached
/// memory, and subtracted the displacement's length. The rounding in particular
/// has no counterpart in the part: it was a rule inferred from where recorded
/// bus cycles started, and it fitted the commonest modes by construction.
pub(crate) fn ea_post_disp_cycles(modrm: u8) -> u8 {
    match (modrm >> 6) & 3 {
        // A register operand has no effective address at all.
        3 => 0,
        // The direct form, `mod=00 rm=110`, whose whole cost is here: one clock
        // at 0x1dc, with nothing before the displacement.
        0 if modrm & 7 == 6 => 1,
        // No displacement, so the address was finished by the arithmetic.
        0 => 0,
        1 => 3,
        _ => 2,
    }
}

/// The first half: the jump into the address routine plus the arithmetic on
/// the register components.
///
/// **The part spends this for every memory mode, displacement or not.** The
/// loader spends it in front of the displacement, which is where the recording
/// puts it, but a mode with no displacement has nothing to put it in front of
/// and the clocks are owed all the same. `MOV word [DS:DI], imm16` is the case
/// that showed it: the part reads the immediate five clocks after the ModR/M
/// byte and this core read it on the very next one.
///
/// So the address phase asks for this when the loader had no displacement to
/// spend it against, and for [`ea_post_disp_cycles`] alone when it did.
pub(crate) fn ea_pre_disp_cycles(modrm: u8) -> u8 {
    let pre = match (modrm >> 6) & 3 {
        3 => return 0,
        // The direct form computes nothing: its address is the displacement.
        0 if modrm & 7 == 6 => 0,
        _ => match modrm & 7 {
            // Two registers, in the published table's asymmetric pairings.
            0 | 3 => 4,
            1 | 2 => 5,
            // One register.
            _ => 2,
        },
    };
    // Plus the jump into the routine.
    1 + pre
}

/// The ModR/M `reg` field, which selects the operation inside a group opcode.
#[inline]
fn reg_of(modrm: u8) -> u8 {
    (modrm >> 3) & 7
}

/// Whether the operand read belongs to the instruction's routine rather than to
/// the phase in front of it.
///
/// The pipeline reads an operand before a routine can start, which is right for
/// every instruction whose microcode begins after the read: the address
/// routine's `1E2: OPR -> tmpb` is the read, and the instruction's own lines
/// follow it. An instruction with microcode *in front of* its read cannot be
/// expressed that way, and this is the list of them.
///
/// `XLAT` is the only one. `mc_10c` spends 0x10c, 0x10d and 0x10e and only then
/// calls `biu_read_u8`, so this core asked for the bus three clocks early and
/// the fetch the part runs in the meantime had nowhere to go. Its address comes
/// from BX and AL rather than from a ModR/M byte, which is why there is no
/// address routine to hold the clocks instead.
///
/// See [`microcode::Step::ReadOperand`], which is what drives the read once the
/// pipeline has been told to leave it alone.
pub(crate) fn reads_from_its_routine(opcode: u8) -> bool {
    opcode == 0xD7
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
        // on the bus, and it reads a **word**: the recording shows two MEMR
        // cycles at consecutive addresses on every memory form.
        //
        // The CPU itself does nothing with the value, so the executor never
        // looks at it. That is the whole instruction on a machine with no
        // coprocessor: an address on the bus and a byte pair nobody reads.
        //
        // Leaving this as `NONE` was worth 8 clocks plus the even-address
        // rounding on every memory form of all eight opcodes, which read -10 on
        // every even effective address and -11 on every odd one.
        0xD8..=0xDF => READ_W,

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
            // forms read a single word. reg=7 is PUSH again, for the reason in
            // `execute.rs`.
            3 | 5 => READ_FAR,
            _ => READ_W,
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

    /// **An eight-bit displacement costs one more to finish than a sixteen-bit
    /// one.** That reads like a transcription error and is not: there is an
    /// extra jump at microcode line 0x1de on the byte path. It is exactly the
    /// sort of thing someone tidying the table would flatten, so it is pinned.
    #[test]
    fn a_byte_displacement_finishes_slower_than_a_word_one() {
        assert_eq!(ea_post_disp_cycles(0b01_000_111), 3, "[BX+d8]");
        assert_eq!(ea_post_disp_cycles(0b10_000_111), 2, "[BX+d16]");
    }

    /// The modes with no displacement have nothing left to do once the address
    /// arithmetic is done, and the direct form is the other way round: all of
    /// its cost is here and none of it is in the arithmetic.
    #[test]
    fn the_halves_of_the_address_split_by_whether_there_is_a_displacement() {
        for rm in [0u8, 1, 2, 3, 4, 5, 7] {
            assert_eq!(ea_post_disp_cycles(rm), 0, "mod=00 rm={rm}");
        }
        assert_eq!(ea_post_disp_cycles(0b00_000_110), 1, "[disp16]");
    }

    /// A register operand has no address to compute.
    #[test]
    fn a_register_operand_costs_nothing_to_address() {
        for rm in 0..8u8 {
            assert_eq!(ea_post_disp_cycles(0b11_000_000 | rm), 0, "rm={rm}");
        }
    }

    /// The two halves of the address calculation live in different modules, and
    /// they have to agree with the published per-mode table between them.
    ///
    /// The first half is the loader's pause before the displacement, which is
    /// the published `pre_disp_cost` plus the one clock of the jump into the
    /// routine. The second is [`ea_post_disp_cycles`]. Nothing else checks that
    /// the two were transcribed from the same table, and the asymmetric pairing
    /// (`BX+DI` and `BP+SI` costing a clock more than `BX+SI` and `BP+DI`) is
    /// the part of it most likely to be quietly flattened.
    #[test]
    fn the_two_halves_match_the_published_per_mode_table() {
        use super::super::timing::{self, LoaderStall};

        // (mod, rm) => (pre_disp_cost, post_disp_cost), read off the published
        // ModR/M table.
        let published: &[(u8, u8, u8, u8)] = &[
            (0, 0, 4, 0),
            (0, 1, 5, 0),
            (0, 2, 5, 0),
            (0, 3, 4, 0),
            (0, 4, 2, 0),
            (0, 5, 2, 0),
            (0, 6, 0, 1),
            (0, 7, 2, 0),
            (1, 0, 4, 3),
            (1, 1, 5, 3),
            (1, 2, 5, 3),
            (1, 3, 4, 3),
            (1, 4, 2, 3),
            (1, 5, 2, 3),
            (1, 6, 2, 3),
            (1, 7, 2, 3),
            (2, 0, 4, 2),
            (2, 1, 5, 2),
            (2, 2, 5, 2),
            (2, 3, 4, 2),
            (2, 4, 2, 2),
            (2, 5, 2, 2),
            (2, 6, 2, 2),
            (2, 7, 2, 2),
        ];

        for &(mod_bits, rm, pre, post) in published {
            let modrm = (mod_bits << 6) | rm;
            assert_eq!(
                ea_post_disp_cycles(modrm),
                post,
                "mod={mod_bits} rm={rm}: after the displacement"
            );
            // `8B`, MOV r16, r/m16, is a plain ModR/M opcode with no immediate,
            // so its stall is the addressing mode's and nothing else.
            let want = match timing::loader_stall(0x8B, modrm) {
                LoaderStall::BeforeDisplacement(n) => n,
                _ => 0,
            };
            let expected = if mod_bits == 0 && rm != 6 {
                // No displacement to pause in front of.
                0
            } else {
                1 + pre
            };
            assert_eq!(
                want, expected,
                "mod={mod_bits} rm={rm}: before the displacement"
            );
        }
    }
}
