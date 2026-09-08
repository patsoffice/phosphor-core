//! How long an instruction is, decided one byte at a time.
//!
//! A per-cycle core cannot run the instruction and find out how many bytes it
//! consumed: it has to fetch each byte over four T-states before it can execute
//! anything, so it needs to know, from the opcode alone, what is still to come.
//! That is what this table is. It is the loader's half of decoding, separate
//! from [`super::decode`], which is the executor's half.
//!
//! The 8088 instruction stream after any prefixes is:
//!
//! ```text
//! opcode  [ModR/M]  [displacement 0/1/2]  [immediate 0/1/2/4]
//! ```
//!
//! Only the first two lengths are fixed by the opcode. The displacement length
//! comes out of the ModR/M byte, and two opcode groups take an immediate whose
//! presence depends on the ModR/M `reg` field, so the loader has to consult
//! this table twice: once on the opcode and once more after the ModR/M byte
//! arrives.
//!
//! # Why this cannot drift from the executor without being noticed
//!
//! A table like this is the classic place for a quiet lie: fetch one byte too
//! few and the executor reads a byte that was never on the bus; fetch one too
//! many and the CPU issues a bus cycle the hardware never issued. Both are
//! caught rather than assumed.
//!
//! - Too few is a panic. [`super::I8088::fetch_byte`] pulls from the loaded
//!   instruction buffer and there is nothing behind it to read.
//! - Too many shows up in the per-cycle gate as four surplus T-states on every
//!   vector for that opcode, which is 10,000 failures in one file rather than a
//!   rounding error.

/// The immediate that follows an instruction's ModR/M and displacement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Imm {
    None,
    /// One byte: `imm8`, `rel8`, a port number, or an interrupt vector.
    Byte,
    /// Two bytes: `imm16`, `rel16`, or a direct memory offset.
    Word,
    /// Four bytes: a far pointer, offset first and then segment. `CALL far`
    /// (0x9A) and `JMP far` (0xEA) are the only two.
    FarPointer,
    /// One byte, but only when the ModR/M `reg` field is 0 or 1. This is the
    /// 0xF6 group, where `TEST r/m8, imm8` shares an opcode with NOT, NEG, MUL,
    /// IMUL, DIV and IDIV, which take no immediate at all.
    ByteIfTest,
    /// Two bytes under the same rule: the 0xF7 group.
    WordIfTest,
}

impl Imm {
    /// How many bytes to fetch, given the ModR/M byte when there is one.
    ///
    /// `modrm` is `None` for an instruction with no ModR/M byte, which is also
    /// every instruction for which the two conditional variants are impossible.
    pub(crate) fn len(self, modrm: Option<u8>) -> u8 {
        match self {
            Imm::None => 0,
            Imm::Byte => 1,
            Imm::Word => 2,
            Imm::FarPointer => 4,
            Imm::ByteIfTest | Imm::WordIfTest => {
                // Bits 5:3 select the operation within the group. 0 and 1 are
                // both TEST: the 1 encoding is an undocumented alias of the 0
                // one and takes its immediate too, which is why the skip list
                // in the state gate can name F6.1 and F7.1 without them being a
                // different length.
                let reg = (modrm.unwrap_or(0) >> 3) & 7;
                match (self, reg) {
                    (Imm::ByteIfTest, 0 | 1) => 1,
                    (Imm::WordIfTest, 0 | 1) => 2,
                    _ => 0,
                }
            }
        }
    }
}

/// What follows an opcode byte.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Format {
    /// Whether a ModR/M byte follows the opcode.
    pub modrm: bool,
    /// The immediate after any ModR/M and displacement.
    pub imm: Imm,
}

impl Format {
    const fn new(modrm: bool, imm: Imm) -> Self {
        Self { modrm, imm }
    }
}

/// No ModR/M, no immediate: the opcode is the whole instruction.
const BARE: Format = Format::new(false, Imm::None);
/// No ModR/M, one immediate byte.
const IMM8: Format = Format::new(false, Imm::Byte);
/// No ModR/M, one immediate word.
const IMM16: Format = Format::new(false, Imm::Word);
/// ModR/M, no immediate.
const MODRM: Format = Format::new(true, Imm::None);
/// ModR/M then an immediate byte.
const MODRM_IMM8: Format = Format::new(true, Imm::Byte);
/// ModR/M then an immediate word.
const MODRM_IMM16: Format = Format::new(true, Imm::Word);

/// The displacement length encoded by a ModR/M byte.
///
/// `mod` selects the form: 00 has no displacement except for the one
/// direct-address escape, 01 has a signed byte, 10 has a word, and 11 is a
/// register operand with no memory address at all.
pub(crate) fn displacement_len(modrm: u8) -> u8 {
    let mod_bits = (modrm >> 6) & 3;
    let rm = modrm & 7;
    match mod_bits {
        // mod=00 rm=110 is not [BP], it is a bare 16-bit address. That escape
        // is why [BP] with no displacement has to be encoded as mod=01 with a
        // displacement of zero.
        0 => {
            if rm == 6 {
                2
            } else {
                0
            }
        }
        1 => 1,
        2 => 2,
        _ => 0,
    }
}

/// What follows `opcode` in the instruction stream.
///
/// Prefix bytes are not in this table. The loader recognizes them through
/// [`super::decode::decode_prefix`] and keeps fetching, because a prefix is
/// followed by another opcode rather than by operands.
pub(crate) fn format_of(opcode: u8) -> Format {
    match opcode {
        // 0x00-0x3F: eight ALU operations, each in the same six forms, with a
        // segment push/pop or a BCD adjust in the two slots left over.
        //   +0 r/m8,r8   +1 r/m16,r16   +2 r8,r/m8   +3 r16,r/m16
        //   +4 AL,imm8   +5 AX,imm16    +6/+7 the leftovers
        0x00..=0x3F => match opcode & 7 {
            0..=3 => MODRM,
            4 => IMM8,
            5 => IMM16,
            // PUSH/POP ES, CS, SS, DS and DAA/DAS/AAA/AAS. The segment override
            // prefixes (0x26, 0x2E, 0x36, 0x3E) also land here and never reach
            // this table.
            _ => BARE,
        },

        // INC/DEC r16, PUSH/POP r16: the register is in the opcode.
        0x40..=0x5F => BARE,

        // 0x60-0x6F have no encodings of their own on the 8088. The opcode
        // decoder masks too loosely and they alias the conditional jumps at
        // 0x70-0x7F, taking a rel8 like those do.
        0x60..=0x7F => IMM8,

        // The immediate-to-r/m ALU group. 0x82 is an undocumented alias of
        // 0x80, and 0x83 sign-extends its byte to a word, so three of the four
        // carry a byte and only 0x81 carries a word.
        0x80 => MODRM_IMM8,
        0x81 => MODRM_IMM16,
        0x82 => MODRM_IMM8,
        0x83 => MODRM_IMM8,

        // TEST, XCHG, MOV r/m,r and r,r/m, MOV to and from a segment register,
        // LEA, POP r/m16.
        0x84..=0x8F => MODRM,

        // XCHG AX,r16 (0x90 being NOP, an XCHG of AX with itself), CBW, CWD.
        0x90..=0x99 => BARE,
        // CALL far ptr16:16.
        0x9A => Format::new(false, Imm::FarPointer),
        // WAIT, PUSHF, POPF, SAHF, LAHF.
        0x9B..=0x9F => BARE,

        // MOV between AL/AX and a direct address. The address is a 16-bit
        // offset in the instruction stream, not a displacement, so it is an
        // immediate here even though it names memory.
        0xA0..=0xA3 => IMM16,

        // MOVS and CMPS: operands are implied by SI and DI.
        0xA4..=0xA7 => BARE,

        // TEST AL,imm8 and TEST AX,imm16.
        0xA8 => IMM8,
        0xA9 => IMM16,

        // STOS, LODS, SCAS.
        0xAA..=0xAF => BARE,

        // MOV r8,imm8 and MOV r16,imm16, register in the opcode.
        0xB0..=0xB7 => IMM8,
        0xB8..=0xBF => IMM16,

        // RET and RETF, each with and without a stack adjustment, and each with
        // an undocumented alias one encoding below it (0xC0 for 0xC2, 0xC1 for
        // 0xC3, 0xC8 for 0xCA, 0xC9 for 0xCB).
        0xC0 | 0xC2 | 0xC8 | 0xCA => IMM16,
        0xC1 | 0xC3 | 0xC9 | 0xCB => BARE,

        // LES and LDS load a segment register and a general register from a
        // far pointer in memory, addressed by the ModR/M byte.
        0xC4 | 0xC5 => MODRM,

        // MOV r/m,imm.
        0xC6 => MODRM_IMM8,
        0xC7 => MODRM_IMM16,

        // INT 3 and INTO carry their vector implicitly; INT n takes it as a
        // byte. IRET takes nothing.
        0xCC => BARE,
        0xCD => IMM8,
        0xCE | 0xCF => BARE,

        // Shifts and rotates, by one and by CL.
        0xD0..=0xD3 => MODRM,

        // AAM and AAD carry a base byte, which is 10 in every assembler's
        // output and arbitrary in the encoding.
        0xD4 | 0xD5 => IMM8,

        // SALC, undocumented, and XLAT.
        0xD6 | 0xD7 => BARE,

        // The 8087 escape opcodes. With no coprocessor fitted the 8088 still
        // fetches the ModR/M byte and performs the memory read it describes,
        // so the instruction has a length here even though nothing acts on it.
        0xD8..=0xDF => MODRM,

        // LOOPNZ, LOOPZ, LOOP, JCXZ: all rel8.
        0xE0..=0xE3 => IMM8,

        // IN and OUT with an immediate port number.
        0xE4..=0xE7 => IMM8,

        // CALL and JMP near, then JMP far, then JMP short.
        0xE8 | 0xE9 => IMM16,
        0xEA => Format::new(false, Imm::FarPointer),
        0xEB => IMM8,

        // IN and OUT with the port in DX.
        0xEC..=0xEF => BARE,

        // LOCK, REP and REPNE are prefixes and never reach this table. HLT and
        // CMC take nothing.
        0xF0..=0xF5 => BARE,

        // The unary group. TEST is the only member with an immediate, and it
        // occupies two of the eight reg encodings.
        0xF6 => Format::new(true, Imm::ByteIfTest),
        0xF7 => Format::new(true, Imm::WordIfTest),

        // CLC through STD.
        0xF8..=0xFD => BARE,

        // INC/DEC r/m8, and the group holding INC, DEC, CALL, JMP and PUSH on
        // r/m16.
        0xFE | 0xFF => MODRM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two forms that share an opcode with six that have no immediate.
    /// Getting this wrong is silent in the state gate, because the executor
    /// would read its immediate out of the next instruction's first byte, which
    /// is a plausible-looking value.
    #[test]
    fn the_unary_group_takes_an_immediate_only_for_test() {
        // reg=0 and reg=1 are both TEST.
        assert_eq!(Imm::ByteIfTest.len(Some(0b11_000_000)), 1);
        assert_eq!(Imm::ByteIfTest.len(Some(0b11_001_000)), 1);
        assert_eq!(Imm::WordIfTest.len(Some(0b11_000_000)), 2);
        assert_eq!(Imm::WordIfTest.len(Some(0b11_001_000)), 2);
        // NOT, NEG, MUL, IMUL, DIV, IDIV take none.
        for reg in 2..8u8 {
            assert_eq!(Imm::ByteIfTest.len(Some(reg << 3)), 0, "reg={reg}");
            assert_eq!(Imm::WordIfTest.len(Some(reg << 3)), 0, "reg={reg}");
        }
    }

    /// mod=00 rm=110 is a 16-bit direct address, not [BP]. Reading it as [BP]
    /// would fetch two bytes too few and leave the executor decoding the next
    /// instruction as a displacement.
    #[test]
    fn the_direct_address_escape_is_two_bytes_and_bp_is_not() {
        assert_eq!(
            displacement_len(0b00_000_110),
            2,
            "mod=00 rm=110 is [disp16]"
        );
        assert_eq!(displacement_len(0b00_000_111), 0, "mod=00 rm=111 is [BX]");
        // The same rm under mod=01 and mod=10 really is BP, with a
        // displacement whose length comes from mod alone.
        assert_eq!(displacement_len(0b01_000_110), 1);
        assert_eq!(displacement_len(0b10_000_110), 2);
    }

    #[test]
    fn a_register_operand_has_no_displacement() {
        for rm in 0..8u8 {
            assert_eq!(displacement_len(0b11_000_000 | rm), 0, "rm={rm}");
        }
    }

    #[test]
    fn every_mod_01_form_has_a_signed_byte_displacement() {
        for rm in 0..8u8 {
            assert_eq!(displacement_len(0b01_000_000 | rm), 1, "rm={rm}");
        }
    }

    /// The ALU block is regular, and the regularity is the reason it is a
    /// computed arm rather than sixty-four rows. If the low three bits stopped
    /// selecting the form, this is what would notice.
    #[test]
    fn the_alu_block_repeats_the_same_six_forms_eight_times() {
        for base in (0x00..0x40).step_by(8) {
            assert_eq!(format_of(base), MODRM, "{base:#04X}");
            assert_eq!(format_of(base + 1), MODRM, "{:#04X}", base + 1);
            assert_eq!(format_of(base + 2), MODRM, "{:#04X}", base + 2);
            assert_eq!(format_of(base + 3), MODRM, "{:#04X}", base + 3);
            assert_eq!(format_of(base + 4), IMM8, "{:#04X}", base + 4);
            assert_eq!(format_of(base + 5), IMM16, "{:#04X}", base + 5);
            assert_eq!(format_of(base + 6), BARE, "{:#04X}", base + 6);
            assert_eq!(format_of(base + 7), BARE, "{:#04X}", base + 7);
        }
    }

    /// The two far-pointer instructions, which are the only four-byte
    /// immediates in the set.
    #[test]
    fn only_the_far_transfers_carry_a_four_byte_operand() {
        assert_eq!(format_of(0x9A).imm, Imm::FarPointer);
        assert_eq!(format_of(0xEA).imm, Imm::FarPointer);
        assert_eq!(Imm::FarPointer.len(None), 4);
        for op in 0..=0xFFu8 {
            if op != 0x9A && op != 0xEA {
                assert_ne!(format_of(op).imm, Imm::FarPointer, "{op:#04X}");
            }
        }
    }

    /// MOV with an immediate is the one place the register is in the opcode and
    /// the operand size changes halfway through the block.
    #[test]
    fn the_mov_immediate_block_changes_width_at_its_midpoint() {
        for op in 0xB0..=0xB7u8 {
            assert_eq!(format_of(op), IMM8, "{op:#04X}");
        }
        for op in 0xB8..=0xBFu8 {
            assert_eq!(format_of(op), IMM16, "{op:#04X}");
        }
    }

    /// The undocumented RET aliases have to take the same operands as the
    /// instructions they alias, or the loader would desynchronize on them.
    #[test]
    fn the_ret_aliases_match_the_instructions_they_alias() {
        assert_eq!(format_of(0xC0), format_of(0xC2), "RET imm16");
        assert_eq!(format_of(0xC1), format_of(0xC3), "RET");
        assert_eq!(format_of(0xC8), format_of(0xCA), "RETF imm16");
        assert_eq!(format_of(0xC9), format_of(0xCB), "RETF");
    }

    /// The 0x60 block aliases the conditional jumps, so it takes a rel8 like
    /// them. Treating it as a bare opcode would lose a byte on every one.
    #[test]
    fn the_alias_block_takes_a_rel8_like_the_jumps_it_aliases() {
        for op in 0x60..=0x7Fu8 {
            assert_eq!(format_of(op), IMM8, "{op:#04X}");
        }
    }

    /// A ModR/M byte and an immediate are independent, and the table has to be
    /// able to say "both". These are the four opcodes that do.
    #[test]
    fn some_instructions_carry_a_modrm_and_an_immediate() {
        for op in [0x80u8, 0x81, 0x82, 0x83, 0xC6, 0xC7] {
            let f = format_of(op);
            assert!(f.modrm, "{op:#04X}");
            assert_ne!(f.imm, Imm::None, "{op:#04X}");
        }
    }

    /// Every opcode has an answer, and the two single-opcode arms sitting
    /// between ranges are not shadowed by their neighbors. The match is
    /// exhaustive over `u8` by construction, so what this really guards is
    /// ordering: 0x9A between 0x90..=0x99 and 0x9B..=0x9F is the one place a
    /// widened range would silently swallow an entry.
    #[test]
    fn every_opcode_has_a_format_including_the_ones_between_ranges() {
        for op in 0..=0xFFu8 {
            let _ = format_of(op);
        }
        assert_eq!(format_of(0x99), BARE, "CWD");
        assert_eq!(format_of(0x9A).imm, Imm::FarPointer, "CALL far");
        assert_eq!(format_of(0x9B), BARE, "WAIT");
    }
}
