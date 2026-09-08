//! How long the execution unit spends on an instruction, excluding its bus.
//!
//! Source: Intel, *iAPX 86/88, 186/188 User's Manual*, order 210912-001 (1985),
//! Table 1-16 "Instruction Set Reference Data". Every number below is derived
//! from that table by one rule, applied without exception.
//!
//! # The decomposition, and why it is not a set of fitted constants
//!
//! Table 1-16 gives two columns per instruction form: `Clocks` and `Transfers`.
//! The clocks are a *total* for the whole instruction, and its footnote says
//! how to move from the 8086 to the 8088:
//!
//! > "For the 8086 (80186) add four clocks for each 16-bit word transfer with
//! > an odd address. For the 8088 (80188) add four clocks for each 16-bit word
//! > transfer."
//!
//! This core already spends those bus clocks itself: four T-states per byte
//! transferred, issued by the pipeline in [`super`], plus the effective-address
//! clocks from [`super::access::ea_cycles`], which the table quotes separately
//! as `+EA`. So what is left for the EU is
//!
//! ```text
//! eu_cycles = documented_clocks - 4 * transfers
//! ```
//!
//! **That subtraction carries its own check.** The EU's microcode time does not
//! depend on operand width, so the byte and word forms of one operation must
//! come out the same. `ADD reg, mem` is 9 clocks with 1 transfer, and on the
//! 8088 a word form is 13 with two bus cycles: `9 - 4` and `13 - 8` are both 5.
//! Any row where the two disagree is a transcription error or a bug in the bus
//! model, and it is caught without consulting the test vectors at all.
//!
//! # Where these cycles are spent
//!
//! Between the operand read and the write-back, which is the one place the
//! table does not say and this core has to choose. It is not a guess: the
//! per-cycle gate's bus-cycle comparison showed the hardware slipping prefetches
//! in exactly there, during time this core was not spending. A wrong choice
//! stays visible in that comparison, because it leaves the prefetches
//! interleaved in the wrong order even when the total is right.
//!
//! # What is not here yet
//!
//! The string operations, the control transfers, and `MUL`/`IMUL`/`DIV`/`IDIV`.
//! The last of those are quoted as *ranges* (`DIV reg8` is 80-90) because their
//! microcode is data-dependent, so no single constant can match them and
//! pretending otherwise would turn a known gap into a number that looks like an
//! answer. An opcode with no entry here is charged nothing, which undercounts
//! it; [`eu_cycles`] returning zero means "not yet modeled", not "free".

/// Clocks the EU spends on `opcode`, beyond its bus cycles and its
/// effective-address calculation.
///
/// `modrm` selects the form: its `mod` field distinguishes a register operand
/// from a memory one, and its `reg` field picks the operation inside a group.
/// It is ignored for opcodes with no ModR/M byte.
pub(crate) fn eu_cycles(opcode: u8, modrm: u8) -> u8 {
    let is_mem = (modrm >> 6) != 3;
    let reg = (modrm >> 3) & 7;

    match opcode {
        // PUSH and POP of a segment register sit in the last two columns of the
        // ALU block's rows and have nothing to do with it. They are matched
        // ahead of it so that the block's `opcode & 7` dispatch cannot charge
        // PUSH ES the accumulator-immediate time.
        //
        // PUSH seg is 10 clocks with one transfer, POP seg 8 with one.
        0x06 | 0x0E | 0x16 | 0x1E => 10 - 4,
        0x07 | 0x0F | 0x17 | 0x1F => 8 - 4,

        // -------------------------------------------------------------------
        // The ALU block, 0x00-0x3F. Seven operations share one set of numbers;
        // CMP differs because it does not write its result back, which costs it
        // one transfer and four clocks less.
        // -------------------------------------------------------------------
        0x00..=0x3F => {
            let is_cmp = (opcode >> 3) & 7 == 7;
            match opcode & 7 {
                // ALU r/m, reg: 3 for registers. In memory, 16 clocks and two
                // transfers for the read-modify-write, or 9 and one for CMP.
                0 | 1 => match (is_mem, is_cmp) {
                    (false, _) => 3,
                    (true, false) => 16 - 8,
                    (true, true) => 9 - 4,
                },
                // ALU reg, r/m: 3, or 9 clocks and one transfer from memory.
                2 | 3 => {
                    if is_mem {
                        9 - 4
                    } else {
                        3
                    }
                }
                // ALU accumulator, immediate: 4, no operand at all.
                4 | 5 => 4,
                // What is left in this range with a low three bits of 6 or 7,
                // now that the segment pushes and pops are handled above, is
                // the segment override prefixes (which never reach here as an
                // opcode) and the four BCD adjusts, which are not extracted.
                _ => 0,
            }
        }

        // -------------------------------------------------------------------
        // The immediate-to-r/m group. 17 clocks and two transfers in memory,
        // again 10 and one for CMP.
        // -------------------------------------------------------------------
        0x80..=0x83 => match (is_mem, reg == 7) {
            (false, _) => 4,
            (true, false) => 17 - 8,
            (true, true) => 10 - 4,
        },

        // TEST r/m, reg: non-destructive, so one transfer.
        0x84 | 0x85 => {
            if is_mem {
                9 - 4
            } else {
                3
            }
        }

        // XCHG r/m, reg: 4 for two registers; 17 clocks and two transfers when
        // one of them is in memory.
        0x86 | 0x87 => {
            if is_mem {
                17 - 8
            } else {
                4
            }
        }

        // MOV r/m, reg is 9 clocks and one transfer; MOV reg, r/m is 8. The
        // asymmetry is real: a store costs the EU one clock more than a load.
        0x88 | 0x89 => {
            if is_mem {
                9 - 4
            } else {
                2
            }
        }
        0x8A | 0x8B => {
            if is_mem {
                8 - 4
            } else {
                2
            }
        }
        // MOV r/m16, sreg and MOV sreg, r/m16, same two numbers.
        0x8C => {
            if is_mem {
                9 - 4
            } else {
                2
            }
        }
        0x8E => {
            if is_mem {
                8 - 4
            } else {
                2
            }
        }
        // LEA: 2 clocks and no transfers. All of its cost is the EA
        // calculation, which is charged separately, and it is the only
        // memory-addressing instruction that runs no bus cycle at all.
        0x8D => 2,
        // POP r/m16: 17 clocks and two transfers, the stack read and the
        // operand write.
        0x8F => 17 - 8,

        // XCHG AX, reg16.
        0x90..=0x97 => 3,

        // MOV accumulator to and from a direct address: 10 clocks, one
        // transfer, either way.
        0xA0..=0xA3 => 10 - 4,

        // TEST accumulator, immediate.
        0xA8 | 0xA9 => 4,

        // MOV reg, immediate.
        0xB0..=0xBF => 4,

        // MOV r/m, immediate: 10 clocks and one transfer in memory.
        0xC6 | 0xC7 => {
            if is_mem {
                10 - 4
            } else {
                4
            }
        }

        // The shifts and rotates. By one, 2 clocks in a register or 15 with two
        // transfers in memory. By CL the table quotes 8+4/bit and 20+4/bit, and
        // the per-bit part is added by the caller, which is the only place that
        // knows CL.
        0xD0 | 0xD1 => {
            if is_mem {
                15 - 8
            } else {
                2
            }
        }
        0xD2 | 0xD3 => {
            if is_mem {
                20 - 8
            } else {
                8
            }
        }

        // The unary group. TEST with an immediate is 11 clocks and one
        // transfer; NOT and NEG are 16 and two.
        //
        // Table 1-16 prints a dash in the Transfers column for TEST
        // memory,immediate, which cannot be right for an instruction that reads
        // memory, and every other TEST form with a memory operand shows one.
        // Read as one here, and flagged rather than silently corrected.
        0xF6 | 0xF7 => match reg {
            0 | 1 => {
                if is_mem {
                    11 - 4
                } else {
                    5
                }
            }
            2 | 3 => {
                if is_mem {
                    16 - 8
                } else {
                    3
                }
            }
            // MUL, IMUL, DIV and IDIV are quoted as ranges because their
            // microcode is data-dependent. Charging a single constant would
            // turn a known gap into something that looks like an answer.
            _ => 0,
        },

        // INC and DEC, both as the single-byte register forms and as the
        // group: 3 clocks in a register, 15 and two transfers in memory.
        0x40..=0x4F => 3,
        0xFE => {
            if is_mem {
                15 - 8
            } else {
                3
            }
        }
        0xFF => match reg {
            0 | 1 => {
                if is_mem {
                    15 - 8
                } else {
                    3
                }
            }
            // PUSH r/m16: 16 clocks and two transfers, the operand read and
            // the stack write.
            6 => {
                if is_mem {
                    16 - 8
                } else {
                    11 - 4
                }
            }
            // The indirect calls and jumps are control transfers, not yet
            // modeled.
            _ => 0,
        },

        // PUSH and POP with the register in the opcode: 11 and 8 clocks, one
        // transfer each. NOP needs no arm of its own: it is an XCHG of AX with
        // itself and the table gives both 3.
        0x50..=0x57 => 11 - 4,
        0x58..=0x5F => 8 - 4,

        // PUSHF and POPF, which the table gives at 10 and 8 with one transfer.
        0x9C => 10 - 4,
        0x9D => 8 - 4,

        // Everything else: not yet modeled, and charged nothing. See the module
        // documentation.
        _ => 0,
    }
}

/// Extra clocks a shift or rotate by `CL` spends, one per bit shifted.
///
/// Table 1-16 quotes these forms as `8+4/bit` and `20+4/bit`, where the base is
/// in [`eu_cycles`] and the per-bit part is here because only the caller knows
/// CL. The 8088 does not mask the count, which is why a shift by 255 really
/// does take about a thousand clocks on this part and only became a masked
/// 5-bit count on the 80186.
pub(crate) fn shift_count_cycles(count: u8) -> u16 {
    4 * count as u16
}

/// Whether this core models `opcode`'s execution time at all.
///
/// Distinguishes "costs the EU nothing" from "not yet measured", which
/// [`eu_cycles`] alone cannot, since both are zero. The per-cycle gate reports
/// the two populations apart, because a cycle count that mixes them says
/// nothing useful about either: an unmodeled opcode is short by its whole
/// microcode time, and averaging that together with the modeled ones hides how
/// well the modeled ones do.
///
/// Ascending, non-overlapping ranges, so that adding a family is a matter of
/// moving one boundary rather than working out which earlier arm shadowed it.
pub(crate) fn is_modeled(opcode: u8, modrm: u8) -> bool {
    let reg = (modrm >> 3) & 7;
    match opcode {
        // The ALU block. Its last column holds the BCD adjusts, which are not
        // extracted; the segment pushes and pops beside them are.
        0x27 | 0x2F | 0x37 | 0x3F => false,
        0x00..=0x3F => true,
        // INC, DEC, PUSH and POP with the register in the opcode.
        0x40..=0x5F => true,
        // The conditional jumps and their aliases: control transfers.
        0x60..=0x7F => false,
        // The immediate group, TEST, XCHG, MOV, LEA and POP r/m16.
        0x80..=0x8F => true,
        // XCHG with the accumulator, including NOP.
        0x90..=0x97 => true,
        // CBW, CWD, CALL far, WAIT, SAHF and LAHF.
        0x98..=0x9B | 0x9E | 0x9F => false,
        // PUSHF and POPF.
        0x9C | 0x9D => true,
        // MOV to and from a direct address.
        0xA0..=0xA3 => true,
        // MOVS and CMPS.
        0xA4..=0xA7 => false,
        // TEST with an immediate.
        0xA8 | 0xA9 => true,
        // STOS, LODS, SCAS.
        0xAA..=0xAF => false,
        // MOV with an immediate.
        0xB0..=0xBF => true,
        // The returns, the far-pointer loads, and the interrupts.
        0xC0..=0xC5 => false,
        // MOV r/m, immediate.
        0xC6 | 0xC7 => true,
        0xC8..=0xCF => false,
        // The shifts and rotates.
        0xD0..=0xD3 => true,
        // AAM, AAD, SALC, XLAT and the coprocessor escapes.
        0xD4..=0xDF => false,
        // The loops, the jumps, and the I/O instructions.
        0xE0..=0xEF => false,
        // The prefixes, HLT and CMC.
        0xF0..=0xF5 => false,
        // The unary group, modeled except for the multiplies and divides.
        0xF6 | 0xF7 => reg < 4,
        // The flag instructions.
        0xF8..=0xFD => false,
        0xFE => true,
        // INC, DEC and PUSH within the group. The indirect calls and jumps are
        // control transfers, which are not extracted.
        0xFF => matches!(reg, 0 | 1 | 6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that makes the decomposition self-checking: the EU's time
    /// does not depend on how wide the operand is, so the byte and word forms
    /// of an operation must agree. These are the opcode pairs where the only
    /// difference between the two encodings is the `w` bit.
    #[test]
    fn byte_and_word_forms_cost_the_eu_the_same() {
        let mem = 0b00_000_100; // mod=00, a memory operand
        let reg = 0b11_000_001; // mod=11, a register operand
        for (byte_op, word_op) in [
            (0x00u8, 0x01u8), // ADD r/m, reg
            (0x02, 0x03),     // ADD reg, r/m
            (0x38, 0x39),     // CMP r/m, reg
            (0x3A, 0x3B),     // CMP reg, r/m
            (0x84, 0x85),     // TEST
            (0x86, 0x87),     // XCHG
            (0x88, 0x89),     // MOV r/m, reg
            (0x8A, 0x8B),     // MOV reg, r/m
            (0xC6, 0xC7),     // MOV r/m, imm
            (0xD0, 0xD1),     // shift by 1
            (0xD2, 0xD3),     // shift by CL
            (0xF6, 0xF7),     // the unary group
            (0xFE, 0xFF),     // INC/DEC
        ] {
            assert_eq!(
                eu_cycles(byte_op, mem),
                eu_cycles(word_op, mem),
                "{byte_op:#04X} and {word_op:#04X} disagree on a memory operand"
            );
            assert_eq!(
                eu_cycles(byte_op, reg),
                eu_cycles(word_op, reg),
                "{byte_op:#04X} and {word_op:#04X} disagree on a register operand"
            );
        }
    }

    /// CMP costs less than the operations it otherwise resembles, because it
    /// does not write its result back. Getting this wrong would charge every
    /// comparison in a program four clocks it never spent.
    #[test]
    fn cmp_costs_less_than_the_alu_operations_it_resembles() {
        let mem = 0b00_000_100;
        assert_eq!(eu_cycles(0x00, mem), 8, "ADD r/m8, reg8 in memory");
        assert_eq!(eu_cycles(0x38, mem), 5, "CMP r/m8, reg8 in memory");
        // And in the immediate group, where the reg field picks the operation.
        assert_eq!(eu_cycles(0x80, 0b00_000_100), 9, "ADD r/m8, imm8");
        assert_eq!(eu_cycles(0x80, 0b00_111_100), 6, "CMP r/m8, imm8");
    }

    /// A store costs the EU one clock more than a load. The two numbers are
    /// adjacent rows in the table and easy to transpose.
    #[test]
    fn a_mov_store_costs_one_more_than_a_load() {
        let mem = 0b00_000_100;
        assert_eq!(eu_cycles(0x88, mem), 5, "MOV r/m8, reg8");
        assert_eq!(eu_cycles(0x8A, mem), 4, "MOV reg8, r/m8");
    }

    /// LEA runs no bus cycle, so all of its 2 clocks are EU time and none of
    /// them is a transfer being subtracted.
    #[test]
    fn lea_is_two_clocks_and_no_transfers() {
        assert_eq!(eu_cycles(0x8D, 0b00_000_100), 2);
    }

    /// The multiply and divide group is deliberately unmodeled rather than
    /// approximated, and says so through `is_modeled` rather than by returning
    /// a plausible number.
    #[test]
    fn multiply_and_divide_are_declared_unmodeled_rather_than_guessed() {
        for reg in 4..8u8 {
            let modrm = 0b11_000_000 | (reg << 3);
            assert_eq!(eu_cycles(0xF6, modrm), 0);
            assert!(!is_modeled(0xF6, modrm), "reg={reg}");
        }
        // The half of the group that is modeled says so.
        for reg in 0..4u8 {
            assert!(is_modeled(0xF7, 0b11_000_000 | (reg << 3)), "reg={reg}");
        }
    }

    /// The segment pushes and pops live inside the ALU block's opcode range and
    /// share none of its timing. Dispatching them through the block's
    /// `opcode & 7` would give PUSH ES the accumulator-immediate cost of 4
    /// rather than its own 6.
    #[test]
    fn segment_pushes_and_pops_are_not_alu_operations() {
        for op in [0x06u8, 0x0E, 0x16, 0x1E] {
            assert_eq!(eu_cycles(op, 0), 6, "{op:#04X} PUSH seg");
        }
        for op in [0x07u8, 0x0F, 0x17, 0x1F] {
            assert_eq!(eu_cycles(op, 0), 4, "{op:#04X} POP seg");
        }
        // The register forms cost one clock more to push and the same to pop.
        assert_eq!(eu_cycles(0x50, 0), 7, "PUSH AX");
        assert_eq!(eu_cycles(0x58, 0), 4, "POP AX");
        // PUSHF and POPF match the segment forms.
        assert_eq!(eu_cycles(0x9C, 0), 6, "PUSHF");
        assert_eq!(eu_cycles(0x9D, 0), 4, "POPF");
    }

    /// The BCD adjusts share the ALU block's range and are not extracted, so
    /// they must report as unmodeled rather than picking up a neighbour's cost.
    #[test]
    fn the_bcd_adjusts_are_unmodeled_rather_than_borrowing_a_neighbour() {
        for op in [0x27u8, 0x2F, 0x37, 0x3F] {
            assert_eq!(eu_cycles(op, 0), 0, "{op:#04X}");
            assert!(!is_modeled(op, 0), "{op:#04X}");
        }
    }

    /// A shift by CL costs four clocks a bit, and the 8088 does not mask the
    /// count.
    #[test]
    fn a_shift_by_cl_costs_four_clocks_per_bit_unmasked() {
        assert_eq!(shift_count_cycles(0), 0);
        assert_eq!(shift_count_cycles(1), 4);
        assert_eq!(shift_count_cycles(255), 1020);
    }

    /// Nothing panics and nothing returns an absurd value.
    #[test]
    fn every_opcode_and_form_gives_a_sane_answer() {
        for opcode in 0..=0xFFu8 {
            for modrm in [0b00_000_100u8, 0b11_000_001, 0b01_111_110, 0b10_100_010] {
                let c = eu_cycles(opcode, modrm);
                assert!(c <= 20, "{opcode:#04X}/{modrm:#04X} gave {c}");
            }
        }
    }
}
