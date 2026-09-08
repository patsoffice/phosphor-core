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
//! # The control transfers, where Table 1-16 does not describe the part
//!
//! Every row above is the manual's, and the hardware recording agrees with the
//! manual exactly: replay a full queue and no prefix, and the span from an
//! instruction's First Byte to the next one's *is* the documented clock count,
//! on `NOP`, on `MOV`, on `PUSH`, on the ALU block, on the flag instructions.
//!
//! The control transfers are the exception, and it is not a small one. Measured
//! the same way, over thousands of cases each and uniform to the case within
//! every one of them, the published clocks are three to eight out, in both
//! directions:
//!
//! ```text
//!                     documented   recorded span   this table
//!   JMP short             15            17            10
//!   JMP near              15            17            10
//!   JMP far               15            19            10
//!   Jcc, taken            16            17            10
//!   Jcc, not taken         4             4             4
//!   LOOP, taken           17            17            10
//!   CALL near             19+4          23             8
//!   RET near              16+4          20             5
//!   RETF                  26+8          34            11
//!   IRET                  32+12         44            13
//! ```
//!
//! So these rows are measured rather than transcribed, and the third column
//! above is what this table holds: the recorded span less everything the
//! pipeline spends on its own account. For a taken transfer that is the bus
//! cycles, plus **seven clocks** for the flush and the reload, which is the
//! queue being thrown away, the two-cycle prefetch restart, and the four
//! T-states of the fetch at the target. Those seven are not a fitted
//! correction: they are cycles this core visibly spends, and the recording
//! shows the part spending exactly seven there too, from the cycle its queue
//! status lines report the flush to the cycle they report the next First Byte.
//!
//! **What makes this a measurement rather than a fit.** Each number is read off
//! one population, the cases that begin with a full queue, and it then has to
//! predict the other half of the suite, which begins with an empty one and
//! reaches the same instruction through a completely different sequence of
//! fetches. It also has to predict the cases carrying a segment override. The
//! per-cycle gate reports both, so a number that only described what it was
//! read off would show up there rather than pass unnoticed.
//!
//! # What is not here yet
//!
//! Every instruction the suite records has a row or a rule now, but coverage is
//! not the same as being right, and the ranked sweep in `row_residuals` says
//! which rows are not. Over the clean population (a full queue and no prefix,
//! where the recorded span *is* the instruction's clock count) 92.10% of cases
//! are exact and 247 of 323 files are exact on every case. What is left, worst
//! first:
//!
//! - the coprocessor escapes `0xD8`-`0xDF`, whose operand read this core does
//!   not perform at all, and `SALC`, which has no row anywhere;
//! - a one-clock memory tail whose sign follows write-back: `-1` on the forms
//!   that only read (the multiplies, the divides, `CMP` with an immediate) and
//!   `+1` on the read-modify-write forms (`NOT`, `NEG`);
//! - the indirect transfers `FF /2`-`FF /5`, `MOV mem, imm`, `MOV r/m, sreg`
//!   and `POP r/m16`, which are multi-valued rather than offset.
//!
//! Those are rows. Separately and larger, the gate's empty-queue half is held
//! down by the loader reading its bytes on a different clock from the part,
//! which no row here can reach.
//!
//! An opcode with no entry here is charged nothing, which undercounts it;
//! [`eu_cycles`] returning zero means "not yet modeled", not "free", and
//! [`is_modeled`] is what tells the two apart.

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
        // Table 1-16 gives PUSH seg 10 clocks with one transfer and POP seg 8
        // with one.
        //
        // **The push is 11 on this part, not 10**, uniformly: -1 on all 5000
        // cases of each of the four files, where `PUSH reg`, documented at 11,
        // reads +0. `PUSHF` is documented at 10 and reads the same -1. So all
        // three push forms cost the part 11 and the table's 10 describes none
        // of them. Both numbers are here rather than the disagreement being
        // tidied away.
        0x06 | 0x0E | 0x16 | 0x1E => 11 - 4,
        0x07 | 0x0F | 0x17 | 0x1F => 8 - 4,

        // DAA and DAS sit in the same block for the same reason, and take 4
        // clocks whichever way their adjust goes. AAA and AAS are the other two
        // corners of it and are not the same shape: they take 8 or 9 depending
        // on the adjust, through [`branch_cycles`].
        0x27 | 0x2F => 4,

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
        // **`CMP` in memory is 7, not the table's `10 - 4`**, measured: all four
        // of `0x80` through `0x83` read -1 on all 24 memory modes.
        //
        // The register forms stay at the table's 4, and one of them is known
        // wrong: `0x81`, the only one of the four carrying a *16-bit*
        // immediate, reads -1 on all eight register modes for both `ADD` and
        // `CMP` where `0x80`, `0x82` and `0x83` read +0. Putting 5 here does not
        // fix it. The span does not move at all, on either opcode, so the
        // pipeline is not spending this row for that form and the missing clock
        // is somewhere between here and the span. A constant that changes no
        // output is not a measurement, so it is not here; the finding is on the
        // issue instead.
        0x80..=0x83 => match (is_mem, reg == 7) {
            (false, _) => 4,
            (true, false) => 17 - 8,
            (true, true) => 7,
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
        // transfer, either way. The recording agrees for the two loads and puts
        // the two stores one clock higher, which is the same asymmetry the
        // manual itself gives the ModR/M forms of MOV: a store costs the EU one
        // more than a load. It simply prints the same 10 for both of these.
        0xA0 | 0xA1 => 10 - 4,
        0xA2 | 0xA3 => 10 - 4 + 1,

        // XLAT, documented 11 with one transfer, recorded one clock above that.
        // Its address is BX plus AL, which the manual does not quote as an
        // effective address and this core does not charge as one.
        0xD7 => 11 - 4 + 1,

        // CBW and CWD, the sign extensions. CBW is the manual's 2. CWD is
        // quoted at 5 and takes 5 when AX is positive and 6 when it is not,
        // which is the microcode's own branch: writing 0FFFFh into DX is a
        // clock dearer than writing zero. The caller supplies that through
        // [`branch_cycles`].
        0x98 => 2,

        // SAHF, which the manual gives at 4 and the recording confirms, and
        // LAHF, which the manual gives at 4 and the part does in 2. Loading AH
        // from the flags is the cheaper direction, not the equal one.
        0x9E => 4,
        0x9F => 2,

        // The flag instructions: CLC, STC, CLI, STI, CLD, STD and CMC, all 2.
        0xF5 | 0xF8..=0xFD => 2,

        // LES and LDS, 16 clocks with two transfers: four bytes of far pointer
        // read into a segment register and a general one.
        0xC4 | 0xC5 => 16 - 8,

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
            // NOT and NEG. The register form is the table's 3. **The memory
            // form is 7, not the table's `16 - 8`**: all four of `F6 /2`,
            // `F6 /3`, `F7 /2` and `F7 /3` read +1 on all 24 memory modes and
            // +0 on all 8 register ones, so the read-modify-write path is a
            // clock dearer here than the part spends.
            2 | 3 => {
                if is_mem {
                    7
                } else {
                    3
                }
            }
            // The four multiplies and divides are functions of their operands
            // rather than of their encoding, so the caller computes them, from
            // [`multiply_cycles`], [`signed_multiply_cycles`],
            // [`divide_cycles`] and [`signed_divide_cycles`]. Those four rules
            // are calibrated on the register forms, which read +0.
            //
            // **A memory operand costs one clock more than the register form
            // plus its bus cycles and its effective address**, on all eight of
            // `F6`/`F7` `/4` through `/7` and all 24 memory modes. Table 1-16's
            // memory rows imply *two*: `MUL r/m8` is 70-77 in a register and
            // (76-83)+EA in memory, and the difference less the one transfer is
            // 2 at both widths. The recording says 1, at both widths, which is
            // the byte-and-word agreement that makes it a row rather than a
            // fudge.
            _ => u8::from(is_mem),
        },

        // INC and DEC as the single-byte register forms. **Measured at 2**,
        // uniformly: this arm held 3 and read +1 against the recording on all
        // 5000 cases of each of the sixteen files.
        //
        // The 3 came from the byte-register form the `0xFE` group encodes.
        // These opcodes have no byte form at all, so the two rows were being
        // charged one number, and nothing caught it because the per-row meter
        // had never been pointed at this range.
        0x40..=0x4F => 2,
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
            // the stack write. reg=7 is the same instruction: the group's
            // decoder does not check the top bit of the reg field.
            6 | 7 => {
                if is_mem {
                    16 - 8
                } else {
                    11 - 4
                }
            }
            // The indirect calls and jumps, measured like the direct ones.
            // Documented at 16 and 21+EA for the near call, 11 and 18+EA for
            // the near jump, 37+EA and 24+EA for the two far forms, which have
            // no register encoding at all.
            //
            // The memory forms sit one clock above their register counterparts
            // here, which is what the recording's commonest case gives and what
            // the manual's own difference between the two rows says. It is a
            // weaker number than the rest: a memory form's recorded span also
            // carries the effective address and the operand read, and those
            // have a residual of their own that is not this instruction's.
            2 => {
                if is_mem {
                    9
                } else {
                    8
                }
            }
            3 => 16,
            4 => {
                if is_mem {
                    8
                } else {
                    7
                }
            }
            5 => 10,
            _ => 0,
        },

        // PUSH and POP with the register in the opcode: 11 and 8 clocks, one
        // transfer each. NOP needs no arm of its own: it is an XCHG of AX with
        // itself and the table gives both 3.
        0x50..=0x57 => 11 - 4,
        0x58..=0x5F => 8 - 4,

        // PUSHF and POPF, which the table gives at 10 and 8 with one transfer.
        // The push is 11 on this part, for the reason under the segment pushes
        // above: every push form costs 11 and only `PUSH reg` is documented so.
        0x9C => 11 - 4,
        0x9D => 8 - 4,

        // -------------------------------------------------------------------
        // The control transfers that always transfer. Measured, for the reason
        // in the module documentation; the documented clocks are quoted beside
        // each so the disagreement stays visible rather than being tidied away.
        // -------------------------------------------------------------------

        // CALL far direct, which pushes CS and IP. Documented 28 with two
        // transfers.
        //
        // Two clocks below what the recorded span alone would give, for the
        // reason set out under `0xEA`: both are five bytes long, which is one
        // more than the queue holds, and over an instruction that long the
        // part's read schedule and this core's differ.
        //
        // Its **bus order** is still wrong, and the count cannot see it. This
        // core runs both pushes, then flushes, then reloads; the part starts
        // the fetch at the target between its two pushes:
        //
        // ```text
        //   got  [... W:CS W:CS W:IP W:IP F:target]
        //   want [... W:CS W:CS F:target W:IP W:IP]
        // ```
        //
        // Every address and byte matches. Only the interleaving differs, and
        // only the bus-cycle comparison says so.
        0x9A => 14,
        // The near returns and their undocumented aliases one encoding below,
        // popping IP. Documented 20 and 16, one transfer each.
        0xC0 | 0xC2 => 10,
        0xC1 | 0xC3 => 5,
        // The far returns, popping IP and CS. Documented 25 and 26, two
        // transfers each.
        0xC8 | 0xCA => 13,
        0xC9 | 0xCB => 11,
        // INT 3 and INT n. Documented 52 with five transfers, and the pipeline
        // now runs all five: the four bytes of the vector as MEMR cycles ahead
        // of the microcode, and the three words of flags, CS and IP as MEMW
        // cycles after it. That is the order the recording shows.
        0xCC => 24,
        0xCD => 23,
        // IRET, popping IP, CS and the flags. Documented 32, three transfers.
        0xCF => 13,
        // IN and OUT. Documented 10 with an immediate port and 8 through DX,
        // one transfer each, and the transfer is an I/O cycle the pipeline
        // runs. A word port is two of them, at consecutive port numbers.
        //
        // `IN` lands on the documented figure exactly, on all 5,000 cases of
        // each of its four encodings. `OUT` needs one clock more, uniformly,
        // which is the asymmetry the manual gives the ModR/M forms of `MOV`
        // and does not give these: a store costs the EU one more than a load.
        // The direct-address `MOV`s are the same story, printed equal in the
        // table and measured a clock apart.
        0xE4 | 0xE5 => 10 - 4,
        0xE6 | 0xE7 => 10 - 4 + 1,
        0xEC | 0xED => 8 - 4,
        0xEE | 0xEF => 8 - 4 + 1,

        // CALL near direct, pushing IP. Documented 19, one transfer.
        0xE8 => 8,
        // JMP near, far and short, none of which touch memory. All three are
        // documented at 15 and all three come out at 10, which is worth
        // noticing: the manual gives them one number and so does the part.
        //
        // The far form needed a correction the other two did not, and it is the
        // one place where a recorded span cannot be read off directly. `JMP
        // far` is five bytes long, one more than the queue holds, so its span
        // includes the EU waiting for a byte, and the part waits differently
        // from this core: it reads its opcode, idles a cycle, then takes the
        // next three back to back, where this core takes all four back to back
        // and then stalls. The microcode window is what is comparable, and the
        // recording measures it directly: from the last instruction byte read
        // to the queue flush is five cycles, and five plus its five bytes is
        // this row. Reading the span instead gives 12 and runs two clocks long
        // on every case.
        0xE9 => 10,
        0xEA => 10,
        0xEB => 10,

        // Everything else: not yet modeled, and charged nothing. See the module
        // documentation.
        _ => 0,
    }
}

/// Clocks an instruction spends when its microcode branches on the state.
///
/// These are the instructions whose cost turns on the registers or the flags
/// rather than on the encoding, so it cannot come out of [`eu_cycles`], which
/// sees only the opcode and the ModR/M byte. The caller evaluates the condition
/// before the instruction runs, which is safe because none of these changes
/// what it branches on.
///
/// Most of them are the conditional transfers, where `branch` means "this one
/// transfers". Three are not, and they are here because the recording shows two
/// costs where the manual prints one:
///
/// - `CWD` takes 5 clocks when AX is positive and 6 when it is negative.
///   Writing 0FFFFh into DX costs a clock that writing zero does not.
/// - `AAA` and `AAS` take 8 when they adjust and **9 when they do not**, which
///   is the same shape as the `MUL` flag step: the path that does less is the
///   longer one. The two groups split exactly on `(AL AND 0Fh) > 9 OR AF`, at
///   the 68.75% of cases that condition predicts from random operands, so the
///   split is the microcode's branch rather than a correlation.
///
/// Measured, like the unconditional transfers, and for the same reason. The
/// not-taken numbers are the ones the manual gets right: `Jcc` at 4 and the
/// loops at 6 are exactly what the recording shows, which is worth saying,
/// because it means the disagreement is confined to the taken path where the
/// queue is thrown away.
///
/// Two of these are **not exercised by the suite at all**, and are stated here
/// rather than left at zero:
///
/// - `LOOP` not taken needs CX to be 1 on entry, and the vectors draw CX at
///   random from sixteen bits. It is given the 6 clocks its two neighbors take
///   when they fall through, which is also what the manual gives them; the
///   manual's own 5 for `LOOP` is the same number it gets wrong for `LOOPNE`,
///   where the recording says 6.
/// - `JCXZ` taken needs CX to be 0, and for the same reason never happens. It
///   is given `LOOPE`'s taken cost, which shares its documented 18 clocks and
///   its shape: test a register, then transfer.
pub(crate) fn branch_cycles(opcode: u8, taken: bool) -> u8 {
    match opcode {
        // AAA and AAS, where `taken` is the adjust.
        0x37 | 0x3F => {
            if taken {
                8
            } else {
                9
            }
        }
        // CWD, where `taken` is AX being negative.
        0x99 => {
            if taken {
                6
            } else {
                5
            }
        }
        // Jcc and its aliases sixteen below. Documented 16 taken, 4 not.
        0x60..=0x7F => {
            if taken {
                10
            } else {
                4
            }
        }
        // LOOPNE and LOOPE. Documented 19 and 18 taken, 5 and 6 not.
        0xE0 | 0xE1 => {
            if taken {
                14
            } else {
                6
            }
        }
        // LOOP. Documented 17 taken, 5 not.
        0xE2 => {
            if taken {
                10
            } else {
                6
            }
        }
        // JCXZ. Documented 18 taken, 6 not.
        0xE3 => {
            if taken {
                14
            } else {
                6
            }
        }
        // INTO, which is an INT 4 when the overflow flag is set and four clocks
        // when it is not. Documented 53 and 4, and one clock dearer than `INT`
        // taken, which is the flag test.
        0xCE => {
            if taken {
                25
            } else {
                4
            }
        }
        _ => 0,
    }
}

/// Whether `opcode`'s microcode branches on the state, so that
/// [`branch_cycles`] gives its cost instead of [`eu_cycles`].
pub(crate) fn branches_on_state(opcode: u8) -> bool {
    matches!(opcode, 0x37 | 0x3F | 0x60..=0x7F | 0x99 | 0xCE | 0xE0..=0xE3)
}

/// Whether `opcode` is one of the conditional *transfers*, the subset of
/// [`branches_on_state`] whose branch is a control transfer and can therefore
/// be checked against what the instruction did.
///
/// Only the cross-check in [`super::I8088::run_execute_step`] asks, so this
/// exists only in a build that runs it.
#[cfg(debug_assertions)]
pub(crate) fn is_conditional_transfer(opcode: u8) -> bool {
    matches!(opcode, 0x60..=0x7F | 0xCE | 0xE0..=0xE3)
}

/// Clocks an unsigned multiply spends, given its multiplier.
///
/// `MUL` is the one instruction here whose time is a function of its operands
/// rather than of its encoding, which is why Table 1-16 quotes it as a range.
/// The mechanism is the microcode's own: a fixed loop, eight iterations for a
/// byte and sixteen for a word, testing one bit of the multiplier per pass and
/// skipping its `ADD` when that bit is zero. That structure comes from Ken
/// Shirriff's reverse-engineering of the 8086 multiply microcode from die
/// photographs, not from the vectors.
///
/// So the cost is a base plus one clock per set bit, and the multiplier is AL
/// for the byte form and AX for the word form, because the microcode begins by
/// moving the accumulator into the register it shifts.
///
/// **Both rules land on Intel's published endpoints.** `MUL r/m8` is quoted at
/// 70 to 77 clocks and a byte has one to eight set bits: `69 + 1` and
/// `69 + 8`. `MUL r/m16` is quoted at 118 to 133 and a word has one to
/// sixteen: `117 + 1` and `117 + 16`. Four endpoints from a document, four
/// hits, from two constants.
///
/// **A known residual, stated rather than smoothed over.** The word form is
/// exact on every comparable vector. The byte form is exact for five or more
/// set bits and one clock low for some cases below that, and the cause is not
/// the bit count: the same AL value appears at both counts, so it turns on
/// something else in the microcode that is not identified yet.
pub(crate) fn multiply_cycles(word: bool, multiplier: u16, product_high_zero: bool) -> u16 {
    let (base, bits) = if word {
        (117, multiplier.count_ones())
    } else {
        (69, (multiplier as u8).count_ones())
    };
    // `MUL` sets carry and overflow when the upper half of the product is
    // nonzero, and that is a branch. The path that leaves them clear is the
    // longer one by a clock, which is the opposite of what one would guess and
    // is why it had to be measured rather than assumed. With this term the byte
    // form is exact on every comparable vector, where the bit count alone left
    // 400 of 636 running one clock over.
    //
    // The word form has no case in the suite whose product fits in sixteen
    // bits, so this term is never exercised there. It is applied anyway: it is
    // the same microcode step, and leaving it off would be asserting the
    // opposite with no more evidence.
    base + bits as u16 + u16::from(product_high_zero)
}

/// What a signed multiply or divide pays for making its operands positive,
/// beyond what the unsigned form of the same instruction pays.
///
/// `PREIMUL` and `PREIDIV` each test two operands and negate the ones that are
/// negative, and the routine after the loop negates the result if exactly one
/// of them was. Each of those three points is a conditional branch, and what
/// this returns is the *difference* the branch makes: the negate path's cost
/// less the skip path's.
///
/// **Two of those differences are negative, and that is the shape of the
/// answer rather than a problem with it.** A short jump costs the 8086's
/// microcode sequencer a clock when it is taken, so a test written as "jump
/// over the negate if positive" charges the positive case for the jump and the
/// negative case for the `NEG`, and the two need not come out equal. Where the
/// negate path is the fall-through it can be the cheaper of the two, which is
/// what `-1` means here. Treating these as costs that must be non-negative is
/// what made the four sign combinations look undecomposable for two passes:
/// solving them under that constraint gives no answer at all.
///
/// **The measurement.** Recorded spans, register operands, full queue, no
/// prefix, grouped by the two signs with the loop's own terms subtracted off.
/// Every group is one value.
///
/// ```text
///                        IMUL byte  IMUL word   IDIV byte  IDIV word
///   neither negative         79        127         101        165
///   left operand negative    90        138         105        169
///   right operand negative   93        141         100        164
///   both negative            80        128         104        168
/// ```
///
/// The offsets from the first row are the same at both widths, which is the
/// evidence that this is microcode overhead rather than anything the loop does:
/// the word loop runs twice as long and pays the same correction. For `IDIV`
/// they are also the same on all three of its populations, including the fault
/// path that never reaches the loop at all.
fn sign_correction(left_negative: bool, right_negative: bool, cost: SignCost) -> i16 {
    let (left, right, result) = match cost {
        // IMUL: the multiplicand comes from the ModR/M byte and the multiplier
        // from the accumulator, and the product is the double-width value
        // `NEGATE` works on.
        SignCost::Multiply => (-1, 2, 12),
        // IDIV: the dividend is double-width, which is why negating it is the
        // expensive one, and negating the quotient costs nothing beyond the
        // path `POSTIDIV` always walks.
        SignCost::Divide => (4, -1, 0),
    };
    i16::from(left_negative) * left
        + i16::from(right_negative) * right
        + i16::from(left_negative != right_negative) * result
}

/// Which of the two signed routines [`sign_correction`] is being asked about.
#[derive(Clone, Copy)]
enum SignCost {
    /// `PREIMUL`, whose left operand is the multiplicand and right the
    /// multiplier.
    Multiply,
    /// `PREIDIV`, whose left operand is the dividend and right the divisor.
    Divide,
}

/// Clocks a signed multiply spends, given its operands.
///
/// `IMUL` is `MUL` with `PREIMUL` in front of it and `NEGATE` behind it: the
/// operands are made positive, the same `CORX` shift-and-add loop runs, and the
/// product is negated if exactly one of them was negative. Everything
/// [`multiply_cycles`] establishes therefore carries over unchanged, and the
/// recording says so:
///
/// - **The loop is the same loop.** One clock per set bit of the multiplier,
///   and the multiplier is the accumulator, exactly as for `MUL`. Grouped by
///   `popcount(|AL|)` at byte width and `popcount(|AX|)` at word width, every
///   group is uniform.
/// - **The flag step is the same step, asking the signed question.** `MUL`
///   costs one clock more when the product's upper half is zero; `IMUL` costs
///   one clock more when the product's upper half is the *sign extension* of
///   its lower, which is the same test for an instruction whose result is
///   signed. Substituting the unsigned test here leaves the groups split; the
///   signed one closes them.
/// - **The base is `MUL`'s base plus ten**, at both widths: 69 to 79 and 117 to
///   127. Those ten clocks are `PREIMUL`'s two sign tests and the check after
///   the loop, in the case where all three fall through.
///
/// What is left is [`sign_correction`], which is the only part of this that the
/// unsigned form does not already vouch for.
///
/// The word form has no case in the suite whose product sign-extends, exactly
/// as it has none whose product's upper half is zero for `MUL`. The term is
/// applied at both widths anyway, for the reason it is there: it is one
/// microcode step, and leaving it off at one width would be asserting the
/// opposite with no more evidence.
pub(crate) fn signed_multiply_cycles(
    word: bool,
    multiplicand: i32,
    multiplier: i32,
    product_sign_extends: bool,
) -> u16 {
    let base: i16 = if word { 127 } else { 79 };
    let bits = multiplier.unsigned_abs().count_ones() as i16;
    (base
        + bits
        + i16::from(product_sign_extends)
        + sign_correction(multiplicand < 0, multiplier < 0, SignCost::Multiply)) as u16
}

/// Clocks a signed divide spends, given its operands.
///
/// `IDIV` is `DIV` with `PREIDIV` in front of it, so [`divide_cycles`]'s two
/// terms carry over unchanged: the compared subtracts of the `CORD` loop, and
/// two more clocks when the last pass subtracts. Both are computed from the
/// *magnitudes*, which is what `PREIDIV` leaves the loop.
///
/// **It has three populations rather than two, and the extra one is the
/// interesting part.** `CORD` checks before it loops, and its check is the
/// unsigned one: it leaves for `INT 0` when the quotient would not fit the
/// operand's full width. A signed quotient has one bit less of room, so a
/// quotient between the two limits passes the check, runs the whole loop, and
/// only then faults. Those cases cost the loop *and* the fault, and the
/// recording separates them cleanly:
///
/// ```text
///                                        byte   word
///   quotient fits signed                  101    165   + loop
///   fits unsigned but not signed          160    224   + loop
///   CORD's check fails, or a zero divisor  89     89
/// ```
///
/// The difference between the first two rows is 59 clocks at both widths and in
/// all four sign combinations, which is what says the late fault really is the
/// same divide with an interrupt on the end of it. The early fault is one
/// number at both widths, because nothing width-dependent has run yet, and it
/// is `DIV`'s own 79 plus the ten clocks `PREIDIV` costs when neither operand
/// needs negating.
///
/// What this returns is 31 clocks below the recorded span on the faulting
/// paths, for the reason [`divide_cycles`] is: the pipeline itself spends 24 on
/// the interrupt's six stack writes and 7 on the flush and reload.
pub(crate) fn signed_divide_cycles(word: bool, dividend: i64, divisor: i64) -> u16 {
    let signs = sign_correction(dividend < 0, divisor < 0, SignCost::Divide);
    let width: u32 = if word { 16 } else { 8 };
    /// Six stack writes, the queue flush and the reload at the handler, which
    /// the pipeline spends itself on any path that faults.
    const PIPELINE: i16 = 31;

    let magnitude = dividend.unsigned_abs();
    let divisor_magnitude = divisor.unsigned_abs();
    if divisor == 0 || magnitude >> width >= divisor_magnitude {
        return (89 + signs - PIPELINE) as u16;
    }

    let quotient = magnitude / divisor_magnitude;
    let (compared, last_bit) = cord(magnitude as u32, divisor_magnitude as u32, width);
    let base: i16 = if word { 165 } else { 101 };
    // The late fault: the loop ran, and the quotient turned out not to fit the
    // signed range the destination half holds.
    let late = if quotient > (1u64 << (width - 1)) - 1 {
        59 - PIPELINE
    } else {
        0
    };
    (base + compared as i16 + 2 * i16::from(last_bit) + signs + late) as u16
}

/// Clocks an unsigned divide spends, given its operands.
///
/// Like `MUL`, quoted as a range because the microcode's loop is
/// data-dependent, and like `MUL` the structure comes from Shirriff's
/// reverse-engineering rather than from the vectors. `CORD` runs a long
/// division, shifting the dividend left each pass and taking one of three
/// paths: straight to the subtract when the bit shifted out of the top means
/// the value must exceed the divisor; a compare and then a subtract; or a
/// compare and no subtract.
///
/// **Only the middle path costs extra.** That is a measured fact and a
/// surprising one: the jump-straight-to-subtract path costs the same as not
/// subtracting at all, so the count follows the number of *compared* subtracts
/// and is independent of how many immediate ones there were. A rule built on
/// the quotient's bit count cannot express that, because the first two paths
/// both set a quotient bit; this is why counting them separately is the only
/// thing that works.
///
/// A divide error is a different path entirely. `CORD` compares before it
/// loops and leaves for `INT 0` at once, so it costs the same whatever caused
/// it, at either width: the recorded span is 79 clocks on every faulting case.
///
/// The number here is 48 rather than 79 because 31 of those clocks are ones
/// the pipeline spends itself. A fault takes an interrupt, and this core writes
/// its three words onto the stack over six MEMW bus cycles and then flushes and
/// reloads the queue, which is 24 and 7. What is *not* subtracted is the
/// interrupt vector read: a fault is conditional on the operands, so the
/// pipeline cannot know to read the vector ahead of the instruction the way it
/// does for `INT`, and the executor still reads it off the bus in no time.
/// Sixteen clocks of that are inside this number.
///
/// **And the last pass costs two clocks more when it subtracts.** That was the
/// residual this rule carried for a while: the compared-subtract count
/// predicted the floor of every group and some cases ran up to two clocks over
/// it. The quotient's top bit was tried and so was a zero remainder, and
/// neither splits the groups; the quotient's *low* bit splits them exactly, at
/// both widths, with every group uniform.
///
/// It is a rule rather than a fitted term because `AAM` confirms it
/// independently. `AAM` divides AL by its immediate through this same `CORD`
/// loop, and its spans follow `77 + compared + 2 x (quotient is odd)`, the same
/// two clocks on the same condition, over a different opcode and a different
/// operand range. See [`aam_cycles`].
pub(crate) fn divide_cycles(word: bool, dividend: u32, divisor: u32) -> u16 {
    let width = if word { 16 } else { 8 };
    let limit = (1u64 << width) - 1;
    if divisor == 0 || u64::from(dividend) / u64::from(divisor) > limit {
        // The divide error, which never enters the loop.
        return 48;
    }

    let (compared, last_bit) = cord(dividend, divisor, width);
    let base = if word { 144 } else { 80 };
    base + compared + 2 * u16::from(last_bit)
}

/// Step the microcode's long division and report what its cost depends on: how
/// many passes compared before subtracting, and whether the last pass set its
/// quotient bit.
///
/// Shared by [`divide_cycles`] and [`aam_cycles`], which is the point: they are
/// the same `CORD` routine reached from two opcodes, and writing the walk twice
/// would let the two drift while both looked right.
fn cord(dividend: u32, divisor: u32, width: u32) -> (u16, bool) {
    let mask = (1u32 << width) - 1;
    let top = 1u32 << (width - 1);
    let mut a = (dividend >> width) & mask;
    let mut c = dividend & mask;
    let mut qbit = 0u32;
    let mut compared = 0u16;

    for _ in 0..width {
        let carry_out = (a & top) != 0;
        a = ((a << 1) & mask) | u32::from((c & top) != 0);
        c = ((c << 1) & mask) | qbit;
        if carry_out {
            a = a.wrapping_sub(divisor) & mask;
            qbit = 1;
        } else if a >= divisor {
            compared += 1;
            a -= divisor;
            qbit = 1;
        } else {
            qbit = 0;
        }
    }

    (compared, qbit == 1)
}

/// Clocks `AAM` spends, given AL and the immediate it divides by.
///
/// `AAM` is a divide wearing a BCD adjust's name: it puts `AL / imm` in AH and
/// `AL mod imm` in AL, through the same `CORD` loop [`divide_cycles`] walks.
/// So it follows the same rule with its own base, and the recording says so
/// over the whole `D4` file with every group uniform:
///
/// ```text
/// 77 + compared subtracts + 2 x (the quotient is odd)
/// ```
///
/// **That is what makes the two-clock term a rule rather than a fudge.** It was
/// found here, on an opcode with an 8-bit dividend and a range of immediates,
/// and it then predicted `DIV`'s residual at both widths without adjustment.
///
/// A zero immediate is a divide error, which never enters the loop: the
/// recorded span is 77, the same as the base, and 46 is what is left after the
/// 31 clocks the pipeline spends pushing, flushing and reloading. See
/// [`divide_cycles`], whose fault path is the same one two clocks up.
///
/// Table 1-16 quotes the whole instruction at 83.
pub(crate) fn aam_cycles(al: u8, imm: u8) -> u16 {
    if imm == 0 {
        return 46;
    }
    let (compared, last_bit) = cord(u32::from(al), u32::from(imm), 8);
    77 + compared + 2 * u16::from(last_bit)
}

/// Clocks `AAD` spends, given the immediate it multiplies by.
///
/// The mirror of `AAM`: a multiply wearing a BCD adjust's name, folding AH into
/// AL by multiplying it by the immediate, through the same shift-and-add loop
/// `MUL` uses. It follows the same rule, one clock per set bit of the
/// multiplier, and the recording says which operand that is: grouped by the set
/// bits of the **immediate** every group is uniform, 59 through 67, and grouped
/// by the set bits of AH nothing separates at all.
///
/// That is the opposite of `MUL`, where the accumulator is the multiplier. It
/// is not a contradiction: the microcode moves a different operand into the
/// register it shifts. Table 1-16 quotes the instruction at 60, which is what
/// this gives for a one-bit immediate, the commonest by far in real code
/// because the immediate is nearly always 10.
pub(crate) fn aad_cycles(imm: u8) -> u16 {
    59 + imm.count_ones() as u16
}

/// Clocks one iteration of a string operation spends, beyond its bus cycles.
///
/// Table 1-16 gives these twice: once for a single operation and once for a
/// repeated one, quoted as `9 + 17/rep`. The two halves are not the same
/// number and the difference is real: a repeated `LODS` costs more per
/// iteration than a lone one, a repeated `CMPS` one more, and a repeated `MOVS`
/// exactly the same.
///
/// The decomposition is the usual one and self-checks the usual way, because
/// the microcode does not know how wide its operand is. `MOVS` is 18 clocks
/// with two transfers, so on the 8088 its word form is 26 with four bus cycles;
/// `18 - 8` and `26 - 16` are both 10, less the opcode byte the loader pulls.
///
/// What is not here is the `REP` prefix's own setup, the `9 +` half of the
/// quotation. That is [`string_entry_cycles`].
/// A single operation is one clock dearer than the manual's number, on four of
/// the five. `MOVS`, `STOS`, `LODS` and `SCAS` each run a clock longer than
/// `documented - bus - opcode byte` predicts, uniformly over every unprefixed
/// case in their files, and `CMPS` lands on it exactly. The four are corrected
/// here and the odd one out is left alone rather than averaged with them.
pub(crate) fn string_cycles(opcode: u8, repeated: bool) -> u8 {
    match opcode {
        // MOVS: 18 alone and 17 repeated, two transfers either way.
        0xA4 | 0xA5 => {
            if repeated {
                9
            } else {
                10
            }
        }
        // CMPS: 22 both ways, two transfers.
        0xA6 | 0xA7 => {
            if repeated {
                14
            } else {
                13
            }
        }
        // STOS: 11 alone, 10 repeated, one transfer.
        0xAA | 0xAB => {
            if repeated {
                6
            } else {
                7
            }
        }
        // LODS: 12 alone, 13 repeated, one transfer. The one operation the
        // table makes *dearer* to repeat.
        0xAC | 0xAD => {
            if repeated {
                9
            } else {
                8
            }
        }
        // SCAS: 15 both ways, one transfer.
        0xAE | 0xAF => 11,
        _ => 0,
    }
}

/// Clocks a `REP` prefix spends before its first iteration.
///
/// The `9 +` in Table 1-16's `9 + 17/rep`, less the two bytes the loader pulls
/// for the prefix and the opcode behind it. A repeated operation whose count is
/// already zero spends this and nothing else, which is the one way a string
/// operation runs no bus cycle at all.
pub(crate) fn string_entry_cycles(repeated: bool) -> u8 {
    if repeated { 7 } else { 0 }
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
        // The ALU block, its last column's four BCD adjusts included: DAA and
        // DAS from the manual, AAA and AAS from their microcode branch.
        0x00..=0x3F => true,
        // INC, DEC, PUSH and POP with the register in the opcode.
        0x40..=0x5F => true,
        // The conditional jumps and their aliases.
        0x60..=0x7F => true,
        // The immediate group, TEST, XCHG, MOV, LEA and POP r/m16.
        0x80..=0x8F => true,
        // XCHG with the accumulator, including NOP.
        0x90..=0x97 => true,
        // CBW, CWD, SAHF and LAHF. WAIT is not in the suite and is not modeled.
        0x98 | 0x99 | 0x9E | 0x9F => true,
        0x9B => false,
        // CALL far direct.
        0x9A => true,
        // PUSHF and POPF.
        0x9C | 0x9D => true,
        // MOV to and from a direct address.
        0xA0..=0xA3 => true,
        // MOVS and CMPS.
        0xA4..=0xA7 => true,
        // TEST with an immediate.
        0xA8 | 0xA9 => true,
        // STOS, LODS, SCAS.
        0xAA..=0xAF => true,
        // MOV with an immediate.
        0xB0..=0xBF => true,
        // The near returns and their aliases.
        0xC0..=0xC3 => true,
        // LES and LDS, the far-pointer loads.
        0xC4 | 0xC5 => true,
        // MOV r/m, immediate.
        0xC6 | 0xC7 => true,
        // The far returns and their aliases, the interrupts and IRET.
        0xC8..=0xCF => true,
        // The shifts and rotates.
        0xD0..=0xD3 => true,
        // AAM and AAD, from the divide and multiply loops their microcode runs.
        0xD4 | 0xD5 => true,
        // SALC, which Intel does not document and which has no row anywhere.
        0xD6 => false,
        0xD7 => true,
        // The coprocessor escapes, which this core does not perform the operand
        // read for at all.
        0xD8..=0xDF => false,
        // The loops and JCXZ.
        0xE0..=0xE3 => true,
        // IN and OUT, with an immediate port and through DX.
        0xE4..=0xE7 => true,
        // CALL near, and the three direct jumps.
        0xE8..=0xEB => true,
        0xEC..=0xEF => true,
        // The prefixes and HLT, which the suite does not record.
        0xF0..=0xF4 => false,
        // CMC.
        0xF5 => true,
        // The unary group. TEST, NOT and NEG come from the table, and the four
        // multiplies and divides from their microcode loops.
        0xF6 | 0xF7 => true,
        // The flag instructions.
        0xF8..=0xFD => true,
        0xFE => true,
        // INC, DEC, PUSH and the four indirect transfers, and reg=7, which the
        // part decodes as PUSH again.
        0xFF => {
            let _ = reg;
            true
        }
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
    ///
    /// The immediate group's memory figure is 7 rather than the table's
    /// `10 - 4`, measured: all four of `0x80` through `0x83` read -1 on all 24
    /// memory modes. The property this test is about is unaffected, which is
    /// the point of asserting the relation and not only the numbers.
    #[test]
    fn cmp_costs_less_than_the_alu_operations_it_resembles() {
        let mem = 0b00_000_100;
        assert_eq!(eu_cycles(0x00, mem), 8, "ADD r/m8, reg8 in memory");
        assert_eq!(eu_cycles(0x38, mem), 5, "CMP r/m8, reg8 in memory");
        // And in the immediate group, where the reg field picks the operation.
        assert_eq!(eu_cycles(0x80, 0b00_000_100), 9, "ADD r/m8, imm8");
        assert_eq!(eu_cycles(0x80, 0b00_111_100), 7, "CMP r/m8, imm8");
        assert!(
            eu_cycles(0x80, 0b00_111_100) < eu_cycles(0x80, 0b00_000_100),
            "CMP must stay cheaper than the operations that write back"
        );
    }

    /// A memory operand costs the multiplies and divides one clock beyond their
    /// register form, its bus cycles and its effective address. Table 1-16's
    /// memory rows imply two; the recording says one, at both widths.
    #[test]
    fn a_memory_operand_costs_the_multiplies_one_clock() {
        for op in [0xF6u8, 0xF7] {
            for reg in [4u8, 5, 6, 7] {
                assert_eq!(eu_cycles(op, 0b11_000_000 | (reg << 3)), 0, "register form");
                assert_eq!(eu_cycles(op, 0b00_000_100 | (reg << 3)), 1, "memory form");
            }
        }
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

    /// The four multiplies and divides are modeled, but through their own
    /// functions rather than through [`eu_cycles`], which knows nothing about
    /// their operands and so returns zero for all four. Their `is_modeled` must
    /// say so anyway: it is what stops the gate reporting them alongside the
    /// opcodes that really are uncounted.
    #[test]
    fn the_multiplies_and_divides_are_modeled_outside_the_table() {
        for reg in [4u8, 5, 6, 7] {
            let modrm = 0b11_000_000 | (reg << 3);
            assert_eq!(eu_cycles(0xF6, modrm), 0);
            assert!(is_modeled(0xF6, modrm), "reg={reg}");
            assert!(is_modeled(0xF7, modrm), "reg={reg}");
        }
        for reg in [0u8, 1, 2, 3] {
            assert!(is_modeled(0xF7, 0b11_000_000 | (reg << 3)), "reg={reg}");
        }
    }

    /// A divide error never enters the loop, so no operand changes its cost,
    /// and the width does not either. The recorded span is 79 at both; 48 is
    /// what is left once the pipeline's own pushes, flush and reload come out.
    #[test]
    fn a_divide_error_costs_the_same_however_it_was_caused() {
        // Division by zero.
        assert_eq!(divide_cycles(false, 0x1234, 0), 48);
        assert_eq!(divide_cycles(true, 0x1234_5678, 0), 48);
        // And a quotient too large for the destination.
        assert_eq!(divide_cycles(false, 0xFF00, 1), 48);
        assert_eq!(divide_cycles(true, 0xFFFF_0000, 1), 48);
        // AAM's is the same path two clocks below it.
        assert_eq!(aam_cycles(0x42, 0), 46);
    }

    /// `AAM` and `DIV` walk the same long division, so over the same operands
    /// they must differ by exactly the gap between their two bases, whatever
    /// the operands do to the loop. That is the check that ties the two rules
    /// together: the two-clock term for a last pass that subtracts was found on
    /// `AAM` and is what closed `DIV`'s residual, and if either drifted this
    /// would stop holding.
    #[test]
    fn aam_and_divide_walk_the_same_loop() {
        for imm in 1..=255u8 {
            for al in [0u8, 1, 7, 8, 9, 10, 63, 64, 127, 128, 200, 255] {
                assert_eq!(
                    divide_cycles(false, u32::from(al), u32::from(imm)) - aam_cycles(al, imm),
                    3,
                    "AL={al} imm={imm}"
                );
            }
        }
    }

    /// The odd-quotient term is not separable from the compared-subtract count
    /// by choosing operands: a last pass that subtracts is a compared subtract
    /// too, nearly always, so an odd quotient costs three rather than two more
    /// than its even neighbor. That is why the term had to be found by
    /// grouping thousands of recorded cases rather than by picking a pair, and
    /// it is why the check above is a relation between two instructions rather
    /// than an arithmetic identity.
    #[test]
    fn the_odd_quotient_term_does_not_stand_on_its_own() {
        assert_eq!(
            divide_cycles(false, 202, 2),
            divide_cycles(false, 200, 2) + 3
        );
    }

    /// `AAD` multiplies by its immediate, one clock a set bit, which is the
    /// opposite operand from the one `MUL` tests.
    #[test]
    fn the_ascii_multiply_counts_the_immediates_bits() {
        assert_eq!(aad_cycles(0), 59);
        assert_eq!(aad_cycles(0x0A), 61, "the usual base of ten, two bits");
        assert_eq!(aad_cycles(0xFF), 67);
    }

    /// The count follows the compared subtracts and ignores the immediate
    /// ones, which is the measured fact a quotient-based rule cannot express.
    #[test]
    fn divide_timing_lands_inside_the_published_range() {
        // MUL r/m8's neighbour in the table is quoted at 80 to 90 clocks.
        for divisor in 1..=255u32 {
            for dividend in [1u32, 0x0100, 0x3FFF, 0x7F00] {
                if dividend / divisor > 0xFF {
                    continue;
                }
                let c = divide_cycles(false, dividend, divisor);
                assert!((80..=90).contains(&c), "{dividend}/{divisor} gave {c}");
            }
        }
    }

    /// The multiply rules must land on the clock ranges Intel published, which
    /// is the check that does not come from the vectors they were read off.
    #[test]
    fn the_multiply_rules_reproduce_the_published_ranges() {
        // The published ranges are for a product whose high half is nonzero,
        // which is the ordinary case and the only one the suite exercises for
        // the word form.
        //
        // MUL r/m8 is quoted at 70 to 77 clocks; a byte has one to eight set
        // bits.
        assert_eq!(multiply_cycles(false, 0x0001, false), 70);
        assert_eq!(multiply_cycles(false, 0x00FF, false), 77);
        // MUL r/m16 at 118 to 133; a word has one to sixteen.
        assert_eq!(multiply_cycles(true, 0x0001, false), 118);
        assert_eq!(multiply_cycles(true, 0xFFFF, false), 133);
    }

    /// The four sign combinations, as they were measured, at both widths. The
    /// point of pinning both is that the offsets between them are the same at
    /// each: the byte form fixes three constants, and the word form's three
    /// numbers are then predictions rather than measurements.
    #[test]
    fn the_signed_multiply_reproduces_its_measured_sign_combinations() {
        // One set bit in the multiplier, and a product too wide to sign-extend,
        // so what is printed is the base plus one.
        for (word, base) in [(false, 79), (true, 127)] {
            let (positive, negative) = if word {
                (0x0100, -0x0100)
            } else {
                (0x10, -0x10)
            };
            let other = if word { 0x0FF0 } else { 0x7F };
            assert_eq!(
                signed_multiply_cycles(word, other, positive, false),
                base + 1,
                "neither negative, word={word}"
            );
            assert_eq!(
                signed_multiply_cycles(word, -other, positive, false),
                base + 1 + 11,
                "multiplicand negative, word={word}"
            );
            assert_eq!(
                signed_multiply_cycles(word, other, negative, false),
                base + 1 + 14,
                "multiplier negative, word={word}"
            );
            assert_eq!(
                signed_multiply_cycles(word, -other, negative, false),
                base + 1 + 1,
                "both negative, word={word}"
            );
        }
    }

    /// `IMUL` costs exactly ten clocks more than `MUL` when nothing needs
    /// negating, at both widths. Those ten are the two sign tests and the check
    /// after the loop, all three falling through, and having them fall out of
    /// two independently measured bases is what says the two rules describe the
    /// same loop.
    #[test]
    fn the_signed_multiply_is_the_unsigned_one_plus_ten() {
        for (word, multiplier) in [(false, 0x0F), (true, 0x0FFF)] {
            assert_eq!(
                signed_multiply_cycles(word, 1, multiplier, false),
                multiply_cycles(word, multiplier as u16, false) + 10,
                "word={word}"
            );
        }
    }

    /// `IDIV`'s three populations, at both widths. The early fault is one
    /// number at both, because nothing width-dependent has run when `CORD`
    /// leaves; the late fault is the ordinary path plus a constant 59, less the
    /// 31 clocks the pipeline spends on the interrupt itself.
    #[test]
    fn the_signed_divide_reproduces_its_three_populations() {
        for (word, base) in [(false, 101u16), (true, 165)] {
            let (dividend, divisor) = if word { (0x4000, 0x1000) } else { (0x40, 0x10) };
            // 0x40 / 0x10 is 4: an even quotient, so no last-pass term.
            let plain = signed_divide_cycles(word, dividend, divisor);
            assert!(plain >= base, "word={word} gave {plain}");
            // Negating the dividend costs four and the divisor minus one, on
            // every population.
            assert_eq!(
                signed_divide_cycles(word, -dividend, divisor),
                plain + 4,
                "word={word}"
            );
            assert_eq!(
                signed_divide_cycles(word, dividend, -divisor),
                plain - 1,
                "word={word}"
            );
            assert_eq!(
                signed_divide_cycles(word, -dividend, -divisor),
                plain + 3,
                "word={word}"
            );
            // A quotient that does not fit the signed range but does fit the
            // unsigned one runs the loop and then faults, so its cost is the
            // ordinary one plus 59, less the 31 the pipeline spends itself.
            let late: i64 = if word { 0x9000 } else { 0x90 };
            let width = if word { 16 } else { 8 };
            let (compared, last_bit) = cord(late as u32, 1, width);
            assert_eq!(
                signed_divide_cycles(word, late, 1),
                base + compared + 2 * u16::from(last_bit) + 59 - 31,
                "word={word}"
            );
            // And CORD's own check, which leaves before the loop: 89 recorded,
            // 31 of which the pipeline spends.
            assert_eq!(signed_divide_cycles(word, dividend, 0), 89 - 31);
            let huge = if word { 0x7FFF_FFFF } else { 0x7FFF };
            assert_eq!(signed_divide_cycles(word, huge, 1), 89 - 31);
        }
    }

    /// A product that fits in the low half costs one clock more, because the
    /// microcode's path that leaves carry and overflow clear is the longer one.
    #[test]
    fn a_product_with_a_zero_high_half_costs_one_more() {
        assert_eq!(multiply_cycles(false, 0x0001, true), 71);
        assert_eq!(multiply_cycles(false, 0x00FF, true), 78);
    }

    /// The byte form looks at AL alone. Reading AX would make the high byte,
    /// which the instruction overwrites with its result, change how long it
    /// takes.
    #[test]
    fn the_byte_multiply_ignores_the_high_half_of_the_accumulator() {
        assert_eq!(
            multiply_cycles(false, 0x0001, false),
            multiply_cycles(false, 0xFF01, false),
            "AH must not affect a byte multiply"
        );
        // Whereas the word form counts the whole accumulator.
        assert_ne!(
            multiply_cycles(true, 0x0001, false),
            multiply_cycles(true, 0xFF01, false)
        );
    }

    /// The segment pushes and pops live inside the ALU block's opcode range and
    /// share none of its timing. Dispatching them through the block's
    /// `opcode & 7` would give PUSH ES the accumulator-immediate cost of 4
    /// rather than its own 6.
    #[test]
    fn segment_pushes_and_pops_are_not_alu_operations() {
        for op in [0x06u8, 0x0E, 0x16, 0x1E] {
            assert_eq!(eu_cycles(op, 0), 7, "{op:#04X} PUSH seg");
        }
        for op in [0x07u8, 0x0F, 0x17, 0x1F] {
            assert_eq!(eu_cycles(op, 0), 4, "{op:#04X} POP seg");
        }
        assert_eq!(eu_cycles(0x50, 0), 7, "PUSH AX");
        assert_eq!(eu_cycles(0x58, 0), 4, "POP AX");
        assert_eq!(eu_cycles(0x9C, 0), 7, "PUSHF");
        assert_eq!(eu_cycles(0x9D, 0), 4, "POPF");
    }

    /// Every push costs the part the same, whatever it is pushing, and the pops
    /// likewise. Table 1-16 says otherwise for four of the six: it gives the
    /// segment pushes and `PUSHF` 10 clocks against `PUSH reg`'s 11. The
    /// recording is uniform against all three, so the rows agree here and
    /// disagree with the manual, which is the way round worth pinning.
    #[test]
    fn every_push_costs_the_same_whatever_it_pushes() {
        let push = eu_cycles(0x50, 0);
        for op in [0x06u8, 0x0E, 0x16, 0x1E, 0x9C] {
            assert_eq!(eu_cycles(op, 0), push, "{op:#04X} against PUSH reg");
        }
        let pop = eu_cycles(0x58, 0);
        for op in [0x07u8, 0x0F, 0x17, 0x1F, 0x9D] {
            assert_eq!(eu_cycles(op, 0), pop, "{op:#04X} against POP reg");
        }
    }

    /// The single-byte `INC`/`DEC` opcodes encode a *word* register and are
    /// measured at 2, where the `0xFE` group's byte-register form is 3.
    /// Charging both 3 is the error this caught, and it was worth 80,000
    /// vectors.
    #[test]
    fn the_single_byte_increments_are_the_word_form() {
        for op in 0x40u8..=0x4F {
            assert_eq!(eu_cycles(op, 0), 2, "{op:#04X}");
        }
        assert_eq!(eu_cycles(0xFE, 0b11_000_000), 3, "INC reg8");
    }

    /// The BCD adjusts share the ALU block's opcode range and none of its
    /// timing, so dispatching them through the block would give `DAA` the
    /// accumulator-immediate cost of 4 by coincidence and `AAA` the same, when
    /// the part takes 8 or 9 over them.
    #[test]
    fn the_bcd_adjusts_are_not_alu_operations() {
        for op in [0x27u8, 0x2F] {
            assert_eq!(eu_cycles(op, 0), 4, "{op:#04X}");
            assert!(is_modeled(op, 0), "{op:#04X}");
        }
        for op in [0x37u8, 0x3F] {
            assert!(is_modeled(op, 0), "{op:#04X}");
            assert!(branches_on_state(op), "{op:#04X}");
            assert!(branch_cycles(op, true) >= 8, "{op:#04X}");
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

    /// Nothing panics and nothing returns an absurd value. The interrupts are
    /// the only rows above twenty clocks, and they are there because an
    /// interrupt really is a forty-clock instruction: three pushes, two words
    /// of vector, and a transfer.
    #[test]
    fn every_opcode_and_form_gives_a_sane_answer() {
        for opcode in 0..=0xFFu8 {
            for modrm in [0b00_000_100u8, 0b11_000_001, 0b01_111_110, 0b10_100_010] {
                let c = eu_cycles(opcode, modrm);
                let limit = if matches!(opcode, 0xCC | 0xCD) {
                    40
                } else {
                    20
                };
                assert!(c <= limit, "{opcode:#04X}/{modrm:#04X} gave {c}");
            }
        }
    }

    /// A conditional transfer costs more when it transfers, and every one of
    /// them is more expensive taken than not. Getting the sense of the
    /// condition backwards would still produce two plausible numbers, and the
    /// only thing that would notice is the gate.
    #[test]
    fn a_conditional_transfer_costs_more_when_it_transfers() {
        for opcode in [0x60u8, 0x70, 0x7F, 0xE0, 0xE1, 0xE2, 0xE3, 0xCE] {
            assert!(
                branch_cycles(opcode, true) > branch_cycles(opcode, false),
                "{opcode:#04X}"
            );
            assert!(branches_on_state(opcode), "{opcode:#04X}");
        }
        // And nothing else branches on the state, in particular the
        // unconditional transfers sitting next to them in the opcode map.
        for opcode in [0x9Au8, 0xC3, 0xCB, 0xCF, 0xE8, 0xE9, 0xEA, 0xEB] {
            assert!(!branches_on_state(opcode), "{opcode:#04X}");
            assert!(eu_cycles(opcode, 0) > 0, "{opcode:#04X}");
        }
    }

    /// The three rows that branch on the state without transferring, where the
    /// path that does *less* work is the longer one. That is the surprising
    /// direction in two of the three, and writing either of them the natural
    /// way round would look entirely reasonable.
    #[test]
    fn the_adjusts_and_the_sign_extension_branch_the_other_way() {
        // AAA and AAS: adjusting is the fast path.
        for opcode in [0x37u8, 0x3F] {
            assert_eq!(branch_cycles(opcode, true), 8, "{opcode:#04X} adjusting");
            assert_eq!(branch_cycles(opcode, false), 9, "{opcode:#04X} not");
            assert!(branches_on_state(opcode));
        }
        // CWD: extending a negative value costs the extra clock.
        assert_eq!(branch_cycles(0x99, true), 6, "AX negative");
        assert_eq!(branch_cycles(0x99, false), 5, "AX positive");
        // DAA and DAS sit beside AAA and AAS and do not branch at all.
        for opcode in [0x27u8, 0x2F] {
            assert!(!branches_on_state(opcode), "{opcode:#04X}");
            assert_eq!(eu_cycles(opcode, 0), 4, "{opcode:#04X}");
        }
    }

    /// The five opcodes that reach memory without a ModR/M byte are modeled,
    /// and a store costs one clock more than a load even here, where Table 1-16
    /// prints the same number for both.
    #[test]
    fn the_direct_address_moves_and_xlat_are_modeled() {
        assert_eq!(eu_cycles(0xA0, 0), 6, "MOV AL, [addr]");
        assert_eq!(eu_cycles(0xA1, 0), 6, "MOV AX, [addr]");
        assert_eq!(eu_cycles(0xA2, 0), 7, "MOV [addr], AL");
        assert_eq!(eu_cycles(0xA3, 0), 7, "MOV [addr], AX");
        assert_eq!(eu_cycles(0xD7, 0), 8, "XLAT");
        for opcode in [0xA0u8, 0xA1, 0xA2, 0xA3, 0xD7] {
            assert!(is_modeled(opcode, 0), "{opcode:#04X}");
        }
    }

    /// The undocumented aliases cost what the documented encodings cost. The
    /// part decodes 0xC0 as 0xC2 and 0xC8 as 0xCA, and the recording gives the
    /// pairs identical spans.
    #[test]
    fn the_return_aliases_cost_what_they_alias() {
        assert_eq!(eu_cycles(0xC0, 0), eu_cycles(0xC2, 0));
        assert_eq!(eu_cycles(0xC1, 0), eu_cycles(0xC3, 0));
        assert_eq!(eu_cycles(0xC8, 0), eu_cycles(0xCA, 0));
        assert_eq!(eu_cycles(0xC9, 0), eu_cycles(0xCB, 0));
    }

    /// A return that pops arguments off the stack costs more than one that does
    /// not, and a far return costs more than a near one. Both orderings are
    /// easy to transpose and neither would look wrong on its own.
    #[test]
    fn the_returns_are_ordered_by_how_much_they_do() {
        assert!(eu_cycles(0xC2, 0) > eu_cycles(0xC3, 0), "RET n over RET");
        assert!(eu_cycles(0xCB, 0) > eu_cycles(0xC3, 0), "RETF over RET");
        assert!(eu_cycles(0xCA, 0) > eu_cycles(0xCB, 0), "RETF n over RETF");
    }
}
