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
        // `POP CS`, the one member of the pop family without a routine. It is a
        // pop on this part and a prefix escape on every later one. Matched
        // ahead of the ALU block so that its `opcode & 7` dispatch cannot
        // charge it the accumulator-immediate time.
        //
        // Table 1-16 gives POP seg 8 clocks with one transfer.
        0x0F => 8 - 4,

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
            match opcode & 7 {
                // The ModR/M forms, `ALU r/m, reg` and `ALU reg, r/m`, have no
                // rows: they run the transcribed routine at 0x008.
                0..=3 => 0,
                // `ALU accumulator, immediate` has no row either: it runs the
                // routine at 0x018, whose only clock is the jump over the
                // immediate's second queue read.
                4 | 5 => 0,
                // What is left in this range with a low three bits of 6 or 7,
                // now that the segment pushes and pops are handled above, is
                // the segment override prefixes (which never reach here as an
                // opcode) and the four BCD adjusts, which are not extracted.
                _ => 0,
            }
        }

        // The immediate-to-r/m group has no rows: it runs the routine at 0x00c.
        //
        // Its old rows carried a puzzle the routine answers. `0x81`, the only
        // one of the four with a real sixteen-bit immediate, read -1 on all
        // eight register modes where `0x80`, `0x82` and `0x83` read +0, and
        // raising its row did not move the span at all. The reason is that the
        // clock is not the row's: the other three jump over the immediate's
        // second queue read and `0x81` does not, so the difference belongs to
        // the microcode rather than to a number fitted to it. `0x83` takes that
        // jump too, being a word-sized instruction with a byte-sized immediate.

        // `TEST r/m, reg` at 84 and 85 has no row: it prices from its microcode,
        // which is one clock at 0x094 and nothing else. See `microcode::routine`.

        // `XCHG r/m, reg` has no row: it runs the transcribed routine at 0x0a4,
        // which spends 0x0a6 and 0x0a7 only when the ModR/M operand is in
        // memory. See `microcode::routine`.

        // `MOV r/m, reg` and `MOV reg, r/m` have no rows: they run the
        // transcribed routine at 0x000. The store direction costs one clock
        // more than the load, and the routine says why rather than asserting
        // it: 0x000 and 0x001 sit in front of the write back and there is
        // nothing in front of a register destination.

        // `MOV r/m, sreg` and `MOV sreg, r/m` have no rows either: they run the
        // transcribed routine at 0x0ec, which spends 0x0ec only when the
        // destination is an address.
        // `LEA` has no row: it runs the transcribed routine at 0x004, whose two
        // clocks are the way out of the address routine for a form that reads
        // nothing.
        // `POP r/m16` has no row: it runs the transcribed routine at 0x040.
        // See `microcode::routine`.

        // XCHG AX, reg16.
        0x90..=0x97 => 3,

        // `MOV acc, [addr]` and `MOV [addr], acc` at A0 through A3 have no rows:
        // they price from their microcode, which spends nothing at all. See
        // `microcode::routine`.

        // `XLAT` has no row: it runs the transcribed routine at 0x10c, whose
        // three clocks stand in front of its read rather than anywhere after
        // it. The row was 11 documented less the transfer plus one recorded on
        // top, and that extra clock was the three landing in the wrong place.

        // The 8087 escapes have no rows: they run the transcribed routine at
        // 0x108, which reads the operand so a coprocessor could see it and
        // spends nothing at all. See [`super::access::operand_access`], which
        // performs that read.

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

        // LES and LDS have no rows: they run the transcribed routines at 0x0f0
        // and 0x0f4, which read the segment half of the far pointer themselves.

        // `TEST acc, imm` at A8 and A9 has no row: it prices from its
        // microcode, which is a jump over the immediate's second queue read for
        // the byte form and nothing at all for the word form. See
        // `microcode::routine`.

        // `MOV reg, imm` at 0x01c and `MOV r/m, imm` at 0x014 have no rows.

        // The shifts and rotates. By one, 2 clocks in a register or 15 with two
        // transfers in memory. By CL the table quotes 8+4/bit and 20+4/bit, and
        // the per-bit part is added by the caller, which is the only place that
        // knows CL.
        // The shifts and rotates have no rows: they run the transcribed
        // routines at 0x088 and 0x08c, and the loop on CL is in the routine
        // rather than in [`shift_count_cycles`].

        // The unary group. TEST with an immediate is 11 clocks and one
        // transfer; NOT and NEG are 16 and two.
        //
        // Table 1-16 prints a dash in the Transfers column for TEST
        // memory,immediate, which cannot be right for an instruction that reads
        // memory, and every other TEST form with a memory operand shows one.
        // Read as one here, and flagged rather than silently corrected.
        0xF6 | 0xF7 => match reg {
            // `TEST r/m, imm` has no row: it runs the transcribed routine at
            // 0x098, which is the jump over the immediate's second queue read
            // and 0x09a.
            0 | 1 => 0,
            // `NOT` and `NEG` have no rows: they run the transcribed routines
            // at 0x04c and 0x050, which are `INC r/m`'s shape with different
            // line numbers. See `microcode::routine`.
            2 | 3 => 0,
            // **`MUL` and `DIV` have no rows.** They run the transcribed
            // routines at 0x150, 0x158, 0x160 and 0x168, whose `CORX` and `CORD`
            // clocks and whose sixteen and eight around them come off the
            // co-routines. `DIV`'s faulting operands run one too: `CORD` leaves
            // for `INT 0` at 0x18a and the instruction walks the same INTR list
            // `INT n` does, vector read and all.
            4 | 6 => 0,
            // `IMUL` and `IDIV` have none either, for the same reason one line
            // up: they are the same co-routines wrapped in `PREIMUL`,
            // `PREIDIV`, `NEGATE` and `POSTIDIV`, and those are branches that
            // can be counted rather than a table that has to be measured.
            //
            // **The row this replaces charged the memory form and the part
            // charges the register form.** `mc_150` and `mc_160` spend a bare
            // `self.cycle()` for a register operand; a memory operand spends the
            // address routine's `RET` instead. They come to the same number,
            // which is how a base calibrated on one form and a row on the other
            // could look right and still be a clock out.
            _ => {
                let _ = is_mem;
                0
            }
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
        // `INC r/m` and `DEC r/m`, the reg 0 and 1 forms of the two groups,
        // have no rows: they run the transcribed routine at 0x020. `FE`'s other
        // reg fields are invalid encodings that the group's decoder treats as
        // the `FF` forms, so they fall through to the arm below.
        0xFE => 0,
        0xFF => match reg {
            0 | 1 => 0,
            // `PUSH r/m16` has no row: it runs the transcribed routine at
            // 0x026. reg=7 is the same instruction, because the group's decoder
            // does not check the top bit of the reg field.
            6 | 7 => 0,
            // The indirect far call and far jump. The memory forms run
            // transcribed routines at 0x068 and 0x0dc, which read the segment
            // half of the far pointer themselves. The register forms are
            // invalid encodings the suite does not record, and keep the rows
            // the manual's 37+EA and 24+EA were measured into.
            3 => {
                if is_mem {
                    0
                } else {
                    16
                }
            }
            5 => {
                if is_mem {
                    0
                } else {
                    10
                }
            }
            _ => 0,
        },

        // The unconditional transfers that carry their target in the
        // instruction have no rows: `CALL rel16`, `JMP rel16`, `JMP rel8`,
        // `JMP far` and `CALL far` all run transcribed routines.
        //
        // Their old rows recorded a disagreement worth keeping in mind while
        // the transcriptions are checked. `9A` and `EA` are five bytes long,
        // one more than the queue holds, so their spans included the execution
        // unit waiting for a byte, and the row was measured against a read
        // schedule that differed from the part's. Both read `-1` on every
        // full-queue case and were exact on every empty-queue one, which is
        // what a population-blind constant looks like when the error is really
        // in the refill.
        //
        // `9A`'s **bus order** was wrong in a way no count could see. The rows
        // ran both pushes, then flushed, then reloaded; the part starts the
        // fetch at the target between its two pushes:
        //
        // ```text
        //   got  [... W:CS W:CS W:IP W:IP F:target]
        //   want [... W:CS W:CS F:target W:IP W:IP]
        // ```
        //
        // The routine places the flush where FARCALL puts it, between the two,
        // which is the whole reason a step list can express this and a row
        // cannot.

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
        // SALC, which Intel never documented and which therefore has no row to
        // transcribe: it sets AL to 0xFF when carry is set and to zero when it
        // is not. Measured, and the split is exactly that branch, uniform on
        // every case: 4 clocks with carry, 3 without. Writing the ones is a
        // clock dearer than writing the zeros, which is the same shape `CWD`
        // has one line above.
        0xD6 => {
            if taken {
                4
            } else {
                3
            }
        }
        // The conditional jumps and their aliases sixteen below have no rows:
        // they run the transcribed routine at 0x0e8, whose one clock is 0x0e9
        // and whose taken arm is RELJMP.
        0x60..=0x7F => 0,
        // LOOPNE and LOOPE. Documented 19 and 18 taken, 5 and 6 not.
        // The `LOOP` family has no rows: it runs the transcribed routines at
        // 0x134, 0x138 and 0x140.
        0xE0 | 0xE1 => 0,
        // `LOOP` has no row: it runs the transcribed routine at 0x140, whose
        // 0x140 and 0x141 are the loader's pause and whose taken arm is RELJMP.
        0xE2 => 0,
        0xE3 => 0,
        // `INTO` has no row either: it runs the transcribed routine at 0x1ac,
        // whose taken arm is the same INTR list `INT n` walks with four clocks
        // in front of it. The row had 25 taken and 4 not, and the part spends
        // two on the untaken arm.
        0xCE => 0,
        _ => 0,
    }
}

/// Whether `opcode`'s microcode branches on the state, so that
/// [`branch_cycles`] gives its cost instead of [`eu_cycles`].
pub(crate) fn branches_on_state(opcode: u8) -> bool {
    matches!(
        opcode,
        0x37 | 0x3F | 0x60..=0x7F | 0x99 | 0xCE | 0xD6 | 0xE0..=0xE3
    )
}

/// Whether `opcode` can redirect the instruction stream, and so may throw the
/// prefetch queue away rather than run on through it.
///
/// Asked by the loader's queue-length correction, which charges a clock for the
/// refill behind an instruction that drained the queue. An instruction that
/// flushes has no such refill: the reload at the target is already counted
/// separately, as the seven clocks under [`eu_cycles`]'s control transfers.
///
/// Conservative on purpose. The conditional forms are included whether or not
/// they take their branch, because this decides a table entry and not a
/// per-case cost, and a `Jcc` that falls through is two bytes and never reaches
/// the four-byte condition anyway.
pub(crate) fn may_flush_the_queue(opcode: u8) -> bool {
    matches!(
        opcode,
        0x60..=0x7F        // the conditional jumps
            | 0x9A          // CALL far direct
            | 0xC2 | 0xC3 | 0xC0 | 0xC1   // RET near and its aliases
            | 0xCA | 0xCB | 0xC8 | 0xC9   // RET far and its aliases
            | 0xCC..=0xCF   // INT, INT 3, INTO, IRET
            | 0xE0..=0xE3   // LOOP and JCXZ
            | 0xE8..=0xEB   // CALL near, JMP near, JMP far, JMP short
            | 0xFF          // the indirect calls and jumps live in this group
    )
}

/// Whether this instruction really will redirect the stream, as against
/// [`may_flush_the_queue`]'s conservative "could".
///
/// The difference matters because this decides when the prefetcher stops rather
/// than a table entry: `SUSP` is the first or second step of a transfer's
/// microcode, and suspending an instruction that turns out not to transfer costs
/// fetches the part does run. So the group opcodes are asked for their reg
/// field, and the conditional forms have to be told whether they branch.
///
/// `INC`, `DEC` and `PUSH` share `FF`'s encoding with the indirect calls and
/// jumps and go nowhere; `taken` is what the caller predicted for the
/// conditional forms, which is the same prediction the timing rows are charged
/// from.
pub(crate) fn will_transfer(opcode: u8, modrm: u8, taken: bool) -> bool {
    match opcode {
        // The indirect group: CALL, CALLF, JMP and JMPF are reg 2 through 5.
        0xFF | 0xFE => matches!((modrm >> 3) & 7, 2..=5),
        _ => may_flush_the_queue(opcode) && taken,
    }
}

// `PREFIX_PAUSE` stood here, one T-state the loader stopped for after reading a
// prefix byte. The manual gives a segment override, `LOCK` and `REP` two clocks
// apiece, and the reasoning was that one is the loader pulling the byte out of
// the queue and this was the other.
//
// **The second clock is real and this was charging it twice.** It is the T-state
// [`super::I8088::tick_eu`] spends taking the prefix out of the preload: a
// non-prefix byte falls straight through and reads the next one on that same
// T-state, and a prefix cannot, because the stage is still `Opcode` and the
// fall-through only admits a ModR/M. That return is the prefix's second clock,
// and it was already being spent before this was added on top.
//
// The whole prefixed half of the corpus was one clock out at the prefix for it,
// which no survey could see: the probe took the unprefixed population until
// `--prefixed` existed.

/// Where in an instruction the loader stops for a T-state, and for how long.
///
/// See [`loader_stall`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoaderStall {
    /// The loader runs flat out, a byte per T-state.
    None,
    /// It pauses after the opcode byte, before the immediate or displacement
    /// behind it. Given back by [`eu_cycles`]'s caller.
    AfterOpcode(u8),
    /// It pauses after the ModR/M byte of a **memory** form, before the
    /// displacement. Given back by [`super::access::address_phase_cycles`],
    /// where the displacement's own read time already lives.
    BeforeDisplacement(u8),
    // A `BeforeImmediate(u8)` stood here, one T-state after the ModR/M byte of a
    // register form. `TEST r/m, imm` was the only opcode that reached it and the
    // part does not pause there at all: `mc_098` reads its two operands one
    // after the other, and `test ax, 5594h` takes its four queue reads on four
    // consecutive T-states. The clock netted to nothing while `F6` and `F7` were
    // rows, because `eu_cycles`'s caller gave it straight back; once every reg
    // field of both became a routine, nothing subtracted it any more.
}

impl LoaderStall {
    /// The T-states spent, wherever they are given back.
    pub(crate) fn clocks(self) -> u8 {
        match self {
            LoaderStall::None => 0,
            LoaderStall::AfterOpcode(n) | LoaderStall::BeforeDisplacement(n) => n,
        }
    }

    /// The part the microcode has to give back, which is all of it except the
    /// pause before a displacement: that one is spent inside the effective
    /// address, and the address phase gives it back instead.
    ///
    /// Measured, both ways round. Charging the displacement pause to the
    /// microcode instead takes the count from 72.68% to 71.86% and the
    /// bus-cycle order from 68.41% to 60.73%: those clocks belong to the
    /// effective address, and taking them from anywhere else moves the operand
    /// access off the cycle the part starts it on.
    pub(crate) fn charged_to_microcode(self) -> u8 {
        match self {
            LoaderStall::BeforeDisplacement(_) => 0,
            other => other.clocks(),
        }
    }
}

/// **The part's loader does not take a byte every T-state.** Where it stops,
/// and for how long, depends on the opcode.
///
/// Measured, not transcribed: no table quotes this, because Table 1-16 gives
/// totals and this is about their distribution. `loader_read_pattern` reports
/// the gaps between successive reads of one instruction over the whole corpus,
/// restricted to a full queue and no prefix so that every byte is already in
/// hand and nothing in the pattern can be waiting on a fetch. **94 of 323 files
/// stall and every one of them is 100.00% uniform.**
///
/// ```text
///   gap after the opcode, when no ModR/M byte follows it      2 T-states
///   the same for E0-E3, the loop forms                        4 T-states
///   gap after the ModR/M byte of a memory form                4 T-states
///   gap after the ModR/M byte of F6 /0, F6 /1, F7 /0, F7 /1   2 T-states
/// ```
///
/// The third is the widest by far: **every ModR/M opcode reads its displacement
/// four T-states after the ModR/M byte**, and this core read it in one.
/// `loader_read_gap_diff` puts our rhythm beside the part's and 148 files differ
/// on exactly that gap, from `8B` and `89` through the whole ALU block, `C4`-`C7`,
/// the shifts, the escapes and the `FF` group. The clocks are the effective
/// address's: Table 1-16 folds the displacement's fetch into `+EA`, which is
/// why [`super::access::address_phase_cycles`] already takes the displacement's
/// own read time out, and this comes out of the same place.
///
/// The last is a decode the part cannot avoid: in that group alone the `reg`
/// field decides whether an immediate follows at all, which is what
/// [`super::format::Imm::ByteIfTest`] exists to express, so the loader cannot
/// know what to fetch until it has read and decoded the ModR/M byte. It shows
/// only on the register forms, because a memory form defers its immediate past
/// the operand access and takes the displacement pause above instead.
///
/// **None of this is a cost on top of the instruction.** Where the queue is
/// full and the EU is the critical path an instruction takes its documented
/// clocks however its reads are distributed, so every one of these T-states is
/// given back: see [`LoaderStall::charged_to_microcode`] for which giver.
/// What the pause changes is *when the queue drains*, which sets when the
/// refill behind it lands, and so the whole fetch schedule.
/// The pause before a **deferred** immediate: the one belonging to a memory
/// form, which the loader goes back for after the operand access rather than
/// fetching with the rest of the instruction.
///
/// **One T-state, and it is the effective-address routine's return.** The
/// reference probe shows the whole seam on `add byte [ds:bx+si-64h], FAh`: the
/// operand read's T4 is spent by `1E2: OPR -> tmpb`, `RET` spends the clock
/// after it, and the immediate is read on the next one at
/// `00C: Q -> tmpbL`, which is also the instruction's first execute line.
///
/// It was two while the loader took its bytes a T-state late, which put the
/// deferred immediate two clocks behind the return instead of one. See
/// [`I8088::preload`].
///
/// Given back by [`eu_cycles`]'s caller, like the other pauses that are not the
/// effective address's.
///
/// **How long it waits is the two ways out of the address routine, the same
/// asymmetry the routines carry.** A form that reads its operand leaves through
/// `1E2: OPR -> tmpb`, which is the line that spends the read's T4, so only
/// `RET` stands between it and the immediate. A form that only writes leaves
/// through `1E3: tmpa -> IND`, which has a clock of its own, and `RET` behind
/// that, so it waits two.
///
/// `C6` and `C7` are the whole of the second case, and their trace shows both
/// clocks: `mov word [ds:di-6E21h], B683h` runs `1E3` on cycle 8 and `RET` on 9
/// and reads its immediate at `014: Q -> tmpbL` on 10. This core read it on 8.
pub(crate) fn deferred_immediate_stall(opcode: u8, modrm: u8) -> u8 {
    use super::format;

    // The loader defers an immediate exactly when the operand is in memory, so
    // that the address can be computed and the operand read first. See
    // `I8088::begin_immediate`.
    let f = format::format_of(opcode);
    if !(f.modrm && modrm >> 6 != 3 && f.imm.len(Some(modrm)) > 0) {
        return 0;
    }
    if super::access::operand_access(opcode, modrm).reads {
        // `1E2` spent the read's T4, so only the return is left.
        1
    } else {
        // `1E3` and the return.
        2
    }
}

pub(crate) fn loader_stall(opcode: u8, modrm: u8) -> LoaderStall {
    use super::format::{self, Imm};

    let f = format::format_of(opcode);
    if !f.modrm {
        // Nothing behind the opcode is nothing to wait for.
        if f.imm.len(None) == 0 {
            return LoaderStall::None;
        }
        // **Nothing between the opcode and the byte behind it.** The clock that
        // used to sit here was the loader's lead-in, spent because this core
        // took the opcode a T-state after the part did; with the boundary fetch
        // taking it on the part's clock the gap is gone. `add al, 2Dh` reads its
        // opcode on cycle 0 and its immediate on cycle 1, with nothing between.
        // See [`I8088::preload`].
        return match opcode {
            // The `LOOP` family decrements CX and spends two clocks before it
            // reads its displacement: 0x138 and 0x139 for `LOOPNE`/`LOOPE`,
            // 0x140 and 0x141 for `LOOP`. It was three while the loader took its
            // bytes a T-state late.
            0xE0..=0xE3 => LoaderStall::AfterOpcode(2),
            _ => LoaderStall::None,
        };
    }
    if format::displacement_len(modrm) > 0 {
        // The gap is the address arithmetic on the *register* components, done
        // before the displacement is read. Measured by
        // `modrm_to_displacement_gap`, pooled over every opcode that carries a
        // ModR/M byte, uniform within each mode on 98% of cases, and `mod=01`
        // and `mod=10` agree exactly:
        //
        //   two registers            6, or 7 for BX+DI and BP+SI
        //   one register             4
        //   none, the direct form    2
        //
        // Those are the effective-address table's own pairings, including its
        // asymmetry: BX+DI and BP+SI cost a clock more there too. The loader
        // already spends one T-state taking the byte, so the pause is one less.
        let gap: u8 = match (modrm >> 6, modrm & 7) {
            (0, 6) => 2,
            (_, 0 | 3) => 6,
            (_, 1 | 2) => 7,
            _ => 4,
        };
        return LoaderStall::BeforeDisplacement(gap - 1);
    }
    // **`TEST r/m, imm`'s register form paused here and the part does not.**
    // `mc_098` reads its ModR/M operand and its immediate one after the other
    // with no `cycle_i` between them, and `test ax, 5594h` bears that out: the
    // part's four queue reads are on four consecutive T-states.
    //
    // The pause was a `BeforeImmediate(1)` that `eu_cycles`'s caller gave back
    // out of the row, so it netted to nothing while `F6` and `F7` were priced
    // by rows. Every reg field of both is a transcribed routine now, and a
    // routine never reaches that subtraction, so what had been a wash became a
    // clock. `Imm::ByteIfTest` and `Imm::WordIfTest` belong to those two opcodes
    // and nothing else, which is why the variant goes with it.
    let _ = Imm::ByteIfTest;
    LoaderStall::None
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

// `multiply_cycles` stood here, `69 + bits + high_zero` for the byte form and
// `117 + bits + high_zero` for the word. Both bases were fitted, both landed on
// Intel's published endpoints, and both were exactly three clocks above what
// `CORX` and the lines around it actually spend. `MUL` runs the transcribed
// routine now and prices from [`multiply_routine_cycles`], which counts them.
//
// The residual that base carried is gone with it. It was recorded here as "the
// byte form is exact for five or more set bits and one clock low for some cases
// below that, and the cause is not the bit count", and the cause was the base.

/// What a signed multiply or divide pays for making its operands positive,
/// beyond what the unsigned form of the same instruction pays.
///
/// **This is the measurement, kept as a cross-check rather than as the model.**
/// `PREIMUL`, `PREIDIV`, `NEGATE` and `POSTIDIV` are counted off the reference
/// now, in [`signed_multiply_routine_cycles`] and
/// [`signed_divide_routine_cycles`], and the counts reproduce every number in
/// the table below. That agreement is worth a test rather than a coincidence, so
/// this and the two rules built on it stay, compiled only for one.
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
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
fn signed_multiply_cycles(
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
#[cfg(test)]
fn signed_divide_cycles(word: bool, dividend: i64, divisor: i64) -> u16 {
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

// `divide_cycles` stood here, `80 + compared + 2 * last` for the byte form and
// `144 + ...` for the word, with 48 for the fault. All three were fitted, and
// the two bases were exactly three clocks above what `CORD` and the lines around
// it spend. `DIV` runs the transcribed routine now and prices from
// [`divide_routine_cycles`], which counts them; the fault runs INTR.
//
// The fault's 48 was the clearest sign that a lump was the wrong shape. It was
// the recorded 79 less 31 the pipeline spent on the pushes and the reload, and
// the note beside it said the sixteen clocks of the vector read were still
// inside the number because "the pipeline cannot know to read the vector ahead
// of the instruction". It can: the fault is decided by a compare `CORD` makes
// before its loop, and the pipeline can make the same one.

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

// `AAM` had a row here, `77 + compared + 2 * (the quotient is odd)`, and a
// second number, 46, for the zero immediate that faults. Both were fitted. It
// runs the transcribed routine at 0x174 now, whose `CORD` clocks come from
// [`cord_cycles`], and its fault walks the same INTR list `INT n` does rather
// than being priced as a lump with the vector read hidden inside it.
//
// The two-clock term survived the move, which is the point worth keeping: it was
// found on this opcode as "the quotient is odd", and reading `CORD` off the
// reference shows why. The last pass costs one more than a middle pass when it
// subtracts and one less when it does not, and whether it subtracted is exactly
// what the quotient's low bit records.

// `AAD` had a row here, `59 + imm.count_ones()`, fitted to the recording. It
// runs the transcribed routine at 0x170 now, whose `CORX` clocks come from
// `corx_cycles` and so are read off the co-routine rather than measured against
// it. The fitted base was one clock over, and the whole of that clock was the
// loader's before the boundary fetch existed.

/// Clocks the `CORX` multiply co-routine spends, for an operand `width` bits
/// wide whose multiplier has `bits` of them set.
///
/// Read off the routine rather than fitted to it. `0x17f` and `0x180` open it;
/// then one pass a bit, each spending `0x181`, either `0x182` and `0x183` when
/// the shifted-out carry is set or a jump when it is not, and `0x184` through
/// `0x186`; each pass but the last spends a jump to get back to the top; and
/// `0x187` and the return close it.
///
/// So a pass is five clocks and a sixth when the bit is set, the jumps back add
/// one short of the width, and the ends add four:
/// `6 * width + bits + 3`.
pub(crate) fn corx_cycles(width: u16, bits: u32) -> u16 {
    6 * width + bits as u16 + 3
}

/// Clocks the `CORD` divide co-routine spends, or `None` when the operands
/// fault before the loop.
///
/// Read off the routine the way [`corx_cycles`] is. `CORD` opens with 0x188,
/// 0x189 and 0x18a, and 0x18a is `NCY INT0`: the compare in front of the loop
/// leaves for `INT 0` through a jump when the high half of the dividend is
/// already at least the divisor, which is the divide error and is why this
/// returns `None` rather than a count.
///
/// Then one pass a bit, each spending 0x18b through 0x18e and one of three arms:
///
/// - the bit shifted out of the top says the value must exceed the divisor, so
///   the subtract is immediate, at a jump, 0x195 and 0x196;
/// - a compare at 0x18f and 0x190 that goes on to subtract, at a jump and
///   0x196;
/// - the same compare that does not, at 0x191.
///
/// Every pass but the last then spends a jump to get back to the top, and the
/// last spends 0x197 and a jump instead when it subtracted and nothing at all
/// when it did not. So a pass is eight clocks and a ninth when it compared
/// before subtracting, and the last pass is one more or one less depending on
/// whether it subtracted at all. 0x192, 0x193, 0x194 and the return close it:
///
/// ```text
/// 8 * width + compared + 2 * (the last pass subtracted) + 6
/// ```
///
/// **The two-clock term on the last pass is that arm structure, not a fudge.**
/// It had been fitted here, keyed on the quotient's low bit after `COM1`, which
/// is the same condition read off the answer instead of off the routine.
pub(crate) fn cord_cycles(dividend: u32, divisor: u32, width: u32) -> Option<u16> {
    // 0x188 and 0x189 compare, and 0x18a jumps to INT0 when it did not borrow.
    if divisor == 0 || (dividend >> width) >= divisor {
        return None;
    }
    let (compared, last_bit) = cord(dividend, divisor, width);
    // 0x188, 0x189, 0x18a in front, and 0x192, 0x193, 0x194 and the return
    // behind, with the loop's `8 * width + compared - 1 + 2 * last` between.
    Some(8 * width as u16 + compared + 2 * u16::from(last_bit) + 6)
}

/// Clocks a `DIV` spends, and whether it divided.
///
/// `div8` and `div16` are the same three parts around `CORD`: 0x160, 0x161 and
/// 0x162 (0x168 through 0x16a for the word form), then 0x163 and the jump into
/// the co-routine, and 0x164 and 0x165 behind it. Seven clocks either width.
///
/// **Both forms then cost one more, for different reasons, which is why the row
/// this replaces looked like it charged the memory form.** A register operand
/// spends the bare `self.cycle()` `mc_160` runs for it, which the comment beside
/// it calls an extra cycle "for some reason". A memory operand spends `RET`
/// instead: `DIV` reads its operand, so the address routine leaves through
/// `1E2: OPR -> tmpb` and the return stands in front of 0x160. Eight clocks
/// either way, and the old base was calibrated on the register form with a row
/// making up the difference on the other.
///
/// The unsigned form has one fault rather than `IDIV`'s two: `CORD`'s check
/// before the loop is the unsigned one, so nothing that passes it can overflow
/// the destination afterwards.
pub(crate) fn divide_routine_cycles(word: bool, dividend: u32, divisor: u32) -> Loop {
    let width = if word { 16 } else { 8 };
    match cord_cycles(dividend, divisor, width) {
        // 0x160, 0x161, 0x162, then 0x163 and the jump, then 0x164 and 0x165,
        // and one for whichever front this form has.
        Some(cord) => Loop::Completed(cord + 8),
        // The front, the same three, 0x163 and the jump, `CORD`'s four, and
        // `int0`'s two.
        None => Loop::Faulted(1 + 3 + 2 + CORD_FAULT + INT0_ENTRY),
    }
}

/// Clocks a `MUL` spends.
///
/// `mul8` and `mul16` are the same shape around `CORX`: 0x150 and 0x151 in
/// front, 0x152 and the jump into the co-routine, then 0x153, 0x154, and 0x155,
/// 0x156, a jump, 0x1d2, 0x1d3 and another jump into `MULCOF`. Fifteen clocks
/// either width, counting `MULCOF`'s shorter arm.
///
/// `MULCOF` then sets carry and overflow from the product's high half, and the
/// arm that *clears* them is the longer one: 0x1d0, a jump, 0x1cc and a jump
/// against 0x1d0, 0x1d1 and a jump. That is the extra clock a product with a
/// zero high half costs, and it comes out of the routine rather than out of the
/// recording, which is where it was found.
///
/// The bit count is the accumulator's, because `CORX` rotates `tmpc` and `tmpc`
/// is what `0x150: A -> tmpc` loaded. The byte form counts AL alone.
///
/// The sixteenth clock is the front, and both forms have one: a register operand
/// spends the bare `self.cycle()` `mc_150` runs for it, and a memory operand
/// spends `RET` instead, `MUL` having read its operand and so left the address
/// routine through `1E2`. See [`divide_routine_cycles`], which is the same
/// story.
pub(crate) fn multiply_routine_cycles(word: bool, multiplier: u16, product_high_zero: bool) -> u16 {
    let (width, bits) = if word {
        (16, multiplier.count_ones())
    } else {
        (8, (multiplier as u8).count_ones())
    };
    corx_cycles(width, bits) + 16 + u16::from(product_high_zero)
}

/// Clocks `AAM` spends, and whether it divided.
///
/// `mc_174` spends 0x175, 0x176 and the jump into `CORD`, and 0x177 behind it.
/// The same loop `DIV` runs, with AL over the immediate. A zero immediate faults
/// at 0x18a, and `mc_174`'s `Err` arm goes straight to `int0`.
pub(crate) fn aam_routine_cycles(al: u8, imm: u8) -> Loop {
    match cord_cycles(u32::from(al), u32::from(imm), 8) {
        // 0x175, 0x176 and the jump in front, 0x177 behind.
        Some(cord) => Loop::Completed(cord + 4),
        // The same three in front, `CORD`'s four, and `int0`'s two.
        None => Loop::Faulted(3 + CORD_FAULT + INT0_ENTRY),
    }
}

/// Clocks `IMUL` spends.
///
/// `MUL` with `PREIMUL` in front of the loop, `NEGATE` behind it when exactly
/// one operand was negative, and `IMULCOF` in place of `MULCOF` at the end.
/// Every one of those is a branch, and counting the arms reproduces the four
/// sign combinations that had been measured into a table.
///
/// - **`PREIMUL`** spends the jump, 0x1c0 and 0x1c1, then either 0x1c2, 0x1c3
///   and a jump when the accumulator is negative or a jump alone when it is not.
///   It falls into `NEGATE` at line 7, which spends 0x1bb, 0x1bc and 0x1bd and
///   then either a jump, 0x1bf and the return for a positive multiplicand or
///   0x1be and the return for a negative one. So `10 + 2a - b`, for `a` and `b`
///   the two signs.
/// - **`NEGATE` behind the loop** costs twelve when it runs: a jump to reach it,
///   then its full form, 0x1b6 through 0x1ba being five whichever arm it takes
///   and the tail three more, the multiplicand being positive by then because
///   `PREIMUL` made it so. It runs when exactly one operand was negative, each
///   having flipped `F1` on its way past.
/// - **`IMULCOF`** spends the jump, 0x1cd, 0x1ce and 0x1cf, then `MULCOF`'s own
///   branch, then 0x15d and a jump.
pub(crate) fn signed_multiply_routine_cycles(
    word: bool,
    multiplicand: i32,
    multiplier: i32,
    product_sign_extends: bool,
) -> u16 {
    let (width, bits) = if word {
        (16, (multiplier.unsigned_abs() as u16).count_ones())
    } else {
        (8, (multiplier.unsigned_abs() as u8).count_ones())
    };
    let accumulator_negative = u16::from(multiplier < 0);
    let multiplicand_negative = u16::from(multiplicand < 0);
    // `PREIMUL` and the `NEGATE` it falls into.
    let pre = 10 + 2 * accumulator_negative - multiplicand_negative;
    // `NEGATE` behind the loop, when exactly one operand was negative.
    let post = 12 * u16::from(accumulator_negative != multiplicand_negative);
    // The front, 0x150 and 0x151, 0x152 and the jump, 0x153, 0x154, then
    // `IMULCOF`'s jump, 0x1cd through 0x1cf, its shorter arm, and 0x15d and a
    // jump.
    corx_cycles(width, bits) + 16 + pre + post + u16::from(product_sign_extends)
}

/// Clocks `IDIV` spends, and whether it divided.
///
/// `DIV` with `PREIDIV` in front of the loop and `POSTIDIV` behind it, and it
/// can leave for `INT 0` from either end.
///
/// - **`PREIDIV`** spends 0x1b4 and 0x1b5, then a jump into `NEGATE` at line 7
///   for a positive dividend or a fall through into its whole form for a
///   negative one. `NEGATE`'s tail then costs three for a positive divisor and
///   two for a negative. So `9 + 4d - v`, for `d` and `v` the two signs, and
///   those four offsets are exactly the table this replaces.
/// - **`POSTIDIV`** is ten clocks flat when it returns: 0x1c4, then 0x1c5
///   through 0x1c7, then one clock whichever way the divisor went, 0x1c9 and
///   0x1ca, one more whichever way `F1` went, and 0x1cc and the return. Its
///   branches all cost the same on both arms, which is why only `PREIDIV`'s
///   signs show up in the total.
/// - **The late fault** is `POSTIDIV`'s own: 0x1c4 tests the carry `CORD` left
///   and jumps to `INT 0` when the quotient did not fit the signed half the
///   destination holds. `CORD`'s check before the loop is the unsigned one, so a
///   quotient between the two limits runs the whole loop and only then faults.
pub(crate) fn signed_divide_routine_cycles(word: bool, dividend: i64, divisor: i64) -> Loop {
    let width: u32 = if word { 16 } else { 8 };
    let magnitude = dividend.unsigned_abs();
    let divisor_magnitude = divisor.unsigned_abs();
    // `PREIDIV`, and the `NEGATE` it enters at line 7 or falls into.
    let pre = 9 + 4 * u16::from(dividend < 0) - u16::from(divisor < 0);
    // The front, 0x160 through 0x162, the jump into `PREIDIV`, 0x163 and the
    // jump into `CORD`.
    let front = 1 + 3 + 1 + pre + 2;

    let Some(cord) = cord_cycles(magnitude as u32, divisor_magnitude as u32, width) else {
        // `CORD` left at 0x18a before its loop, and `mc_160`'s `Err` arm goes
        // straight to `int0`.
        return Loop::Faulted(front + CORD_FAULT + INT0_ENTRY);
    };
    // 0x164 and 0x165, then the jump into `POSTIDIV`.
    let behind = front + cord + 2 + 1;
    if magnitude / divisor_magnitude > (1 << (width - 1)) - 1 {
        // 0x1c4 tests `CORD`'s carry and jumps out, then `int0`.
        return Loop::Faulted(behind + POSTIDIV_FAULT + INT0_ENTRY);
    }
    Loop::Completed(behind + 10)
}

/// What `CORD` spends before leaving for `INT 0`: 0x188 and 0x189 compare and
/// 0x18a jumps out, the loop never running.
const CORD_FAULT: u16 = 4;

/// What `POSTIDIV` spends before leaving for `INT 0`: 0x1c4 and its jump.
const POSTIDIV_FAULT: u16 = 2;

/// What `int0` spends reaching INTR: 0x1a7 and a jump. It enters one line down,
/// so [`microcode::interrupt`] is asked not to spend 0x19d.
const INT0_ENTRY: u16 = 2;

/// What a multiply or divide's co-routine costs, and whether the instruction
/// produced an answer or left for `INT 0`.
///
/// The two are different routines from the sequencer's point of view rather than
/// two numbers: a fault does not retire, it walks the whole interrupt list, four
/// vector-read bus cycles and three pushes and a flush. Handing the caller one
/// integer and a flag is what lets [`microcode::routine`] pick between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loop {
    /// The clocks it spends, having produced an answer.
    Completed(u16),
    /// The clocks it spends before `INT 0`, `int0`'s own two included.
    Faulted(u16),
}

/// Where a string operation's microcode clocks fall around its bus cycles:
/// before the first access, between the two, and after the last.
///
/// **A total cannot express a string operation.** Table 1-16 gives one number an
/// iteration, and the recording shows the clocks distributed around the
/// accesses: `CMPS` spends two before its source read, two more before its
/// destination read, and three after. Charged as a lump at the end, the reads
/// come out too early and the prefetching around them goes with them.
///
/// Read off `string_op` and the line that dispatches to it. Every operation
/// spends one clock on its own entry line first, `0x11c` for `STOS`, `0x120` for
/// `CMPS` and `SCAS`, `0x12c` for `MOVS` and `LODS`, and then:
///
/// - `STOS` writes with nothing on either side.
/// - `LODS` reads with nothing on either side.
/// - `MOVS` reads, spends `0x12e`, and writes.
/// - `SCAS` spends `0x121` and a jump, reads, and spends `0x126` through
///   `0x128`.
/// - `CMPS` spends `0x121`, reads the source, spends `0x123` and `0x124`, reads
///   the destination, and spends `0x126` through `0x128`.
pub(crate) fn string_clocks(opcode: u8) -> StringClocks {
    let (before, between, after) = match opcode {
        // MOVS: the entry line, then `0x12e` between the read and the write,
        // which falls on the read's own release clock and so counts for
        // nothing here.
        0xA4 | 0xA5 => (1, 0, 2),
        // CMPS: the entry line and `0x121`, then `0x123` and `0x124`, then
        // `0x126`, `0x127` and `0x128`.
        //
        // `0x123` falls on the source read's own T4, the release clock the
        // microcode behind a transfer always spends, so only `0x124` is left to
        // count between the two reads. `0x126` does the same for the
        // destination read, and the tail is what stands past it.
        0xA6 | 0xA7 => (2, 1, 3),
        // STOS and LODS: the entry line, and the same tail. All three of this
        // group run the same routine behind `string_op` and so count the same,
        // which is the check on the rule rather than three numbers that happen
        // to work.
        0xAA..=0xAD => (1, 0, 2),
        // SCAS: the entry line, `0x121` and the jump, then `0x126` through
        // `0x128`. It reads once, so nothing falls between.
        0xAE | 0xAF => (3, 0, 3),
        _ => (0, 0, 0),
    };
    StringClocks {
        before,
        between,
        after,
    }
}

/// The three positions [`string_clocks`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StringClocks {
    /// Before the iteration's first bus cycle.
    pub before: u8,
    /// Between the two, for the operations that have two.
    pub between: u8,
    /// After the last, before the repeat decision.
    pub after: u8,
}

/// Clocks a repeated iteration spends on the loop control, which a single one
/// does not.
///
/// `mc_11c` runs 0x11d and 0x11e whichever way and only reaches 0x11f and 0x1f0
/// when `in_rep` is set: the interrupt check and the decrement of CX. The jump
/// behind them is there either way, to 1 to go round again or to 1f1 to stop, so
/// it is not part of the difference.
///
/// See [`string_clocks`] for the other half of the repeat's accounting, which is
/// the entry line those two are paid for with.
///
/// **The last iteration spends one of the two, not both.** `after` is counted on
/// the single form, where it is the operation's closing microcode line plus the
/// RNI clock that retires the instruction: `AA` unrepeated spends `JMP:` and
/// then the `FETCH_END` clock. A repeat's last iteration closes on `0x11f` and
/// `0x1f0` and retires on the clock after, so it spends one clock more than the
/// single form and not two. A continuing iteration does spend both, because the
/// clock the last one gives to the RNI it gives to re-entering the loop.
///
/// `cs rep stosb` is the check at both ends: 45 writes, every one of them on the
/// reference's clock, and the instruction retiring on its clock too.
///
/// **An iteration that ends on a read closes in six clocks, whatever it is.**
/// Counted off three whole reference iterations, stepped one at a time. Each
/// spends five microcode lines and then the RNI, and `SCAS` and `CMPS` spend
/// literally the same five, sharing the tail of the 0x120 routine:
///
/// - `rep lodsb` is thirteen clocks and closes on a jump, `1F8: OPR -> M`, a
///   second jump, `0x131` and `0x132`.
/// - `cs repne scasb` is fifteen and closes on `0x127` through `0x12b`.
/// - `ds repne cmpsb` is twenty-two and closes on `0x127` through `0x12b`.
///
/// So the repeat's term is whatever brings `after` up to that six, which is four
/// for `LODS` and three for the other two. `MOVS` and `STOS` end on a write,
/// which releases its wait a clock earlier than a read does, so they are counted
/// in the other frame: `rep stosb` closes on `0x11f`, `0x1f0` and the RNI, and
/// the plain two is already right for both.
///
/// **And a repeat that stops on its flag stops a clock sooner than one that runs
/// its count out.** `CMPS` and `SCAS` leave through two different lines and the
/// reference shows both: after `129: SIGMA-> BC` a count exit spends
/// `12A: SIGMA-> tmpc` and `12B` before the RNI, and a flag exit spends a single
/// jump. `AE`'s `repe` cases are all flag exits and its `repne` cases all run
/// out of count, so the two look like "one iteration against many" in the
/// corpus and are not. The case that separates them is a `cs repne cmpsb` that
/// finds its match with 31 of 78 still in CX: 47 iterations and a flag exit, and
/// it is a clock shorter than the count exits beside it.
pub(crate) fn string_repeat_cycles(
    opcode: u8,
    rep: Option<super::RepPrefix>,
    again: bool,
    stopped_on_flag: bool,
) -> u8 {
    if rep.is_none() {
        return 0;
    }
    // `CMPS`, `LODS` and `SCAS` take their last byte off the bus with a read;
    // `MOVS` and `STOS` finish with a write.
    let tail = if matches!(opcode, 0xA6 | 0xA7 | 0xAC..=0xAF) {
        READ_ITERATION_TAIL.saturating_sub(string_clocks(opcode).after)
    } else {
        2
    };
    if again {
        return tail;
    }
    tail.saturating_sub(1 + u8::from(stopped_on_flag))
}

/// What a repeated string iteration spends behind its last read, RNI included.
///
/// See [`string_repeat_cycles`], where the three iterations this was counted off
/// are written out.
const READ_ITERATION_TAIL: u8 = 6;

/// The clocks this iteration spends in front of its first bus cycle.
///
/// **`rep_start` runs the entry line once and not once per iteration.**
/// `rep_init` gates it: the first entry spends 0x11c (or 0x120, or 0x12c) and
/// then RPTS, and every iteration after it returns having spent nothing there.
/// So a continuing iteration of a repeat drops the entry line and adds
/// [`string_repeat_cycles`], for one clock more than a single iteration and not
/// two.
///
/// **It drops that one line and not the whole of `before`.** For the three
/// operations whose `before` is the entry line alone the two are the same thing,
/// which is why dropping all of it was right for `MOVS`, `STOS` and `LODS` and
/// wrong for the two that count more. Whole iterations off the reference:
///
/// - `rep stosb` and `rep lodsb` open on the bus request itself, spending
///   nothing in front of it, and their `before` is 1.
/// - `ds repne cmpsb` opens on `121: M -> tmpa` and requests the bus on the
///   clock after, spending one, and its `before` is 2.
/// - `cs repne scasb` opens on `121` and a jump and requests on the third clock,
///   spending two, and its `before` is 3.
///
/// In each case what a continuing iteration spends is `before` less one. `CMPS`
/// and `SCAS` were short by exactly that: a `cs repne scasb` iteration is
/// fifteen clocks and this core ran twelve.
///
/// `cs rep stosb` is the check at the other end: the part writes every ten
/// cycles and this core wrote every nine before the entry line and the loop
/// control were put where the reference has them.
pub(crate) fn string_before_cycles(opcode: u8, first_iteration: bool) -> u8 {
    let before = string_clocks(opcode).before;
    if first_iteration {
        before
    } else {
        before.saturating_sub(1)
    }
}

/// Clocks a `REP` prefix spends before its first iteration.
///
/// The `9 +` in Table 1-16's `9 + 17/rep`, less the two bytes the loader pulls
/// for the prefix and the opcode behind it. A repeated operation whose count is
/// already zero spends this and nothing else, which is the one way a string
/// operation runs no bus cycle at all.
///
/// **`RPTS` leaves three lines earlier when the count is already zero**, and
/// that is the whole of the difference. The reference runs `120`, a jump, then
/// `112: BC -> tmpc`, `113: SIGMA-> no dest` and `114`, and there it either
/// falls out or carries on. A `repne cmpsb` with CX at zero retires on the clock
/// after `114`, five clocks in all; one with a count to run spends a jump, `116`
/// and `RET` behind it and only then reaches `121`, which is where the operation
/// proper starts and where [`string_clocks`]'s `before` picks it up.
///
/// So the zero-count entry is three shorter than the other. This core charged
/// the full one either way and ran a zero-count repeat three clocks long, with a
/// code fetch to match: `repne cmpsb` from an empty queue took fourteen cycles
/// against the part's eleven and put a third fetch on the bus.
pub(crate) fn string_entry_cycles(repeated: bool, count_is_zero: bool) -> u8 {
    match (repeated, count_is_zero) {
        (false, _) => 0,
        (true, false) => 7,
        (true, true) => 7 - RPTS_EARLY_EXIT,
    }
}

/// The jump, `116` and `RET` that a repeat with a count to run spends after
/// `114` and one with an empty count does not.
///
/// See [`string_entry_cycles`].
const RPTS_EARLY_EXIT: u8 = 3;

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
        // SALC, undocumented, measured off the recording rather than a table.
        0xD6 => true,
        0xD7 => true,
        // The coprocessor escapes, whose operand read this core now performs.
        0xD8..=0xDF => true,
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
        for (byte_op, word_op, carries_immediate) in [
            (0x00u8, 0x01u8, false), // ADD r/m, reg
            (0x02, 0x03, false),     // ADD reg, r/m
            (0x38, 0x39, false),     // CMP r/m, reg
            (0x3A, 0x3B, false),     // CMP reg, r/m
            (0x84, 0x85, false),     // TEST
            (0x86, 0x87, false),     // XCHG
            (0x88, 0x89, false),     // MOV r/m, reg
            (0x8A, 0x8B, false),     // MOV reg, r/m
            (0xC6, 0xC7, true),      // MOV r/m, imm
            (0xD0, 0xD1, false),     // shift by 1
            (0xD2, 0xD3, false),     // shift by CL
            // Both ModR/M bytes here carry a reg field of zero, which in the
            // unary group selects `TEST r/m, imm`, so this pair does carry an
            // immediate and does take the jump over its second queue read.
            (0xF6, 0xF7, true),  // the unary group's TEST
            (0xFE, 0xFF, false), // INC/DEC
        ] {
            for modrm in [mem, reg] {
                let what = if modrm == mem { "memory" } else { "register" };
                assert_eq!(
                    eu_cycles(byte_op, modrm),
                    eu_cycles(word_op, modrm),
                    "{byte_op:#04X} and {word_op:#04X} disagree on a {what} operand"
                );
                // The same property, asked of the routine for a pair that has
                // been transcribed. Without this the assertion above goes
                // quietly vacuous as each row is deleted, which is exactly when
                // it stops protecting anything.
                //
                // **The one lawful difference is the immediate.** A byte-sized
                // form with an immediate spends a jump over the second queue
                // read that its word-sized twin does not, so the two routines
                // differ by exactly one clock and nothing else. That is a
                // property of the microcode rather than of the operand's width,
                // which is why the rule above still holds for everything else.
                let by = super::super::microcode::routine(byte_op, modrm, false, 0, None);
                let wo = super::super::microcode::routine(word_op, modrm, false, 0, None);
                if carries_immediate {
                    let (Some(by), Some(wo)) = (by, wo) else {
                        continue;
                    };
                    assert_eq!(
                        by.clocks(),
                        wo.clocks() + 1,
                        "{byte_op:#04X} and {word_op:#04X} differ by other than \
                         the immediate's jump on a {what} operand"
                    );
                } else {
                    assert_eq!(
                        by, wo,
                        "{byte_op:#04X} and {word_op:#04X} run different \
                         routines on a {what} operand"
                    );
                }
            }
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
        // The ModR/M forms are the microcode module's now, and there the
        // property is structural rather than arithmetic: `CMP` has no write
        // back, so it has neither the write step nor the two clocks the part
        // spends in front of one.
        let add =
            super::super::microcode::routine(0x00, mem, false, 0, None).expect("ADD r/m8, reg8");
        let cmp =
            super::super::microcode::routine(0x38, mem, false, 0, None).expect("CMP r/m8, reg8");
        assert_ne!(add, cmp, "CMP must not run the writing routine");
        // And in the immediate group, where the reg field picks the operation.
        use super::super::microcode::Step;
        let add_imm = super::super::microcode::routine(0x80, 0b00_000_100, false, 0, None)
            .expect("ADD r/m8, imm8");
        let cmp_imm = super::super::microcode::routine(0x80, 0b00_111_100, false, 0, None)
            .expect("CMP r/m8, imm8");
        assert!(add_imm.contains(Step::WriteOperand), "ADD writes back");
        assert!(
            !cmp_imm.contains(Step::WriteOperand),
            "CMP must stay cheaper than the operations that write back"
        );
    }

    /// **The two operand forms of a multiply or divide cost the same, and the
    /// row this replaces said otherwise.**
    ///
    /// That row charged the memory form one clock beyond the register form, its
    /// bus cycles and its effective address, and it was measured against bases
    /// calibrated on the register form. The part charges the *register* form:
    /// `mc_150` and `mc_160` spend a bare `self.cycle()` for a register operand,
    /// which the reference's own comment calls an extra cycle "for some reason",
    /// and a memory operand spends the address routine's `RET` instead, having
    /// read its operand and so left through `1E2`. One clock either way, and a
    /// base plus a one-sided row came to the same total on one form and a clock
    /// out on the other.
    ///
    /// So none of the eight has a row at all now, and the clock is inside the
    /// counted routine where the reference puts it.
    #[test]
    fn the_multiplies_and_divides_cost_both_operand_forms_alike() {
        for op in [0xF6u8, 0xF7] {
            for reg in 4u8..=7 {
                assert_eq!(
                    eu_cycles(op, 0b11_000_000 | (reg << 3)),
                    0,
                    "{op:#04X} /{reg} register form"
                );
                assert_eq!(
                    eu_cycles(op, 0b00_000_100 | (reg << 3)),
                    0,
                    "{op:#04X} /{reg} memory form"
                );
            }
        }
    }

    /// A store costs the EU more than a load, and the routines say why rather
    /// than asserting it: the store spends 0x000 and 0x001 in front of its
    /// write back, and a register destination has nothing in front of it.
    ///
    /// The direction bit is easy to read backwards, and the two forms are
    /// adjacent encodings, so this pins which way round it goes.
    #[test]
    fn a_mov_store_does_more_than_a_load() {
        use super::super::microcode::{self, Step};
        let mem = 0b00_000_100;
        let store = microcode::routine(0x88, mem, false, 0, None).expect("MOV r/m8, reg8");
        let load = microcode::routine(0x8A, mem, false, 0, None).expect("MOV reg8, r/m8");
        assert!(store.contains(Step::WriteOperand), "a store writes back");
        assert!(
            !load.contains(Step::WriteOperand),
            "a load's destination is a register"
        );
        assert_ne!(store, load);
    }

    /// LEA runs no bus cycle, so all of its 2 clocks are EU time and none of
    /// them is a transfer being subtracted. They are the way out of the address
    /// routine for a form that reads nothing: `1E3: tmpa -> IND` and the return
    /// behind it. Its own `004: IND -> R` costs nothing and shares the boundary
    /// fetch's clock.
    #[test]
    fn lea_is_two_clocks_and_no_transfers() {
        assert_eq!(eu_cycles(0x8D, 0b00_000_100), 0, "no row");
        assert_eq!(
            super::super::microcode::routine(0x8D, 0b00_000_100, false, 0, None)
                .expect("LEA runs a routine")
                .clocks(),
            2
        );
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

    /// A divide error never enters the loop, so no operand changes what `CORD`
    /// spent before it left, and the width does not either. What follows is
    /// `INT 0`, whose cost is the INTR routine's rather than a number here.
    #[test]
    fn a_divide_error_costs_the_same_however_it_was_caused() {
        // Division by zero, and a quotient too large for the destination, at
        // both widths and for `AAM`.
        for (word, dividend, divisor) in [
            (false, 0x1234, 0),
            (true, 0x1234_5678, 0),
            (false, 0xFF00, 1),
            (true, 0xFFFF_0000, 1),
        ] {
            assert!(
                matches!(
                    divide_routine_cycles(word, dividend, divisor),
                    Loop::Faulted(_)
                ),
                "{dividend:#X}/{divisor}"
            );
        }
        assert!(
            matches!(aam_routine_cycles(0x42, 0), Loop::Faulted(_)),
            "AAM by zero"
        );
    }

    /// `AAM` and `DIV` walk the same long division, so over the same operands
    /// they must differ by exactly the gap between what each spends around it,
    /// whatever the operands do to the loop. `DIV` spends eight and `AAM` four,
    /// both read off the reference, so the gap is four and does not move.
    ///
    /// That is the check that ties the two transcriptions together. The
    /// two-clock term for a last pass that subtracts was found on `AAM` when
    /// both were fitted, and it survived the move to a counted `CORD` because
    /// the routine's last-pass arms are what it was measuring all along.
    #[test]
    fn aam_and_divide_walk_the_same_loop() {
        for imm in 1..=255u8 {
            for al in [0u8, 1, 7, 8, 9, 10, 63, 64, 127, 128, 200, 255] {
                let div = divide_routine_cycles(false, u32::from(al), u32::from(imm));
                let aam = aam_routine_cycles(al, imm);
                match (div, aam) {
                    (Loop::Completed(div), Loop::Completed(aam)) => {
                        assert_eq!(div - aam, 4, "AL={al} imm={imm}");
                    }
                    // Both leave at 0x18a on the same operands, `AAM` three
                    // clocks into its routine and `DIV` six.
                    (Loop::Faulted(div), Loop::Faulted(aam)) => {
                        assert_eq!(div - aam, 3, "AL={al} imm={imm}");
                    }
                    _ => panic!("AL={al} imm={imm}: one faulted and the other did not"),
                }
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
        let (Loop::Completed(odd), Loop::Completed(even)) = (
            divide_routine_cycles(false, 202, 2),
            divide_routine_cycles(false, 200, 2),
        ) else {
            panic!("neither divide faults");
        };
        assert_eq!(odd, even + 3);
    }

    /// `AAD` multiplies by its immediate, one clock a set bit, which is the
    /// opposite operand from the one `MUL` tests.
    ///
    /// Asked of the routine, which is where the count lives now, and of
    /// [`corx_cycles`] underneath it. The co-routine is `6 * width + bits + 3`
    /// and `AAD` adds five of its own: 0x170, 0x171 and a jump in front, 0x172
    /// and 0x173 behind.
    #[test]
    fn the_ascii_multiply_counts_the_immediates_bits() {
        for (imm, bits) in [(0u8, 0u32), (0x0A, 2), (0xFF, 8)] {
            assert_eq!(imm.count_ones(), bits, "{imm:#04X}");
            assert_eq!(
                super::super::microcode::routine(0xD5, imm, false, 0, None)
                    .expect("AAD runs a routine")
                    .clocks(),
                corx_cycles(8, bits) + 5,
                "{imm:#04X}"
            );
        }
        // And the co-routine itself: five clocks a pass, a sixth for a set bit,
        // one short of the width in jumps back, and four at the ends.
        assert_eq!(corx_cycles(8, 0), 51);
        assert_eq!(corx_cycles(8, 8), 59);
        assert_eq!(corx_cycles(16, 0), 99);
    }

    /// The count follows the compared subtracts and ignores the immediate
    /// ones, which is the measured fact a quotient-based rule cannot express.
    ///
    /// `DIV r/m8` is quoted at 80 to 90 clocks. What is counted here runs two
    /// below that at both ends, for the reason [`PUBLISHED_OVERHEAD`] gives.
    #[test]
    fn divide_timing_lands_inside_the_published_range() {
        for divisor in 1..=255u32 {
            for dividend in [1u32, 0x0100, 0x3FFF, 0x7F00] {
                let Loop::Completed(c) = divide_routine_cycles(false, dividend, divisor) else {
                    continue;
                };
                assert!(
                    (80 - PUBLISHED_OVERHEAD..=90 - PUBLISHED_OVERHEAD).contains(&c),
                    "{dividend}/{divisor} gave {c}"
                );
            }
        }
    }

    /// What Intel's table charges that a microcode count does not: the two bytes
    /// of the instruction itself.
    ///
    /// The published figures are whole-instruction times and the transcriptions
    /// are the execution unit's own clocks, so the two differ by a constant, and
    /// it being the *same* constant across four independent endpoints at two
    /// widths and two instructions is what makes the counts checkable against a
    /// document at all.
    const PUBLISHED_OVERHEAD: u16 = 2;

    /// The multiply and divide counts must land on the ranges Intel published,
    /// which is the check that comes from neither the reference nor the vectors.
    ///
    /// **These used to be exact and the bases were fitted to make them so.**
    /// Reading `CORX` and the lines around it off the reference gives numbers
    /// two lower, uniformly, and the two are the instruction's own bytes. The
    /// old bases were the published endpoints with nothing between them and the
    /// recording, which is why they hit four endpoints and still ran a clock
    /// over on the bus.
    #[test]
    fn the_multiply_rules_reproduce_the_published_ranges() {
        // The published ranges are for a product whose high half is nonzero,
        // which is the ordinary case and the only one the suite exercises for
        // the word form.
        //
        // MUL r/m8 is quoted at 70 to 77 clocks; a byte has one to eight set
        // bits. MUL r/m16 at 118 to 133; a word has one to sixteen.
        for (word, multiplier, published) in [
            (false, 0x0001u16, 70u16),
            (false, 0x00FF, 77),
            (true, 0x0001, 118),
            (true, 0xFFFF, 133),
        ] {
            assert_eq!(
                multiply_routine_cycles(word, multiplier, false) + PUBLISHED_OVERHEAD,
                published,
                "word={word} multiplier={multiplier:#06X}"
            );
        }
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
    /// after the loop, all three falling through.
    ///
    /// The gap reads twelve against the counted `MUL` because the measured rule
    /// is a whole-instruction time and the count is the execution unit's own.
    /// The two between them are [`PUBLISHED_OVERHEAD`].
    #[test]
    fn the_signed_multiply_is_the_unsigned_one_plus_ten() {
        for (word, multiplier) in [(false, 0x0F), (true, 0x0FFF)] {
            assert_eq!(
                signed_multiply_cycles(word, 1, multiplier, false),
                multiply_routine_cycles(word, multiplier as u16, false) + 10 + PUBLISHED_OVERHEAD,
                "word={word}"
            );
        }
    }

    /// **The counted signed routines reproduce the measured table, exactly.**
    ///
    /// `PREIMUL`, `PREIDIV` and `NEGATE` were four numbers per instruction per
    /// width, solved out of grouped recorded spans over two passes, and the note
    /// beside them says two of the four had to come out negative before the
    /// groups would close. Counting the routines' arms off the reference gives
    /// all sixteen without solving anything: `10 + 2a - b` for `PREIMUL` with
    /// the `NEGATE` it falls into, twelve more when exactly one operand was
    /// negative, and `9 + 4d - v` for `PREIDIV`.
    ///
    /// This is the check that the two describe the same part. It sweeps every
    /// sign combination at both widths for both instructions, and the constant
    /// between them is the instruction's own bytes and nothing else.
    #[test]
    fn the_counted_signed_routines_reproduce_the_measured_table() {
        for word in [false, true] {
            let (a, b) = if word {
                (0x0FFFi32, 0x0033)
            } else {
                (0x0Fi32, 0x33)
            };
            for (multiplicand, multiplier) in [(b, a), (-b, a), (b, -a), (-b, -a)] {
                assert_eq!(
                    signed_multiply_routine_cycles(word, multiplicand, multiplier, false)
                        + PUBLISHED_OVERHEAD,
                    signed_multiply_cycles(word, multiplicand, multiplier, false),
                    "IMUL word={word} {multiplicand}x{multiplier}"
                );
            }
            // A dividend and divisor whose quotient fits the signed half, so
            // neither fault path is taken and `POSTIDIV` returns.
            let (dividend, divisor) = if word {
                (0x4000i64, 0x1000i64)
            } else {
                (0x40, 0x10)
            };
            for (dividend, divisor) in [
                (dividend, divisor),
                (-dividend, divisor),
                (dividend, -divisor),
                (-dividend, -divisor),
            ] {
                let Loop::Completed(counted) =
                    signed_divide_routine_cycles(word, dividend, divisor)
                else {
                    panic!("IDIV word={word} {dividend}/{divisor} should divide");
                };
                assert_eq!(
                    counted + PUBLISHED_OVERHEAD,
                    signed_divide_cycles(word, dividend, divisor),
                    "IDIV word={word} {dividend}/{divisor}"
                );
            }
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

    /// A product that fits in the low half costs one clock more, because
    /// `MULCOF`'s arm that leaves carry and overflow clear is the longer one:
    /// 0x1d0, a jump, 0x1cc and a jump, against 0x1d0, 0x1d1 and a jump.
    #[test]
    fn a_product_with_a_zero_high_half_costs_one_more() {
        for multiplier in [0x0001u16, 0x00FF] {
            assert_eq!(
                multiply_routine_cycles(false, multiplier, true),
                multiply_routine_cycles(false, multiplier, false) + 1,
                "multiplier={multiplier:#06X}"
            );
        }
    }

    /// The byte form looks at AL alone. `CORX` rotates `tmpc`, and `0x150: A ->
    /// tmpc` loads the accumulator one half wide. Reading AX would make the high
    /// byte, which the instruction overwrites with its result, change how long
    /// it takes.
    #[test]
    fn the_byte_multiply_ignores_the_high_half_of_the_accumulator() {
        assert_eq!(
            multiply_routine_cycles(false, 0x0001, false),
            multiply_routine_cycles(false, 0xFF01, false),
            "AH must not affect a byte multiply"
        );
        // Whereas the word form counts the whole accumulator.
        assert_ne!(
            multiply_routine_cycles(true, 0x0001, false),
            multiply_routine_cycles(true, 0xFF01, false)
        );
    }

    /// **No opcode may be priced twice.** An instruction whose microcode has
    /// been transcribed is walked step by step and never reaches
    /// [`eu_cycles`], so a row left behind for it is dead; but a row left
    /// behind is also how the two models come back to life together after
    /// somebody adds an arm here without checking. Since the bus unit grew a
    /// real address cycle the row is longer than the instruction by exactly the
    /// clocks it holds for one, so charging both is not a small error.
    ///
    /// This is the invariant that replaces the old per-family row assertions
    /// for the stack and the returns: those families are the microcode
    /// module's now, and `microcode`'s own sweeps hold their shape.
    #[test]
    fn an_opcode_with_a_routine_has_no_row() {
        for opcode in 0..=u8::MAX {
            for modrm in [0x00u8, 0xC0, 0x10, 0xD0, 0x20, 0xE0, 0x30, 0xF0] {
                for branch in [false, true] {
                    if super::super::microcode::routine(opcode, modrm, branch, 0, None).is_none() {
                        continue;
                    }
                    assert_eq!(
                        eu_cycles(opcode, modrm),
                        0,
                        "{opcode:#04X}/{modrm:#04X} is priced by a routine and by a row"
                    );
                    // **And by the other table.** A conditional form is priced
                    // by `branch_cycles` rather than by `eu_cycles`, so asking
                    // only the latter let `LOOP` carry a routine and a row at
                    // once without a word said. The two arms are asked
                    // separately because `branches_on_state` gates which table
                    // an opcode is read from at all.
                    if branches_on_state(opcode) {
                        assert_eq!(
                            branch_cycles(opcode, branch),
                            0,
                            "{opcode:#04X} is priced by a routine and by a branch row"
                        );
                    }
                }
            }
        }
    }

    /// `POP CS` keeps its row, and keeps it out of the ALU block it sits
    /// inside. Dispatching it through that block's `opcode & 7` would give it
    /// the accumulator-immediate cost of 4 by a different route and hide the
    /// omission.
    #[test]
    fn pop_cs_is_not_an_alu_operation() {
        assert_eq!(eu_cycles(0x0F, 0), 4, "POP CS");
    }

    /// The single-byte `INC`/`DEC` opcodes encode a *word* register and are
    /// measured at 2. Charging them what the `0xFE` group's byte-register form
    /// costs is the error this caught, and it was worth 80,000 vectors.
    ///
    /// The group form is the microcode module's now, and its one clock at
    /// 0x020 plus the ModR/M byte the single-byte forms do not carry is what
    /// makes the difference. These two are priced by different models, which is
    /// exactly the confusion the original error came from, so the separation is
    /// worth keeping pinned.
    #[test]
    fn the_single_byte_increments_are_the_word_form() {
        for op in 0x40u8..=0x4F {
            assert_eq!(eu_cycles(op, 0), 2, "{op:#04X}");
            assert!(
                super::super::microcode::routine(op, 0, false, 0, None).is_none(),
                "{op:#04X} is a row, not a routine"
            );
        }
        assert_eq!(
            eu_cycles(0xFE, 0b11_000_000),
            0,
            "the group form has no row"
        );
        assert_eq!(
            super::super::microcode::routine(0xFE, 0b11_000_000, false, 0, None)
                .expect("INC reg8 runs a routine")
                .clocks(),
            1,
            "INC reg8 spends 0x020 and nothing else"
        );
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
    ///
    /// Asked of the routine where one has been transcribed, and of the row
    /// otherwise. The property is the same either way and the point of the test
    /// is that it holds; asking only the rows would go quietly vacuous as each
    /// family moves to its microcode, which is exactly when it stops protecting
    /// anything.
    #[test]
    fn a_conditional_transfer_costs_more_when_it_transfers() {
        for opcode in [0x60u8, 0x70, 0x7F, 0xE0, 0xE1, 0xE2, 0xE3, 0xCE] {
            let taken = super::super::microcode::routine(opcode, 0, true, 0, None);
            let untaken = super::super::microcode::routine(opcode, 0, false, 0, None);
            match (taken, untaken) {
                (Some(taken), Some(untaken)) => {
                    assert!(taken.clocks() > untaken.clocks(), "{opcode:#04X} routine")
                }
                _ => assert!(
                    branch_cycles(opcode, true) > branch_cycles(opcode, false),
                    "{opcode:#04X} row"
                ),
            }
            assert!(branches_on_state(opcode), "{opcode:#04X}");
        }
        // And nothing else branches on the state, in particular the
        // unconditional transfers sitting next to them in the opcode map.
        for opcode in [0x9Au8, 0xC3, 0xCB, 0xCF, 0xE8, 0xE9, 0xEA, 0xEB] {
            assert!(!branches_on_state(opcode), "{opcode:#04X}");
            // Priced by a row or by a routine, but priced. The returns moved to
            // the microcode module and their rows went with them.
            assert!(
                eu_cycles(opcode, 0) > 0
                    || super::super::microcode::routine(opcode, 0, false, 0, None).is_some(),
                "{opcode:#04X}"
            );
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

    /// The five opcodes that reach memory without a ModR/M byte are modeled.
    ///
    /// The four moves price from their microcode now rather than from a row,
    /// and **the two directions are not symmetric**. Both read the displacement
    /// out of the queue with one `q_read_u16`, which is 0x064 and 0x065. The
    /// load then goes straight to `biu_read_u8`; the store spends a bare
    /// `self.cycle()` first, which is 0x066, and only then asks for the bus.
    ///
    /// The asymmetry is the thing worth pinning, because it is worth two clocks
    /// rather than one. Asking for the bus a clock early suppresses a prefetch
    /// decision the part takes, and the part's write then arrives behind the
    /// fetch that decision started and aborts it.
    ///
    /// `XLAT` is the fifth and it prices from a routine now too, three clocks
    /// that stand *in front of* its read: `mc_10c` spends 0x10c, 0x10d and
    /// 0x10e and only then calls `biu_read_u8`. It is the one opcode whose
    /// routine drives its own operand read.
    #[test]
    fn the_direct_address_moves_and_xlat_are_modeled() {
        for opcode in [0xA0u8, 0xA1, 0xA2, 0xA3] {
            assert_eq!(eu_cycles(opcode, 0), 0, "{opcode:#04X} has no row");
            // The store direction spends 0x066 and the load direction nothing.
            let stores = opcode & 0x02 != 0;
            assert_eq!(
                super::super::microcode::routine(opcode, 0, false, 0, None)
                    .expect("a direct-address move runs a routine")
                    .clocks(),
                u16::from(stores),
                "{opcode:#04X} spends 0x066 only when it writes"
            );
        }
        assert_eq!(eu_cycles(0xD7, 0), 0, "XLAT has no row");
        assert_eq!(
            super::super::microcode::routine(0xD7, 0, false, 0, None)
                .expect("XLAT runs a routine")
                .clocks(),
            3,
            "0x10c, 0x10d and 0x10e, all three in front of the read"
        );
        assert!(
            super::super::access::reads_from_its_routine(0xD7),
            "and the pipeline leaves the read to it"
        );
        for opcode in [0xA0u8, 0xA1, 0xA2, 0xA3, 0xD7] {
            assert!(is_modeled(opcode, 0), "{opcode:#04X}");
        }
    }

    /// The undocumented aliases run what the documented encodings run. The part
    /// decodes 0xC0 as 0xC2 and 0xC8 as 0xCA, and the recording gives the pairs
    /// identical spans.
    ///
    /// Asked of the routines rather than of rows, because that is where the
    /// returns are priced now. The aliasing is the thing worth pinning either
    /// way: it is a property of the part's decoder, not of whichever model
    /// happens to hold the clocks.
    #[test]
    fn the_return_aliases_run_what_they_alias() {
        for (alias, documented) in [(0xC0u8, 0xC2u8), (0xC1, 0xC3), (0xC8, 0xCA), (0xC9, 0xCB)] {
            assert_eq!(
                super::super::microcode::routine(alias, 0, false, 0, None),
                super::super::microcode::routine(documented, 0, false, 0, None),
                "{alias:#04X} against {documented:#04X}"
            );
        }
    }
}
