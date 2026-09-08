//! The part's microcode as data.
//!
//! A timing row says what an instruction *costs*. It cannot say where inside
//! that cost the instruction touches the bus, and that is what the recording
//! measures: `INT 3` and `INT n` take the same four vector reads and the same
//! three pushes, and the part interleaves a code fetch between the two vector
//! words and throws the queue away between the second push and the third. No
//! total, however exactly it is fitted, can express either.
//!
//! So the sequencer walks a step list instead. Each step is a thing the part's
//! microcode does that the outside world can see: clocks with the bus free, a
//! word onto or off the stack, the prefetcher stopping, the queue being thrown
//! away.
//!
//! The lists are transcribed from the published microcode rather than measured.
//! Where one disagrees with a timing row, the step list is the statement of
//! record and the row is the thing to delete.
//!
//! ## Reading a list against a trace
//!
//! The steps are the EU's, so a step's clock is the T-state the EU spends on
//! it. Bus cycles are not steps and do not appear: the EU asks for the bus and
//! the bus unit decides when the cycle actually starts, which is why an anchor
//! in a recording lands after the step that requested it, by as much of the
//! address cycle as the running bus cycle left to spend.

/// One step of an instruction's microcode.
///
/// Everything here is either a clock the EU spends or an instruction to the
/// bus unit. What the instruction *computes* is not modeled: that stays in
/// `execute.rs` and happens on [`Step::Run`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// `n` clocks of microcode with the bus free, so the prefetcher runs
    /// through them.
    Spend(u8),
    /// Stop prefetching, the `SUSP` of the published microcode.
    ///
    /// Free in itself. It costs clocks only when a code fetch is already
    /// latched, and then only until that fetch reaches its last T-state. A
    /// fetch that has got no further than computing an address is canceled
    /// outright. Only a [`Step::Flush`] lifts the suspension.
    Susp,
    /// Throw the queue away and start the reload.
    ///
    /// **Costs one clock**, and lifts a suspension. The reload's bus cycle
    /// begins after its address cycle, so a flush placed before a push is
    /// visible in the recording as a code fetch that beats the write to the
    /// bus.
    Flush,
    /// Read one word through the interrupt vector table, low byte first.
    ReadVectorWord,
    /// Read the segment half of a far pointer, the word two bytes above the
    /// operand's address.
    ///
    /// **A far pointer is two reads, not one, and the microcode runs between
    /// them.** The address routine's operand load reads the offset word and
    /// nothing else: `read_operand_farptr` in the reference takes the offset
    /// from `ea_opr`, which the load already filled, and puts only the segment
    /// word on the bus. Whatever the instruction spends in between is time the
    /// bus is free, which is why `les cx, dword [ds:di]` gets a code fetch
    /// between its two words and a core that runs four byte cycles back to back
    /// cannot.
    ReadPointerSegment,
    /// Put one staged word on the stack.
    ///
    /// The words are staged in the order the executor pushed them, so the
    /// first `Push` of a routine writes the deepest.
    Push,
    /// Take one word off the stack, before the instruction that wants it runs.
    ///
    /// A routine's first `Pop` reads the word at SP, the second the one above
    /// it. They land in the same buffer the executor's own `pop16` reads from,
    /// in the same order, so the body sees what the sequencer fetched.
    Pop,
    /// Read the instruction's I/O port, one cycle per byte at consecutive port
    /// numbers. Ahead of [`Step::Run`], which is what puts the byte in the
    /// accumulator.
    ReadPort,
    /// Write it, behind [`Step::Run`], which is what decides the byte.
    WritePort,
    /// Write the instruction's memory operand back, behind [`Step::Run`].
    ///
    /// Only for an operand that resolved to an address. A register destination
    /// is written by the body itself and costs no bus cycle, so a routine's
    /// register form simply has no `WriteOperand` in it.
    WriteOperand,
    /// Run the instruction body, which is what decides the values the pushes
    /// carry and where control goes.
    ///
    /// Costs nothing. It is the hand-off from the sequencer to `execute.rs`,
    /// not a clock: the part computes as it goes and the clocks it spends
    /// doing so are the `Spend`s around this.
    Run,
    /// Point the instruction stream at the words already taken off the stack,
    /// without running the rest of the instruction.
    ///
    /// **`Run` is atomic and several routines are not.** A far return pops its
    /// offset and its segment, throws the queue away, and only then pops the
    /// flags: the reload has to be on the bus before the third read starts. A
    /// single hand-off to `execute.rs` cannot express that, because the
    /// executor sets the flags in the same breath as the transfer. This is the
    /// transfer alone, so a `Flush` can follow it with the rest of the
    /// instruction still to come.
    ///
    /// `far` says whether a segment was popped as well as an offset. Word 0 is
    /// the offset and word 1 the segment, which is the order they came off the
    /// stack in.
    Transfer { far: bool },
}

/// The most steps any one routine takes. The longest is the interrupt's, at
/// fourteen.
pub(crate) const MAX_STEPS: usize = 24;

/// One instruction's microcode, built for the case in hand.
///
/// **Routines cannot be static.** Nearly every one in the published microcode
/// branches: on whether the operand is in memory or a register, on whether the
/// return is near or far, on whether a conditional transfer is taken, on the
/// shift count in CL. A table of `&'static [Step]` can hold the ones that do
/// not branch and nothing else, which is why the first few transcriptions were
/// exactly the ones that happen not to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Routine {
    steps: [Step; MAX_STEPS],
    len: u8,
}

impl Routine {
    /// The step at `index`, or `None` past the end.
    fn get(&self, index: u8) -> Option<Step> {
        (index < self.len).then(|| self.steps[index as usize])
    }

    /// The steps it holds, for the transcription's own tests.
    #[cfg(test)]
    fn steps(&self) -> &[Step] {
        &self.steps[..self.len as usize]
    }

    /// Whether it contains `step`, for cross-checks in neighboring modules.
    #[cfg(test)]
    pub(crate) fn contains(&self, step: Step) -> bool {
        self.steps[..self.len as usize].contains(&step)
    }

    /// The clocks its `Spend`s add up to, which is its microcode time and not
    /// its span: the bus steps around them cost whatever the bus unit spends.
    #[cfg(test)]
    pub(crate) fn clocks(&self) -> u8 {
        self.steps[..self.len as usize]
            .iter()
            .map(|s| match s {
                Step::Spend(n) => *n,
                _ => 0,
            })
            .sum()
    }
}

/// Assembles a [`Routine`] a step at a time.
///
/// `spend(0)` is dropped rather than stored, so a routine can be written the
/// way the published microcode reads, with a conditional clock count in line,
/// and never produce a step that occupies no T-state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Build {
    routine: Routine,
}

impl Build {
    fn new() -> Self {
        Self {
            routine: Routine {
                steps: [Step::Run; MAX_STEPS],
                len: 0,
            },
        }
    }

    /// Add a step.
    fn then(mut self, step: Step) -> Self {
        assert!(
            (self.routine.len as usize) < MAX_STEPS,
            "a routine longer than {MAX_STEPS} steps"
        );
        self.routine.steps[self.routine.len as usize] = step;
        self.routine.len += 1;
        self
    }

    /// Add `n` clocks of microcode, or nothing at all when `n` is zero.
    fn spend(self, n: u8) -> Self {
        if n == 0 {
            self
        } else {
            self.then(Step::Spend(n))
        }
    }

    fn done(self) -> Routine {
        self.routine
    }
}

/// Start a routine.
fn mc() -> Build {
    Build::new()
}

/// The routine `opcode` runs with this `modrm`, or `None` for an instruction
/// still priced from a timing row.
///
/// Transcribing an opcode means adding an arm here and deleting whatever in
/// `timing.rs` used to price it. The two must never both be live for the same
/// opcode: the row would be charged on top of the steps, and since the bus unit
/// grew a real address cycle the row is longer than the instruction by exactly
/// the clocks it holds for one.
///
/// The group opcodes are asked for their reg field because the group is not one
/// instruction: `FF` carries `INC` and `PUSH`, which go nowhere, beside the
/// indirect calls and jumps, which are transfers.
pub(crate) fn routine(opcode: u8, modrm: u8, branch: bool) -> Option<Routine> {
    let register_form = modrm >> 6 == 3;
    match opcode {
        // The conditional jumps at 0x0e8, opcodes 70 through 7F and their
        // aliases sixteen below. `mc_0e8` tests the flag, reads the
        // displacement, spends one clock at 0x0e9 and only then, if it is
        // taken, falls into RELJMP.
        //
        // The displacement read is the loader's, so what is left here is that
        // one clock and the transfer behind it. RELJMP itself is `JMP rel8`'s
        // routine without the jump into it: suspend, 0x0d2, 0x0d3, CORR, 0x0d4,
        // the flush, and 0x0d5.
        0x60..=0x7F => {
            // 0x0e9.
            let r = mc().spend(1);
            if !branch {
                return Some(r.then(Step::Run).done());
            }
            Some(
                r.then(Step::Run)
                    // The jump into RELJMP. `reljmp2` takes it when the caller
                    // fell in rather than through, which the conditional forms
                    // do, and a taken `JO` shows the `JMP` clock in front of its
                    // suspend.
                    .spend(1)
                    .then(Step::Susp)
                    // 0x0d2, 0x0d3, CORR, 0x0d4.
                    .spend(4)
                    .then(Step::Flush)
                    // 0x0d5.
                    .spend(1)
                    .done(),
            )
        }

        // -------------------------------------------------------------------
        // The ModR/M groups. These begin *after* the pipeline's operand read,
        // because that read is the published `load_operand`, which runs between
        // decode and the routine.
        //
        // The two clocks every memory form spends at the front are its return
        // delay: 0x1e2 and the return when the address was loaded, 0x1e3 and
        // the return when it was computed and not loaded. A register form has
        // no address and spends neither.
        // -------------------------------------------------------------------

        // `MOV r/m, reg` and `MOV reg, r/m` at 0x000, opcodes 88 through 8B.
        // The store direction spends 0x000 and 0x001 before the write; the load
        // direction, whose destination is a register, spends nothing at all.
        // A register-to-register `MOV` runs no microcode: its two clocks are
        // the opcode and the ModR/M byte.
        0x88..=0x8B => {
            let stores = opcode & 0x02 == 0;
            let mut r = mc();
            if !register_form {
                // **The two ways out of the effective-address routine cost
                // differently, and `MOV` is where it shows.** The load direction
                // leaves through `1E2: OPR -> tmpb`, which is the line that
                // spends the operand read's T4, so only `RET` is left and the
                // front of the routine is one clock. The store direction never
                // reads: it leaves through `1E3: tmpa -> IND`, which has a clock
                // of its own, and `RET` behind it, so the front is two.
                //
                // The probe has both on screen. `mov cl, byte [ss:bp+di-64h]`
                // runs `1E2` on the read's T4, `RET`, and then reaches
                // `000: XA -> tmpb` on the same T-state as `FETCH_NEXT`.
                // `mov byte [ds:bx+di+Dh], ch` runs `1E3`, `RET`, `000`, `001`,
                // and asks for the bus on the clock after that.
                r = r.spend(if stores { 2 } else { 1 });
                if stores {
                    // 0x000 and 0x001.
                    r = r.spend(2);
                }
            }
            r = r.then(Step::Run);
            if !register_form && stores {
                r = r.then(Step::WriteOperand);
            }
            Some(r.done())
        }

        // The ALU block's register forms at 0x008: `ADD`, `OR`, `ADC`, `SBB`,
        // `AND`, `SUB`, `XOR` and `CMP` against a ModR/M operand, opcodes
        // 00 through 3B with the low two bits selecting the direction.
        //
        // One clock at 0x008 whichever way it goes, then 0x009 and 0x00a before
        // a write back to memory. `CMP` writes nothing and spends neither.
        0x00..=0x3B if opcode & 0xC4 == 0 && opcode & 0x07 < 4 => {
            let stores = opcode & 0x02 == 0;
            let compares = (opcode >> 3) & 7 == 7;
            let mut r = mc();
            if !register_form {
                // The effective-address routine's return, and only that. The
                // reference probe puts the whole seam on one screen: the operand
                // read's T4 is spent by `1E2: OPR -> tmpb`, `RET` spends the
                // clock behind it, and `008: M -> tmpa` runs on the next. It was
                // two while the loader took its bytes a T-state late. See
                // [`I8088::preload`].
                r = r.spend(1);
            }
            // 0x008.
            r = r.spend(1).then(Step::Run);
            if !register_form && stores && !compares {
                // 0x009, 0x00a.
                r = r.spend(2).then(Step::WriteOperand);
            }
            Some(r.done())
        }

        // `ALU r/m, imm` at 0x00c, opcodes 80 through 83. The jump over the
        // immediate's second queue read is spent by every form that has only
        // one byte to read, and `83` is the odd one out: a word-sized
        // instruction with a byte-sized immediate, which takes the jump anyway.
        // `81`, the only one carrying a real word immediate, does not.
        0x80..=0x83 => {
            let compares = (modrm >> 3) & 7 == 7;
            let one_immediate_byte = opcode != 0x81;
            // **A memory form spends nothing at the front.** Its deferred
            // immediate is the loader's, and the effective-address routine's
            // return is already the pause in front of that read
            // (`timing::deferred_immediate_stall`). The reference probe puts the
            // whole seam in view on `add byte [ds:bx+si-64h], FAh`: `RET` on one
            // clock, `00C: Q -> tmpbL` reading the immediate on the next, the
            // jump behind it, then `00E` and the write request. Spending two
            // here charged the return a second time and put the write two clocks
            // late.
            let mut r = mc().spend(u8::from(one_immediate_byte)).then(Step::Run);
            if !register_form {
                // 0x00e.
                r = r.spend(1);
                if !compares {
                    r = r.then(Step::WriteOperand);
                }
            }
            Some(r.done())
        }

        // `MOV r/m, imm` at 0x014, opcodes C6 and C7. Write-only, so its
        // address is computed and not loaded, and the clock at 0x016 is an
        // end-of-instruction for a register destination and a real one for a
        // memory destination.
        0xC6 | 0xC7 => {
            let mut r = mc();
            if !register_form {
                r = r.spend(2);
            }
            r = r.spend(u8::from(opcode == 0xC6)).then(Step::Run);
            if !register_form {
                // 0x016.
                r = r.spend(1).then(Step::WriteOperand);
            }
            Some(r.done())
        }

        // `LES` at 0x0f0 and `LDS` at 0x0f4, the far-pointer loads. The address
        // routine's operand load brought back the offset word and left through
        // `1E2: OPR -> tmpb`, so the front is `RET` and the routine's own two
        // lines, and the segment word goes out behind them. Nothing follows that
        // read, so its T4 belongs to the boundary fetch.
        //
        // Those two clocks are the whole point of the split. The reference's
        // prefetch decision at the offset word's T4 sees no request pending,
        // because the segment word is not asked for until `0F1`, and starts a
        // code fetch that lands between the two words.
        0xC4 | 0xC5 => Some(
            // `RET`, then 0x0f0 and 0x0f1, or 0x0f4 and 0x0f5.
            mc().spend(3)
                .then(Step::ReadPointerSegment)
                .then(Step::Run)
                .done(),
        ),

        // `ALU accumulator, imm` at 0x018, and `MOV reg, imm` at 0x01c. Neither
        // touches memory, and the only clock either spends is the jump over the
        // immediate's second queue read, taken by the byte-sized forms.
        0x04 | 0x05 | 0x0C | 0x0D | 0x14 | 0x15 | 0x1C | 0x1D | 0x24 | 0x25 | 0x2C | 0x2D
        | 0x34 | 0x35 | 0x3C | 0x3D => {
            Some(mc().spend(u8::from(opcode & 1 == 0)).then(Step::Run).done())
        }
        0xB0..=0xBF => Some(
            mc().spend(u8::from(opcode & 0x08 == 0))
                .then(Step::Run)
                .done(),
        ),

        // -------------------------------------------------------------------
        // The stack. Three clocks in front of a push and none at all in front
        // of a pop, which is the whole asymmetry between the two families.
        // -------------------------------------------------------------------

        // `PUSH r16` at 0x028, `PUSH sreg` at 0x02c and `PUSHF` at its own
        // three unnumbered lines. The part spends the same three whichever it
        // is, and then the word goes out.
        0x50..=0x57 | 0x06 | 0x0E | 0x16 | 0x1E | 0x9C => {
            Some(mc().spend(3).then(Step::Run).then(Step::Push).done())
        }

        // `POP r16` at 0x034, `POP sreg` at 0x038 and `POPF` at 0x03c. **No
        // microcode in front of the read at all**: the routine is the read,
        // and the register is not written until the word is back.
        //
        // `POP CS` at 0x0F keeps its row. It is a pop on this part and a prefix
        // escape on every later one, and it is the one member of the family the
        // row already prices exactly.
        0x58..=0x5F | 0x07 | 0x17 | 0x1F | 0x9D => {
            Some(mc().then(Step::Pop).then(Step::Run).done())
        }

        // -------------------------------------------------------------------
        // The returns. `RET` near has a routine of its own at 0x0bc; every
        // other form goes through FARRET at 0x0c0, which pops the offset,
        // suspends, and then either flushes or pops a segment first.
        // -------------------------------------------------------------------

        // `RET` near at 0x0bc, and 0xC1, its undocumented alias one encoding
        // below. Pop the offset, point the stream at it, stop prefetching, one
        // clock, throw the queue away, two more.
        0xC1 | 0xC3 => Some(
            mc().then(Step::Pop)
                .then(Step::Transfer { far: false })
                .then(Step::Susp)
                // 0x0bd.
                .spend(1)
                .then(Step::Flush)
                // 0x0be, 0x0bf.
                .spend(2)
                .then(Step::Run)
                .done(),
        ),

        // `RET` far at 0x0c0 into FARRET, and 0xC9 below it. One clock, the
        // jump into the routine, the offset off the stack, `SUSP`, two clocks,
        // the jump into the far arm, the segment off the stack, then the flush
        // with two behind it.
        0xC9 | 0xCB => Some(
            // 0x0c0 and FARRET's MC_JUMP.
            mc().spend(2)
                .then(Step::Pop)
                .then(Step::Susp)
                // 0x0c3, 0x0c4, then the jump into the far arm.
                .spend(3)
                .then(Step::Pop)
                .then(Step::Transfer { far: true })
                .then(Step::Flush)
                // 0x0c7 and the return.
                .spend(2)
                .then(Step::Run)
                .done(),
        ),

        // `RET imm16`, near and far, at 0x0cc, with 0xC0 and 0xC8 below them.
        // The same FARRET, entered without 0x0c0's clock in front of it, and
        // with 0x0ce behind it for the stack release. The far arm costs one
        // more clock than the near, for its jump.
        0xC0 | 0xC2 | 0xC8 | 0xCA => {
            let far = opcode & 0x08 != 0;
            let mut r = mc()
                // FARRET's MC_JUMP.
                .spend(1)
                .then(Step::Pop);
            if !far {
                r = r.then(Step::Transfer { far: false });
            }
            r = r.then(Step::Susp).spend(if far { 3 } else { 2 });
            if far {
                r = r.then(Step::Pop).then(Step::Transfer { far: true });
            }
            Some(
                r.then(Step::Flush)
                    // 0x0c5 or 0x0c7, the return, and 0x0ce.
                    .spend(3)
                    .then(Step::Run)
                    .done(),
            )
        }

        // `IRET` at 0x0c8 into FARRET and then the flags. **This is the routine
        // the split between `Transfer` and `Run` exists for**: the flags come
        // off the stack after the queue has been thrown away, so the reload at
        // the return address is on the bus before the third read starts.
        0xCF => Some(
            // 0x0c8 and FARRET's MC_JUMP.
            mc().spend(2)
                .then(Step::Pop)
                .then(Step::Susp)
                .spend(3)
                .then(Step::Pop)
                .then(Step::Transfer { far: true })
                .then(Step::Flush)
                .spend(2)
                .then(Step::Pop)
                // 0x0ca.
                .spend(1)
                .then(Step::Run)
                .done(),
        ),

        // -------------------------------------------------------------------
        // The ports. A read goes on the bus before the accumulator is written
        // and a write after it is read, and the four encodings differ only in
        // how many clocks sit in front of the transfer: one for an immediate
        // port read, two for an immediate port write, none at all through DX,
        // and one for a write through DX.
        // -------------------------------------------------------------------

        // `IN A, imm8` at 0x0ac, whose 0x0ad is the one clock.
        0xE4 | 0xE5 => Some(mc().spend(1).then(Step::ReadPort).then(Step::Run).done()),
        // `OUT imm8, A` at 0x0b0, with 0x0b1 and 0x0b2.
        0xE6 | 0xE7 => Some(mc().spend(2).then(Step::Run).then(Step::WritePort).done()),
        // `IN A, DX` at 0x0b4. No microcode in front of the read at all, the
        // port being in a register already.
        0xEC | 0xED => Some(mc().then(Step::ReadPort).then(Step::Run).done()),
        // `OUT DX, A` at 0x0b8, whose 0x0b8 is the one clock.
        0xEE | 0xEF => Some(mc().spend(1).then(Step::Run).then(Step::WritePort).done()),

        // -------------------------------------------------------------------
        // The unconditional transfers that carry their target in the
        // instruction. All four suspend, correct the program counter, throw the
        // queue away and reload; they differ in what they push and in whether
        // the encoding jumps into the shared routine or falls through to it.
        //
        // `Run` sits at the front of each rather than beside its pushes: it is
        // what points the stream at the target, and the flush behind it reloads
        // from there.
        // -------------------------------------------------------------------

        // `CALL rel16` at 0x07c, which has its own routine and does not go
        // through NEARCALL: 0x07e, 0x07f, CORR and 0x080, then the flush, then
        // 0x081, 0x082 and a jump, then the return offset.
        0xE8 => Some(
            mc().then(Step::Run)
                .then(Step::Susp)
                .spend(4)
                .then(Step::Flush)
                .spend(3)
                .then(Step::Push)
                .done(),
        ),

        // `JMP rel16` and `JMP rel8` through RELJMP at 0x0d2.
        //
        // **Neither form spends a clock for the jump into the routine.** The
        // byte form is written as the arm that takes it, but its trace carries
        // no `JMP` line at all: the reference probe shows `0D0`, `0D1` and the
        // suspend wait running straight into `0D2`. Charging it here put every
        // case of the file one clock late from the queue read onward, which
        // `fetch_gap_diff` reads as a reload at gap 11 against the part's 10.
        0xE9 | 0xEB => Some(
            mc().then(Step::Run)
                .then(Step::Susp)
                // 0x0d2, 0x0d3, CORR, 0x0d4.
                .spend(4)
                .then(Step::Flush)
                // 0x0d5.
                .spend(1)
                .done(),
        ),

        // `JMP far` at 0x0e0. No push and no correction: the target is absolute,
        // so it suspends, spends 0x0e4 and 0x0e5, flushes and spends 0x0e6.
        0xEA => Some(
            mc().then(Step::Run)
                .then(Step::Susp)
                .spend(2)
                .then(Step::Flush)
                .spend(1)
                .done(),
        ),

        // `CALL far` at 0x070 through FARCALL, which pushes the return segment
        // before it reaches NEARCALL and the return offset after the flush.
        // That split is why the recording shows the reload at the target
        // between the two writes.
        0x9A => Some(
            mc().then(Step::Run)
                // The jump into FARCALL.
                .spend(1)
                .then(Step::Susp)
                // 0x06b, 0x06c, CORR, 0x06d.
                .spend(4)
                .then(Step::Push)
                // 0x06e, 0x06f, then NEARCALL's jump.
                .spend(3)
                .then(Step::Flush)
                // 0x077, 0x078, 0x079.
                .spend(3)
                .then(Step::Push)
                .done(),
        ),

        // -------------------------------------------------------------------
        // The interrupt, and the indirect transfers.
        // -------------------------------------------------------------------

        // `INT n` at 0x1a8 goes straight into INTR with nothing in front of it.
        // The published routine has a jump there and the reference skips it,
        // saying so: "Another cycle deviance here between observed timings and
        // microcode."
        0xCD => Some(interrupt(0)),
        // `INT 3` at 0x1b0 spends 0x1b1, 0x1b2 and the jump into INTR. The
        // published routine jumps over a blank line and the reference does not
        // reproduce that, for the same reason.
        0xCC => Some(interrupt(3)),

        // The indirect calls and jumps live in the `FF` group, beside `INC`,
        // `DEC` and `PUSH`, which do not transfer and keep their rows.
        //
        // `FF /3` and `FF /5` reach their far pointer in two halves, with their
        // own microcode in the middle: the address routine's load brings back
        // the offset word, the routine spends its lines, and only then does the
        // segment word go on the bus. [`Step::ReadPointerSegment`] is what lets
        // a routine say that.
        // `INC r/m` and `DEC r/m` at 0x020, the reg 0 and 1 forms of the FE and
        // FF groups. One clock at 0x020 whichever the operand, and 0x021 as
        // well when it is in memory.
        0xFE | 0xFF if (modrm >> 3) & 7 < 2 => {
            let mut r = mc();
            if !register_form {
                r = r.spend(2);
            }
            // 0x020.
            r = r.then(Step::Run).spend(1);
            if !register_form {
                // 0x021.
                r = r.spend(1).then(Step::WriteOperand);
            }
            Some(r.done())
        }

        0xFF => match (modrm >> 3) & 7 {
            // `CALL r/m16` at 0x074. The register form spends a clock the
            // memory form does not; everything after is `CALL rel16`'s routine.
            2 => Some(
                mc().then(Step::Run)
                    // 0x074, spent only when the operand was a register.
                    .spend(u8::from(register_form))
                    .then(Step::Susp)
                    // 0x074, 0x075, CORR, 0x076.
                    .spend(4)
                    .then(Step::Flush)
                    // 0x077, 0x078, 0x079.
                    .spend(3)
                    .then(Step::Push)
                    .done(),
            ),
            // `CALL FAR r/m` at 0x068, which is FARCALL with the pointer read
            // in front of it. `RET` and 0x068, the segment word, then the jump
            // into FARCALL on that read's release clock.
            //
            // FARCALL itself is `SUSP`, 0x06b, 0x06c, CORR and 0x06d, the
            // return segment, 0x06e and 0x06f, and then NEARCALL: the jump the
            // flush sits on, 0x077, 0x078, 0x079 and the return offset. The tail
            // from the flush down is `CALL rel16`'s, which is why the two agree
            // clock for clock once the pointer is in hand.
            3 if !register_form => Some(
                mc().spend(2)
                    .then(Step::ReadPointerSegment)
                    .then(Step::Run)
                    // MC_JUMP into FARCALL, which is the read's release clock.
                    .spend(1)
                    .then(Step::Susp)
                    // 0x06b, 0x06c, CORR, 0x06d.
                    .spend(4)
                    .then(Step::Push)
                    // 0x06e, 0x06f, then NEARCALL's jump.
                    .spend(3)
                    .then(Step::Flush)
                    // 0x077, 0x078, 0x079.
                    .spend(3)
                    .then(Step::Push)
                    .done(),
            ),
            // `JMP r/m16` at 0x0d8. No pushes and one clock of microcode.
            4 => Some(
                mc().then(Step::Run)
                    .spend(u8::from(register_form))
                    .then(Step::Susp)
                    // 0x0d8.
                    .spend(1)
                    .then(Step::Flush)
                    .done(),
            ),
            // `JMP FAR r/m` at 0x0dc. `RET` and 0x0dc, then the prefetcher
            // stops, 0x0dd goes by, and the segment word goes out. The flush is
            // behind the read, so it lands on that read's release clock.
            5 if !register_form => Some(
                mc().spend(2)
                    .then(Step::Susp)
                    // 0x0dd.
                    .spend(1)
                    .then(Step::ReadPointerSegment)
                    .then(Step::Run)
                    .then(Step::Flush)
                    .done(),
            ),
            _ => None,
        },
        _ => None,
    }
}

/// The interrupt routine, entered by `INT n`, `INT 3`, a taken `INTO` and a
/// hardware interrupt alike.
///
/// Transcribed from the published microcode at 0x19d, and every anchor of it is
/// visible in the recording of `CD`:
///
/// - **The single clock between the two vector words is the whole reason this
///   list exists.** The prefetcher is still running there, and the part spends
///   that clock starting a code fetch which lands between the two reads. A core
///   that runs the four byte cycles back to back cannot fit it, and the fetch
///   comes out at the far end of the instruction instead.
/// - `SUSP` is after the reads, not before them, which is why that fetch
///   happens at all.
/// - The queue is thrown away **between the second push and the third**, so the
///   reload at the handler is on the bus before the return offset is. This core
///   used to write all three words and then flush, which puts the same three
///   writes and the same fetch on the bus in the wrong order.
///
/// The `entry` count is what the opcode spends before it reaches INTR: nothing
/// for `INT n`, three clocks for `INT 3`. Everything behind it is shared, which
/// is why one routine with an entry cost is the right shape rather than two
/// lists to keep in step by hand.
fn interrupt(entry: u8) -> Routine {
    mc().spend(entry)
        // 0x19d, 0x19e, 0x19f.
        .spend(3)
        // The handler's offset.
        .then(Step::ReadVectorWord)
        // 0x1a1, the clock the recording's code fetch uses.
        .spend(1)
        // And its segment.
        .then(Step::ReadVectorWord)
        // 0x1a3 SUSP, then 0x1a3, 0x1a4, 0x1a5.
        .then(Step::Susp)
        .spend(3)
        // The vector is in hand, so the transfer can be computed and the three
        // words staged.
        .then(Step::Run)
        .then(Step::Push)
        // 0x1a6, the jump into FARCALL2, 0x06c, CORR and 0x06d.
        //
        // **Five, as the published routine reads.** This was six for a long
        // time and the six was never explained: reading the routine off gives
        // 0x1a6, the jump, 0x06c, CORR and 0x06d, which is five, and the
        // recording wanted one more. The extra clock was the address cycle the
        // push in front of it did not have, showing up one seam further on. It
        // was never a property of the microcode and there was never a microcode
        // line to hang it on.
        .spend(5)
        .then(Step::Push)
        // 0x06e, 0x06f, then NEARCALL's MC_JUMP.
        .spend(3)
        .then(Step::Flush)
        // 0x077, 0x078, 0x079.
        .spend(3)
        .then(Step::Push)
        .done()
}

/// How far into a routine the sequencer is, and what it has already done that a
/// step alone does not say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cursor {
    /// The routine being walked.
    pub(crate) steps: Routine,
    /// The next step to run.
    pub(crate) at: u8,
    /// How many words the routine has already pushed, which is the slot the
    /// next [`Step::Push`] writes.
    pub(crate) pushed: u8,
    /// How many words it has already taken off the stack, which is the slot the
    /// next [`Step::Pop`] fills.
    pub(crate) popped: u8,
    /// How many vector words it has already read.
    pub(crate) read: u8,
    /// Whether the routine has already thrown the queue away.
    ///
    /// A routine that flushes in the middle of itself goes on to run the rest
    /// of the instruction, and the executor points the stream at the target
    /// again on its way through. Without this the instruction retires still
    /// owing a transfer and flushes a second time.
    pub(crate) flushed: bool,
    /// Whether [`Step::Susp`] has handed back the clock it was reached on so
    /// that it can be asked again on the next one.
    ///
    /// **The published suspend runs at a clock boundary and this sequencer runs
    /// inside one.** `biu_fetch_suspend` is called between two `cycle_i` calls,
    /// and `cycle_i`'s tail latches a waiting address cycle and promotes it to
    /// `T1` before it returns, so the suspend sees a fetch that this core's
    /// execution unit, running before `tick_bus`, does not. Handing the clock
    /// back puts the question at the boundary the part asks it on.
    pub(crate) suspend_deferred: bool,
}

impl Cursor {
    /// Start `steps` from the beginning.
    ///
    /// **There is no lead-in clock here, and that is measured rather than
    /// overlooked.** The published `execute_instruction` spends one before the
    /// routine begins, when the last queue operation was a First Byte, which is
    /// every instruction that read nothing after its opcode. Transcribed
    /// literally it costs 6.7 points of the clean population, 48.20% to 41.50%,
    /// and takes `POP r16`, `RET` near and `IRET` off cycle-for-cycle exact to
    /// `+1`.
    ///
    /// The reason is a difference in where the two models put the read. The
    /// published loader spends a clock reporting the First Byte and only then
    /// enters the routine, so its lead-in is the first clock the microcode
    /// owns. This loader reads the byte and starts the routine within the same
    /// T-state, so that clock has already been spent by the time a cursor
    /// exists, and charging it again charges it twice.
    ///
    /// The companion branch there is a deferred RNI carried over from the
    /// previous instruction, on a flag nothing in the reference ever sets.
    pub(crate) fn new(steps: Routine) -> Self {
        Self {
            steps,
            at: 0,
            pushed: 0,
            popped: 0,
            read: 0,
            flushed: false,
            suspend_deferred: false,
        }
    }

    /// Take the next step, or `None` when the routine is over.
    pub(crate) fn next(&mut self) -> Option<Step> {
        let step = self.steps.get(self.at)?;
        self.at += 1;
        Some(step)
    }

    /// Put a [`Step::Susp`] back so it is asked again on the next clock.
    ///
    /// `SUSP` stops the prefetcher at once and then waits for a fetch already
    /// on the bus, so it is re-run rather than resumed: the prefetcher stays
    /// stopped and the question each clock is only whether that fetch has
    /// reached its last T-state yet.
    pub(crate) fn rewind_to_wait(&mut self) {
        self.at -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(r: &Routine, step: Step) -> usize {
        r.steps().iter().filter(|s| **s == step).count()
    }

    fn position(r: &Routine, step: Step) -> Option<usize> {
        r.steps().iter().position(|s| *s == step)
    }

    /// Every opcode a routine claims, so the invariants below can sweep the
    /// whole transcription rather than the one routine somebody remembered.
    fn every_routine() -> Vec<(u8, u8, Routine)> {
        let mut out = Vec::new();
        for opcode in 0..=u8::MAX {
            for modrm in [0x00u8, 0xC0, 0x10, 0xD0, 0x20, 0xE0, 0x30, 0xF0] {
                if let Some(r) = routine(opcode, modrm, false) {
                    out.push((opcode, modrm, r));
                }
            }
        }
        out
    }

    /// The one invariant that keeps a transcription honest: a routine has to
    /// stage exactly as many words as the executor pushes, or the sequencer
    /// writes a slot nothing filled.
    #[test]
    fn the_interrupt_routine_pushes_the_three_words_it_stages() {
        for entry in [0, 3] {
            assert_eq!(
                count(&interrupt(entry), Step::Push),
                3,
                "the interrupt pushes the flags, the return segment and the return offset"
            );
        }
    }

    /// `INT 3` is `INT n` with three clocks in front of it and nothing else
    /// different. The two share every transfer and every seam, which is what
    /// makes one routine with an entry count the right shape rather than two
    /// lists that have to be kept in step by hand.
    #[test]
    fn int_3_is_the_interrupt_routine_with_an_entry_cost() {
        let n = interrupt(0);
        let three = interrupt(3);
        assert_eq!(
            three.steps()[0],
            Step::Spend(3),
            "INT 3 spends 0x1b1, 0x1b2 and the jump"
        );
        assert_eq!(
            &three.steps()[1..],
            n.steps(),
            "everything behind the entry is the same routine"
        );
    }

    /// And reads both halves of the vector before it needs either.
    #[test]
    fn the_vector_is_read_before_the_transfer_is_computed() {
        let r = interrupt(0);
        let run = position(&r, Step::Run).expect("the routine runs the instruction");
        let reads = r.steps()[..run]
            .iter()
            .filter(|s| **s == Step::ReadVectorWord)
            .count();
        assert_eq!(reads, 2, "both vector words are read before the transfer");
    }

    /// The placement the whole list exists for: the queue is thrown away with
    /// one push still to go, so the reload beats the last write to the bus.
    #[test]
    fn the_flush_lands_between_the_second_push_and_the_third() {
        let r = interrupt(0);
        let flush = position(&r, Step::Flush).expect("the routine flushes");
        let before = r.steps()[..flush]
            .iter()
            .filter(|s| **s == Step::Push)
            .count();
        assert_eq!(before, 2, "two words are on the stack when the queue goes");
    }

    /// A `Spend(0)` would be a step that occupies no T-state, which the
    /// sequencer would spin on rather than pass over.
    #[test]
    fn no_routine_spends_zero_clocks() {
        for (opcode, modrm, r) in every_routine() {
            assert!(
                !r.steps().contains(&Step::Spend(0)),
                "{opcode:02X}/{modrm:02X}: a zero-clock spend never retires"
            );
        }
    }

    /// A routine that points the stream at popped words must have popped them.
    /// A far transfer needs two, a near one.
    #[test]
    fn a_transfer_reads_words_the_routine_actually_popped() {
        for (opcode, modrm, r) in every_routine() {
            for far in [false, true] {
                let Some(at) = position(&r, Step::Transfer { far }) else {
                    continue;
                };
                let popped = r.steps()[..at].iter().filter(|s| **s == Step::Pop).count();
                let wanted = if far { 2 } else { 1 };
                assert_eq!(
                    popped,
                    wanted,
                    "{opcode:02X}/{modrm:02X}: a {} transfer wants {wanted} words, \
                     the routine popped {popped}",
                    if far { "far" } else { "near" },
                );
            }
        }
    }

    /// Every routine ends by running the instruction body or by flushing, and
    /// one that does neither would retire having computed nothing.
    #[test]
    fn every_routine_runs_or_flushes() {
        for (opcode, modrm, r) in every_routine() {
            assert!(
                position(&r, Step::Run).is_some() || position(&r, Step::Flush).is_some(),
                "{opcode:02X}/{modrm:02X}: a routine that neither runs nor flushes"
            );
        }
    }

    /// `SUSP` is only worth spending if the queue is thrown away afterwards:
    /// nothing else lifts it, so a routine that suspends and does not flush
    /// stops the prefetcher for good.
    #[test]
    fn every_suspend_is_followed_by_a_flush() {
        for (opcode, modrm, r) in every_routine() {
            let Some(susp) = position(&r, Step::Susp) else {
                continue;
            };
            assert!(
                r.steps()[susp..].contains(&Step::Flush),
                "{opcode:02X}/{modrm:02X}: SUSP with no flush behind it"
            );
        }
    }
}
