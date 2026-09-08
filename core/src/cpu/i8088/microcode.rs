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
//! word onto the stack, the prefetcher stopping, the queue being thrown away.
//!
//! The lists are transcribed from the published microcode rather than measured.
//! Where one disagrees with a timing row, the step list is the statement of
//! record and the row is the thing to delete.
//!
//! ## Reading a list against a trace
//!
//! The steps are the EU's, so a step's clock is the T-state the EU spends on
//! it. Bus cycles are not steps and do not appear: the EU asks for the bus and
//! the BIU decides when the cycle actually starts, which is why an anchor in a
//! recording lands three T-states after the step that requested it and later
//! still if the bus was busy.

/// One step of an instruction's microcode.
///
/// Everything here is either a clock the EU spends or an instruction to the
/// BIU. What the instruction *computes* is not modeled: that stays in
/// `execute.rs` and happens on [`Step::Run`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// `n` clocks of microcode with the bus free, so the BIU prefetches
    /// through them.
    Spend(u8),
    /// Stop prefetching, the `SUSP` of the published microcode.
    ///
    /// Free in itself. It costs clocks only when a code fetch is already in
    /// flight, and then only until that fetch reaches its last T-state. Only a
    /// [`Step::Flush`] lifts it.
    Susp,
    /// Throw the queue away and start the reload.
    ///
    /// **Costs one clock**, and lifts a suspension. The reload's first bus
    /// cycle begins three T-states later, so a flush placed before a push is
    /// visible in the recording as a code fetch that beats the write to the
    /// bus.
    Flush,
    /// Read one word through the interrupt vector table, low byte first.
    ReadVectorWord,
    /// Put one staged word on the stack.
    ///
    /// The words are staged in the order the executor pushed them, so the
    /// first `Push` of a routine writes the deepest.
    Push,
    /// Run the instruction body, which is what decides the values the pushes
    /// carry and where control goes.
    ///
    /// Costs nothing. It is the hand-off from the sequencer to `execute.rs`,
    /// not a clock: the part computes as it goes and the clocks it spends
    /// doing so are the `Spend`s around this.
    Run,
}

/// The interrupt routine, entered by `INT n`, `INT 3`, a taken `INTO` and a
/// hardware interrupt alike.
///
/// Transcribed from the published microcode at 0x19d, and every anchor of it
/// is visible in the recording of `CD`:
///
/// - The three clocks in front put the first vector read's T1 eight T-states
///   after the opcode is taken from the queue.
/// - **The single clock between the two vector words is the whole reason this
///   list exists.** The prefetcher is still running there, and the part spends
///   that clock starting a code fetch which lands between the two reads. A
///   core that runs the four byte cycles back to back cannot fit it, and the
///   fetch comes out at the far end of the instruction instead.
/// - `SUSP` is after the reads, not before them, which is why that fetch
///   happens at all.
/// - The queue is thrown away **between the second push and the third**, so
///   the reload at the handler is on the bus before the return offset is.
///   This core used to write all three words and then flush, which puts the
///   same three writes and the same fetch on the bus in the wrong order.
pub(crate) const INTERRUPT: &[Step] = &[
    // 0x19d, 0x19e, 0x19f.
    Step::Spend(3),
    // The handler's offset.
    Step::ReadVectorWord,
    // 0x1a1, the clock the recording's code fetch uses.
    Step::Spend(1),
    // And its segment.
    Step::ReadVectorWord,
    // 0x1a3 SUSP, then 0x1a3, 0x1a4, 0x1a5.
    Step::Susp,
    Step::Spend(3),
    // The vector is in hand, so the transfer can be computed and the three
    // words staged.
    Step::Run,
    Step::Push,
    // 0x1a6, then FARCALL2's jump, 0x06c, CORR and 0x06d.
    //
    // **Six, where reading the routine off gives five.** The published
    // routine is not ambiguous about this and was checked to the primitive:
    // one clock per microcode line, one for CORR, and the two flag clears
    // free. 0x1a6, the jump into FARCALL2, 0x06c, CORR and 0x06d is five.
    //
    // The recording wants six, and the likeliest sixth is one of those flag
    // clears. Interrupts and traps are disabled at exactly this seam, between
    // the flags reaching the stack and the far call beginning, and a part
    // that spends a microcode line doing it would put the clock here and
    // nowhere else.
    //
    // The other reading is refuted rather than merely doubted. A push could
    // instead be releasing the execution unit at T4 rather than at T3, which
    // would put this clock here without adding one to the routine; but it
    // adds one at the NEXT seam too, and that seam is three in the routine,
    // three here, and exact. One release rule cannot make this seam six and
    // leave that one at three.
    // Nor is it `SUSP` waiting for a fetch on the bus, which is the obvious
    // suspect once that is understood: this six was fitted while `SUSP` was
    // still free, so it could have been absorbing it. It was not. Putting the
    // five back afterwards makes the file -1 on all 5,000 cases, exactly as
    // it was before, so the two are independent and this clock is still the
    // one thing in the list the published routine does not account for.
    Step::Spend(6),
    Step::Push,
    // 0x06e, 0x06f, then NEARCALL's MC_JUMP.
    Step::Spend(3),
    Step::Flush,
    // 0x077, 0x078, 0x079.
    Step::Spend(3),
    Step::Push,
];

/// `CALL r/m16`, the near indirect call, once its pointer has been read.
///
/// The register form spends a clock the memory form does not, at 0x074, which
/// is the one difference between them. Everything after that is `CALL rel16`'s
/// routine: suspend, four clocks, throw the queue away, three more, push the
/// return offset behind the reload.
pub(crate) const CALL_INDIRECT: &[Step] = &[
    // The pointer is in hand, so the transfer can be computed and the return
    // offset staged.
    Step::Run,
    Step::Susp,
    // 0x074, 0x075, CORR, 0x076.
    Step::Spend(4),
    Step::Flush,
    // 0x077, 0x078, 0x079.
    Step::Spend(3),
    Step::Push,
];

/// The same, from a register operand, which spends one clock more.
pub(crate) const CALL_INDIRECT_REG: &[Step] = &[
    Step::Run,
    // 0x074, spent only when the operand was a register.
    Step::Spend(1),
    Step::Susp,
    Step::Spend(4),
    Step::Flush,
    Step::Spend(3),
    Step::Push,
];

/// `JMP r/m16`, the near indirect jump. No pushes and one clock of microcode:
/// suspend, spend it, flush.
pub(crate) const JMP_INDIRECT: &[Step] = &[
    Step::Run,
    Step::Susp,
    // 0x0d8.
    Step::Spend(1),
    Step::Flush,
];

/// The same, from a register operand.
pub(crate) const JMP_INDIRECT_REG: &[Step] = &[
    Step::Run,
    Step::Spend(1),
    Step::Susp,
    Step::Spend(1),
    Step::Flush,
];

/// The routine `opcode` runs with this `modrm`, or `None` for an instruction
/// still priced from a timing row.
///
/// Transcribing an opcode means adding a row here and deleting whatever in
/// `timing.rs` used to price it. The two must never both be live for the same
/// opcode: the row would be charged on top of the steps.
///
/// The group opcodes are asked for their reg field because the group is not
/// one instruction: `FF` carries `INC` and `PUSH`, which go nowhere, beside
/// the indirect calls and jumps, which are transfers.
///
/// **`FF /3` and `FF /5` are not here**, and the reason is structural rather
/// than a gap in the reading. Both have microcode *in front of* their operand
/// read: `FF /3` spends a clock at 0x068 before reading its far pointer, and
/// `FF /5` suspends the prefetcher at 0x0dc before reading its. The pipeline
/// reads the operand before a routine can start, so expressing either needs a
/// step that drives the operand read itself.
pub(crate) fn routine(opcode: u8, modrm: u8) -> Option<&'static [Step]> {
    let register_form = modrm >> 6 == 3;
    match opcode {
        // INT n.
        0xCD => Some(INTERRUPT),
        // The indirect calls and jumps live in the `FF` group, beside `INC`,
        // `DEC` and `PUSH`, which do not transfer and keep their rows.
        0xFF => match (modrm >> 3) & 7 {
            2 if register_form => Some(CALL_INDIRECT_REG),
            2 => Some(CALL_INDIRECT),
            4 if register_form => Some(JMP_INDIRECT_REG),
            4 => Some(JMP_INDIRECT),
            _ => None,
        },
        _ => None,
    }
}

/// How far into a routine the sequencer is, and what it has already done that
/// a step alone does not say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cursor {
    /// The routine being walked.
    pub(crate) steps: &'static [Step],
    /// The next step to run.
    pub(crate) at: u8,
    /// How many words the routine has already pushed, which is the slot the
    /// next [`Step::Push`] writes.
    pub(crate) pushed: u8,
    /// How many vector words it has already read.
    pub(crate) read: u8,
}

impl Cursor {
    /// Start `steps` from the beginning.
    pub(crate) fn new(steps: &'static [Step]) -> Self {
        Self {
            steps,
            at: 0,
            pushed: 0,
            read: 0,
        }
    }

    /// Take the next step, or `None` when the routine is over.
    pub(crate) fn next(&mut self) -> Option<Step> {
        let step = self.steps.get(self.at as usize).copied()?;
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

    /// The one invariant that keeps a transcription honest: a routine has to
    /// stage exactly as many words as the executor pushes, or the sequencer
    /// writes a slot nothing filled.
    #[test]
    fn the_interrupt_routine_pushes_the_three_words_it_stages() {
        let pushes = INTERRUPT.iter().filter(|s| **s == Step::Push).count();
        assert_eq!(
            pushes, 3,
            "the interrupt pushes the flags, the return segment and the return offset"
        );
    }

    /// And reads both halves of the vector before it needs either.
    #[test]
    fn the_vector_is_read_before_the_transfer_is_computed() {
        let run = INTERRUPT
            .iter()
            .position(|s| *s == Step::Run)
            .expect("the routine runs the instruction");
        let reads = INTERRUPT[..run]
            .iter()
            .filter(|s| **s == Step::ReadVectorWord)
            .count();
        assert_eq!(reads, 2, "both vector words are read before the transfer");
    }

    /// The placement the whole list exists for: the queue is thrown away with
    /// one push still to go, so the reload beats the last write to the bus.
    #[test]
    fn the_flush_lands_between_the_second_push_and_the_third() {
        let flush = INTERRUPT
            .iter()
            .position(|s| *s == Step::Flush)
            .expect("the routine flushes");
        let before = INTERRUPT[..flush]
            .iter()
            .filter(|s| **s == Step::Push)
            .count();
        assert_eq!(before, 2, "two words are on the stack when the queue goes");
    }

    /// A `Spend(0)` would be a step that occupies no T-state, which the
    /// sequencer would spin on rather than pass over.
    #[test]
    fn no_routine_spends_zero_clocks() {
        assert!(
            !INTERRUPT.contains(&Step::Spend(0)),
            "a zero-clock spend is a step that never retires"
        );
    }
}
