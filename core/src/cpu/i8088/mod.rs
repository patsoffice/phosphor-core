//! Intel 8088 CPU emulation.
//!
//! The 8088 is the 8-bit external data bus variant of the 8086. Internally it
//! operates on 16-bit data with a segmented 20-bit address space (1 MB).
//! Physical addresses are computed as `(segment << 4) + offset`, masked to
//! 20 bits.
//!
//! This implementation models the CPU at the instruction level: each call to
//! `execute_cycle` runs one bus cycle, with multi-cycle instructions tracked
//! via internal state. The bus interface uses `Address = u32` for 20-bit
//! physical addresses and `Data = u8` for the 8-bit external data bus.

pub(crate) mod access;
pub mod addressing;
pub mod alu;
pub mod decode;
pub mod execute;
pub mod flags;
pub(crate) mod format;
pub mod registers;
pub(crate) mod timing;

pub use registers::SegReg;

use crate::core::bus::InterruptState;
use crate::core::component::BusMasterComponent;
use crate::core::{Bus, BusMaster};
use crate::cpu::Cpu;
use crate::cpu::state::CpuStateTrait;
use crate::prelude::Saveable;

/// The longest byte sequence the loader can be asked to hold.
///
/// A real instruction is at most six bytes (opcode, ModR/M, two displacement,
/// two immediate, or the four-byte far pointer forms), and the 8088 accepts any
/// number of prefixes ahead of that. Four is more prefixes than any encoding
/// the test suite or any assembler produces, and the loader asserts rather than
/// overruns if that is ever wrong.
pub(crate) const MAX_INSTRUCTION: usize = 10;

/// Bytes the BIU's instruction queue holds. Four on the 8088; the 8086, with
/// twice the external bus, has six.
pub(crate) const QUEUE_LEN: usize = 4;

/// Where a fetch decided on an idle bus enters the address cycle.
///
/// A fetch chained off the end of another spends `Tr` inside the running cycle
/// and costs nothing for it. From idle there is nothing to hide `Tr` inside, so
/// the whole address cycle would be spent afterwards and T1 would land three
/// T-states after the decision rather than two.
///
/// It lands two, and that is not this core taking a shortcut. What the vectors
/// hand it is not idle in that sense: they install a queue rather than
/// prefetching into one, so the BIU arrives part way through a decision it never
/// took. The suite's own README states it as an observable rather than as a
/// design note: "It takes two cycles to begin a fetch after reading from a full
/// queue, therefore tests that specify an initial queue state will start with
/// two 'Ti' cycle states."
///
/// Entering at `Tr` instead was measured at 68.83% on cycle count against
/// 73.76% and 66.33% on bus-cycle order against 72.88%.
const IDLE_RESTART_FROM: TaCycle = TaCycle::Ts;

/// T-states between a bus request and the T1 it produces: `Tr`, `Ts`, `T0`.
///
/// This is how far ahead of its own T1 the EU claims the bus, and it is the
/// number that decides whether a prefetch fits in front of an operand access.
/// A request this far out takes the address cycle away from the prefetcher
/// before it can reach T1; a request any later leaves a code fetch in front of
/// the access that the part does not run. See [`TaCycle`].
const ADDRESS_CYCLE_CLOCKS: u8 = 3;

/// T-states in one bus cycle, T1 through T4.
///
/// The BIU cannot abandon one partway, which is why it matters whether a fetch
/// would finish before the EU comes for the bus. See [`I8088::eu_wants_bus`].
const BUS_CYCLE_CLOCKS: u8 = 4;

/// S0-S2: what kind of bus cycle the CPU is running.
///
/// The 8288 bus controller decodes these into the memory and I/O command lines.
/// `Inta`, `IoRead`, `IoWrite` and `Halt` are declared here because they are
/// what the pins can say; the EU does not drive them yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BusStatus {
    /// Interrupt acknowledge.
    Inta,
    /// I/O read.
    IoRead,
    /// I/O write.
    IoWrite,
    /// Memory read of data, as opposed to of an instruction.
    MemRead,
    /// Memory write.
    MemWrite,
    /// Halt acknowledge.
    Halt,
    /// Instruction fetch: a queue refill.
    Code,
    /// Passive. No bus cycle is in progress.
    #[default]
    Passive,
}

/// Which T-state of a bus cycle this is.
///
/// A bus cycle is T1 through T4, with wait states inserted between T3 and T4
/// when a device is not ready. `Idle` is the 8088's Ti: no bus cycle at all.
/// Nothing on the Gottlieb board inserts wait states and the test suite records
/// none, so `Wait` is declared for completeness and never produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TState {
    T1,
    T2,
    T3,
    T4,
    Wait,
    #[default]
    Idle,
}

/// What the CPU's bus pins are doing this T-state.
///
/// The address and the data share the same twenty pins, which is why each is an
/// `Option` here rather than a value that is sometimes stale. The address is
/// only on the pins during T1, while ALE is asserted for the external latch to
/// capture it, and the data only on T3. A caller that reads either on any other
/// cycle is reading pins that are mid-turnaround, and comparing that against a
/// recording produces failures that mean nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusPins {
    /// S0-S2.
    pub status: BusStatus,
    /// Which T-state of the bus cycle.
    pub t_state: TState,
    /// The 20-bit physical address, latched on T1 with ALE asserted.
    pub address: Option<u32>,
    /// The byte on the data pins, valid on T3.
    pub data: Option<u8>,
    /// S3/S4: which segment register computed the address.
    pub segment: Option<SegReg>,
}

/// What the EU did to the queue on a given cycle: the QS0/QS1 status lines.
///
/// The part reports these one cycle after the operation they describe. This
/// enum is what happened *now*; the delay is the reader's to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStatus {
    /// First byte of an instruction, or of one of its prefixes.
    First,
    /// A subsequent byte: ModR/M, displacement or immediate.
    Subsequent,
    /// The queue was flushed by a control transfer.
    Emptied,
}

/// The address cycle that runs in front of every bus cycle, and the reason a
/// bus cycle takes seven clocks where the datasheet draws four.
///
/// A bus cycle is documented as four T-states and it is not: **the physical
/// address is computed in the clocks before T1**, so that it can be on the pins
/// when T1 begins. Those clocks are their own little state machine, and it runs
/// *in parallel with whatever the bus is already doing*, which is why two bus
/// cycles back to back show no gap between one T4 and the next T1.
///
/// ```text
///   Tr   the request: something has decided it wants a bus cycle
///   Ts   the address is computed
///   T0   the address is ready, and this REPEATS until the bus is free
///   Td   no address cycle in progress
/// ```
///
/// **`T0` repeating is the structure this core spent an epic without.** A fetch
/// decided in the middle of somebody else's bus cycle does not begin four
/// T-states later regardless of what the bus is doing: it waits in `T0` and
/// issues on the clock after that cycle's T4. Every rule about *where* to take
/// the prefetch decision is fitted around this one, and six of them in a row
/// were tried and rejected here because without the hold each of them moved a
/// fetch to a place the part never puts one. With the hold, the decision point
/// stops mattering nearly as much: a decision taken early simply waits.
///
/// It is also what makes an idle restart cost three clocks and a chained fetch
/// none. From idle the whole of `Tr`, `Ts`, `T0` has to be spent after the
/// event that triggered it, so a queue read that frees a slot puts T1 three
/// T-states later. A fetch decided at the end of T2 spends `Ts` in T3 and `T0`
/// in T4 and issues immediately after.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TaCycle {
    /// The request, on the clock the decision is taken.
    Tr,
    /// The address is computed.
    Ts,
    /// The address is ready and the cycle is waiting for the bus. Repeats.
    T0,
    /// Nothing scheduled.
    #[default]
    Td,
}

/// Why the BIU is not prefetching right now.
///
/// `Normal` is not "fetching": it is "nothing is stopping it", and whether a
/// fetch is actually in flight is [`Biu`]'s business. This is the reason the
/// decision came back negative, kept because it says what event lifts it: the
/// queue being full is lifted by the EU taking a byte out, and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FetchState {
    #[default]
    Normal,
    /// The queue is full. Lifted when the EU takes a byte out.
    PausedFull,
    /// A control transfer's microcode has stopped prefetching, because the
    /// bytes behind it are on the path not taken and fetching more of them is
    /// wasted bus. Lifted only by the flush at the end of that microcode.
    ///
    /// This is the part's `SUSP`, and it is the first or second step of every
    /// transfer's microcode. A fetch already in flight is not abandoned: `SUSP`
    /// waits for it, which is why a transfer entered while the queue is
    /// refilling costs more than one entered on an idle bus.
    Suspended,
}

/// The bus interface unit's code-fetch state machine, which runs alongside the
/// EU and independently of it.
///
/// This is only the four documented T-states. The clocks in front of T1 are
/// [`TaCycle`], and they are not here because they are not exclusive to the
/// prefetcher and not exclusive to the bus: an address cycle overlaps the bus
/// cycle before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Biu {
    /// Not driving the bus.
    #[default]
    Idle,
    /// The address is latched and T1 is driven on the next clock. This is one
    /// real T-state: it is the clock the address cycle ends on, and the bus is
    /// already claimed even though no status has gone out yet.
    Starting { addr: u32 },
    /// Driving T-state `t` of a CODE bus cycle on *this* clock, `addr` being
    /// what went out on T1. The address is held here rather than recomputed,
    /// because the EU consuming bytes while a fetch is in flight must not move
    /// an address the BIU has already put on the pins.
    ///
    /// `byte` is what the addressed device drove on T3, held until T4, which is
    /// when it joins the queue.
    Fetching { t: u8, addr: u32, byte: u8 },
}

/// What the execution unit is doing, at the granularity of whole bus cycles.
///
/// An instruction with a memory operand is three phases, and they have to be
/// three because the middle one cannot start until the first has finished and
/// the last cannot start until the middle has decided what to write:
///
/// ```text
/// Loading -> AddressCalc -> Reading (MEMR) -> execute -> Writing (MEMW)
/// ```
///
/// `execute` itself is still one indivisible step. What has moved out of it is
/// the bus traffic on either side, and the address arithmetic in front of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Eu {
    /// Taking instruction bytes out of the queue.
    #[default]
    Loading,
    /// Adding up an effective address, with the clocks still to go.
    ///
    /// The bus is free during this, so the BIU keeps prefetching through it.
    /// That is not a detail: an addressing mode that takes twelve clocks to
    /// compute is twelve clocks in which the queue refills, which is why a
    /// complicated address can be nearly free on a part that would otherwise
    /// have been waiting for instruction bytes.
    AddressCalc(u8),
    /// Reading the memory operand before the instruction runs.
    Reading {
        /// Which byte of the operand, counting from zero.
        byte: u8,
        /// How many there are: 1, 2, or 4 for a far pointer.
        total: u8,
        /// T-state within the current bus cycle.
        t: u8,
    },
    /// The microcode a pop runs *before* its first read reaches the bus, with
    /// the clocks still to go.
    ///
    /// **A bus cycle's position inside an instruction is not a consequence of
    /// its total.** `POP AX` from a full queue takes twelve clocks here and
    /// twelve on the part, and this core drove the read's T1 three T-states
    /// before the part did, every case of every file in the family.
    /// `fetch_gap_diff` measures it, and `side_by_side` shows the whole span
    /// shifted by those three: what looked like a code fetch this core runs and
    /// the part does not is that shift arriving at the window's end.
    ///
    /// These clocks come out of [`Eu::Executing`]'s, so the total is fixed by
    /// construction and only the read moves. That is the whole shape of the
    /// remaining work: the part's microcode requests the bus at a point within
    /// itself, and a phase machine that always requests it first cannot express
    /// where.
    StackLeadIn(u8),
    /// Reading words off the stack before the instruction runs, `word` of
    /// `total`, `byte` of that word's two, on T-state `t`.
    PoppingStack {
        word: u8,
        total: u8,
        byte: u8,
        t: u8,
    },
    /// Writing words onto the stack after it has run.
    PushingStack {
        word: u8,
        total: u8,
        byte: u8,
        t: u8,
    },
    /// Reading the four bytes of an interrupt vector out of the table at the
    /// bottom of memory, `byte` of four, on T-state `t`.
    ///
    /// Ahead of the pushes, which is the order the recording shows: `INT 3`
    /// reads 0000C through 0000F and only then writes the three words onto the
    /// stack.
    ReadingVector { byte: u8, t: u8 },
    /// A `REP` prefix's setup, before the first iteration reaches the bus.
    StringEntry(u8),
    /// One access of a string operation's current iteration: which of the
    /// three, `byte` of the one or two it moves, on T-state `t`.
    StringAccess { part: StringPart, byte: u8, t: u8 },
    /// The microcode time of a string iteration, with the clocks still to go.
    /// Ends with either another iteration or the end of the instruction.
    StringDelay(u8),
    /// Acknowledging a maskable interrupt: two INTA bus cycles, `cycle` being
    /// 0 or 1 and `t` the T-state within it.
    ///
    /// The part runs two rather than one, and the interrupting device puts the
    /// vector number on the data pins during the second. Here the board has
    /// already supplied that number through `InterruptState`, so these cycles
    /// carry no information this core needs; they are driven because a device
    /// watching the bus can see them, and because the interrupt costs the eight
    /// clocks they take.
    Acknowledging { cycle: u8, t: u8 },
    /// Reading an I/O port, `byte` of `total`, on T-state `t`. IOR rather than
    /// MEMR, and after the microcode rather than before it: the recording puts
    /// `IN AL, imm8`'s port cycle four clocks after its last instruction byte.
    PortReading { byte: u8, total: u8, t: u8 },
    /// Writing an I/O port, after the instruction has decided what to write.
    PortWriting { byte: u8, total: u8, t: u8 },
    /// Running the instruction's microcode, with the clocks still to go.
    ///
    /// The bus is free throughout, so the BIU prefetches through it. That is
    /// what the recorded traces show the hardware doing between an operand read
    /// and its write-back, and it is why this phase sits where it does.
    Executing(u8),
    /// Writing the memory operand back after the instruction has run.
    Writing { byte: u8, total: u8, t: u8 },
}

/// Which of a string iteration's three possible accesses is happening.
///
/// In this order, which is the order the recording shows: `CMPS` reads its
/// source and then its destination, and `MOVS` reads its source and then writes
/// its destination, with prefetches falling in between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StringPart {
    /// `[DS:SI]`, for `MOVS`, `CMPS` and `LODS`.
    Source,
    /// `[ES:DI]` read, for `CMPS` and `SCAS`.
    Destination,
    /// `[ES:DI]` written, for `MOVS` and `STOS`.
    Write,
}

/// An interrupt the pipeline is servicing in place of an instruction.
///
/// A hardware interrupt is not an instruction and does not pretend to be one
/// here: nothing is loaded, no queue byte is read, and the phases that would
/// look at an opcode ask this instead. What it shares with `INT n` is
/// everything after the vector number is known, which is why it runs through
/// the same vector read and the same three pushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Servicing {
    /// The interrupt vector to take.
    pub vector: u8,
    /// Whether the bus runs an acknowledge pair first. A maskable interrupt
    /// acknowledges; NMI does not, because nothing has to tell the part which
    /// vector it is.
    pub acknowledge: bool,
}

/// Which part of the instruction the loader's next fetched byte belongs to.
///
/// This is the shape of an 8088 instruction read left to right, and the loader
/// walks it once per instruction. Only the opcode's position is known in
/// advance: whether a ModR/M byte follows comes from the opcode, how long the
/// displacement is comes from the ModR/M byte, and for two opcode groups
/// whether there is an immediate at all comes from the ModR/M byte too. See
/// [`format`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Stage {
    /// The opcode, or one of the prefixes ahead of it. The loader stays here
    /// for as long as the bytes arriving are prefixes.
    #[default]
    Opcode,
    /// The ModR/M byte.
    Modrm,
    /// Displacement bytes, with the number still to fetch.
    Displacement(u8),
    /// Immediate bytes, with the number still to fetch.
    Immediate(u8),
}

/// REP/REPZ/REPNZ prefix state.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RepPrefix {
    Rep,   // REP (MOVS/STOS/LODS/INS/OUTS) or REPZ (CMPS/SCAS)
    Repnz, // REPNZ (CMPS/SCAS)
}

/// Interrupt type for the 8088 interrupt response sequence.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub(crate) enum InterruptType {
    /// Non-maskable interrupt (vector 2)
    Nmi = 0,
    /// Maskable hardware interrupt (vector from PIC)
    Irq = 1,
    /// Software interrupt (INT n instruction)
    Software = 2,
}

/// Fields are ordered to match the save-state serialization layout (version 1).
#[derive(Saveable)]
#[save_version(1)]
pub struct I8088 {
    // General-purpose registers (accessible as 16-bit or 8-bit halves)
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,

    // Index registers
    pub si: u16,
    pub di: u16,

    // Pointer registers
    pub bp: u16,
    pub sp: u16,

    // Segment registers
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,

    // Instruction pointer
    pub ip: u16,

    // FLAGS register (16-bit, with always-one bits)
    pub flags: u16,

    // Interrupt state (nmi_prev before nmi_pending to match save-state order)
    pub(crate) nmi_prev: bool,
    pub(crate) nmi_pending: bool,

    /// Halted by HLT, waiting for an interrupt.
    #[save_skip(default)]
    pub(crate) halted: bool,

    // -- Loader state ------------------------------------------------------
    //
    // None of this is serialized, and it does not need to be, because the
    // loader is restartable: IP does not move until the *executor* consumes a
    // byte out of `instr`, so a partially loaded instruction can be thrown away
    // and refetched from CS:IP with no observable difference except the cycles
    // spent. That is what makes a save state taken mid-instruction safe here
    // rather than merely tolerated.
    /// Instruction bytes fetched so far, prefixes included.
    #[save_skip(default = [0; MAX_INSTRUCTION])]
    pub(crate) instr: [u8; MAX_INSTRUCTION],
    /// How many bytes of `instr` the loader has fetched.
    #[save_skip(default)]
    pub(crate) instr_len: u8,
    /// How many of those the executor has consumed.
    #[save_skip(default)]
    pub(crate) instr_pos: u8,
    /// Where in `instr` the opcode byte sits, past any prefixes.
    #[save_skip(default)]
    pub(crate) opcode_at: u8,
    /// Which part of the instruction the loader is fetching.
    #[save_skip(default)]
    pub(crate) stage: Stage,
    /// Which phase of an instruction the execution unit is in.
    #[save_skip(default)]
    pub(crate) eu: Eu,
    /// The memory operand this instruction addresses, once resolved, as
    /// segment and offset. `None` for a register operand or no operand at all,
    /// which is also the case where no bus cycle is owed.
    #[save_skip(default)]
    pub(crate) operand_at: Option<(u16, u16)>,
    /// The operand's bytes, low first: read into here before the instruction
    /// runs, and written out of here after. Four bytes because a far pointer is
    /// the widest operand there is.
    #[save_skip(default = [0; 4])]
    pub(crate) operand_bytes: [u8; 4],
    /// Set when the instruction wrote its memory operand, so the write-back
    /// phase knows there is something to do.
    #[save_skip(default)]
    pub(crate) operand_written: bool,
    /// Stack words in flight: read into here before the instruction runs, or
    /// staged into it by `push16` for the pipeline to write after.
    ///
    /// Three is the deepest any instruction goes, and it is an interrupt
    /// pushing flags, segment and offset.
    #[save_skip(default = [0; 3])]
    pub(crate) stack_words: [u16; 3],
    /// Where in `stack_words` the executor's next pop or push lands.
    #[save_skip(default)]
    pub(crate) stack_pos: u8,
    /// Where the current string iteration reads and writes: the source
    /// `[DS:SI]` and the destination `[ES:DI]`, as they stood when the
    /// iteration began.
    ///
    /// Taken once per iteration rather than read off SI and DI at each access,
    /// because the iteration steps both registers and some of its bus cycles
    /// come after that. `MOVS` was writing to the address after the one it
    /// should have, and `STOS` likewise, for exactly that reason.
    #[save_skip(default)]
    pub(crate) string_at: [(u16, u16); 2],
    /// The interrupt the pipeline is servicing, if it is servicing one rather
    /// than running an instruction. See [`Servicing`].
    #[save_skip(default)]
    pub(crate) servicing: Option<Servicing>,
    /// The bytes an `IN` read from its port, or an `OUT` is about to write,
    /// low byte first, and whether the instruction produced any.
    ///
    /// Kept apart from `operand_bytes` rather than sharing it: an I/O access is
    /// not a ModR/M operand, it is not described by [`access::operand_access`],
    /// and it is not covered by the cross-check that keeps that table honest.
    /// Sharing the buffer would make the two look like one thing to a reader
    /// and to that assertion.
    #[save_skip(default = [0; 2])]
    pub(crate) port_bytes: [u8; 2],
    #[save_skip(default)]
    pub(crate) port_written: bool,
    /// An immediate that belongs to an instruction with a memory operand, and
    /// which the loader has therefore not fetched yet.
    ///
    /// The part does not fetch it before the operand access. `ADD [BX+SI], imm`
    /// starts its operand read on exactly the cycle `MOV reg, [BX+SI]` does,
    /// though it is two bytes longer, and the recording shows the immediate
    /// arriving in the queue afterwards. Fetching it early cost this core those
    /// two clocks before every such read, and a queue stall on top wherever the
    /// instruction ran past the four bytes the queue holds.
    ///
    /// `deferred` is set when the loader stops short of the immediate;
    /// `resuming` while it goes back for it once the operand access is done.
    #[save_skip(default)]
    pub(crate) immediate_deferred: bool,
    #[save_skip(default)]
    pub(crate) immediate_resuming: bool,
    /// The interrupt vector the pipeline read for this instruction, offset then
    /// segment, and whether it read one at all.
    ///
    /// Set for `INT`, `INT 3` and a taken `INTO`, whose vector number is known
    /// before the instruction runs. Not set for the interrupts an instruction
    /// takes only when it faults: `DIV`, `IDIV` and `AAM` read their vector
    /// from inside the executor, off the bus entirely, and their timing rows
    /// carry those sixteen clocks instead.
    #[save_skip(default)]
    pub(crate) vector_words: (u16, u16),
    #[save_skip(default)]
    pub(crate) vector_staged: bool,
    /// Whether the pipeline is handling this instruction's stack traffic.
    ///
    /// False for the conditional cases a fixed count cannot predict, where
    /// `push16` and `pop16` fall back to reaching the bus directly.
    #[save_skip(default)]
    pub(crate) stack_staged: bool,
    /// The stack pointer as it stood before the pushes were staged, which is
    /// where the pipeline starts writing them.
    #[save_skip(default)]
    pub(crate) stack_base: u16,
    /// What the executor actually did to its ModR/M operand while running the
    /// current instruction, as (reads, writes).
    ///
    /// This exists to keep [`access`] honest. That table is a second statement
    /// of something `execute.rs` already knows implicitly, and the pair is
    /// exactly the shape that drifts apart silently, so the table is checked
    /// against what the executor did rather than trusted. Written in release
    /// too, because a pair of counter bumps is cheaper than two code paths.
    #[save_skip(default)]
    pub(crate) operand_ops: (u8, u8),
    /// What the executor actually did to the stack while running the current
    /// instruction, as (pops, pushes). Keeps [`access::stack_access`] honest
    /// the same way `operand_ops` keeps the operand table honest.
    #[save_skip(default)]
    pub(crate) stack_ops: (u8, u8),
    /// Set for the one T-state on which an instruction retires.
    #[save_skip(default)]
    pub(crate) retired: bool,
    /// This instruction transferred control, so the queue holds bytes from the
    /// path not taken. Set by [`I8088::set_ip`] and [`I8088::set_cs`], cleared
    /// when the instruction retires.
    #[save_skip(default)]
    pub(crate) transferred: bool,
    /// A control transfer has run and the queue must be flushed on the next
    /// T-state.
    ///
    /// It cannot be flushed on the same one. The part reports a single queue
    /// operation per cycle, and a taken branch produces two: the read of the
    /// last byte of the branch instruction, and then the flush. They have to be
    /// in that order because the branch is not resolved until that byte is in
    /// hand. The recorded traces show it directly: a taken `JO` reports
    /// `F` then `S` then `E` on three separate cycles.
    #[save_skip(default)]
    pub(crate) pending_flush: bool,

    // -- BIU and prefetch queue --------------------------------------------
    /// The instruction queue, oldest byte first.
    #[save_skip(default = [0; QUEUE_LEN])]
    pub(crate) queue: [u8; QUEUE_LEN],
    /// How many bytes of `queue` are live.
    #[save_skip(default)]
    pub(crate) queue_len: u8,
    /// The address the BIU will fetch next.
    ///
    /// On the part this *is* IP, and the architectural IP is computed by
    /// subtracting the queue length when something needs it. Here it is the
    /// other way round, because the value the test vectors report as `ip` is
    /// the architectural one: `ip` trails, and this runs ahead of it by however
    /// many bytes are queued or already loaded.
    #[save_skip(default)]
    pub(crate) prefetch_ip: u16,
    /// The BIU's own state machine, independent of the EU's.
    #[save_skip(default)]
    pub(crate) biu: Biu,
    /// A bus cycle's T4 that the execution unit has already walked away from.
    ///
    /// **A write does not hold the EU until T4.** The data is on the pins at
    /// T3 and there is nothing left for the EU to wait for, so its microcode
    /// moves on and the next instruction's first byte is taken from the queue
    /// on the very T-state that finishes the write. The bus cycle still has to
    /// end, and this carries the T4 that goes out while the EU is elsewhere.
    ///
    /// The recording says so without ambiguity. `ADD word [DS:DI+4611h],
    /// BF61h` from a full queue puts every one of its nine bus cycles on the
    /// T-state this core does, and then reads the next first byte on the last
    /// write's T4 where this core read it on the T-state after. That one clock
    /// is the whole difference on 140,000 cases of the override population
    /// alone.
    #[save_skip(default)]
    pub(crate) eu_tail: Option<(BusStatus, SegReg)>,
    /// The address cycle in front of the next code fetch. See [`TaCycle`].
    #[save_skip(default)]
    pub(crate) ta: TaCycle,
    /// Why the BIU is not prefetching, when it is not. See [`FetchState`].
    #[save_skip(default)]
    pub(crate) fetch: FetchState,
    /// T-states the loader still owes before it may take its next byte, from
    /// [`timing::loader_stall`]. Decode time, not a queue wait: it is spent
    /// whether or not there is a byte waiting.
    #[save_skip(default)]
    pub(crate) loader_stall: u8,
    /// T-states this instruction's loader has spent with an empty queue, since
    /// its first byte. A pause taken inside one of these cost nothing, so it is
    /// not charged back. See [`I8088::begin_execute_phase`].
    #[save_skip(default)]
    pub(crate) loader_starved: u8,
    /// What the EU did to the queue on this T-state, cleared at the start of
    /// each one. These are the QS0/QS1 status lines, which the part exposes for
    /// exactly this reason: an outside observer cannot otherwise tell where one
    /// instruction ends and the next begins.
    #[save_skip(default)]
    pub queue_status: Option<(QueueStatus, u8)>,
    /// What the bus pins are doing this T-state, rewritten at the start of each
    /// one. This is the other half of what an outside observer can see, and the
    /// half the recorded vectors devote eight of their eleven fields to.
    #[save_skip(default)]
    pub bus: BusPins,

    #[save_skip(default)]
    pub(crate) segment_override: Option<SegReg>,
    #[save_skip(default)]
    pub(crate) rep_prefix: Option<RepPrefix>,
    #[save_skip(default)]
    pub(crate) irq_line: bool,
    /// Total T-states executed. Not serialized; keeps its current value.
    #[save_skip]
    pub(crate) clock: u64,
}

impl Default for I8088 {
    fn default() -> Self {
        Self::new()
    }
}

impl I8088 {
    pub fn new() -> Self {
        Self {
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            bp: 0,
            sp: 0,
            // Reset state: CS=0xFFFF, all others 0
            cs: 0xFFFF,
            ds: 0,
            es: 0,
            ss: 0,
            ip: 0,
            flags: flags::normalize(0),
            halted: false,
            instr: [0; MAX_INSTRUCTION],
            instr_len: 0,
            instr_pos: 0,
            opcode_at: 0,
            stage: Stage::Opcode,
            eu: Eu::Loading,
            operand_at: None,
            operand_bytes: [0; 4],
            operand_written: false,
            stack_words: [0; 3],
            stack_pos: 0,
            string_at: [(0, 0); 2],
            servicing: None,
            port_bytes: [0; 2],
            port_written: false,
            immediate_deferred: false,
            immediate_resuming: false,
            vector_words: (0, 0),
            vector_staged: false,
            stack_staged: false,
            stack_base: 0,
            operand_ops: (0, 0),
            stack_ops: (0, 0),
            retired: false,
            transferred: false,
            pending_flush: false,
            queue: [0; QUEUE_LEN],
            loader_stall: 0,
            loader_starved: 0,
            queue_len: 0,
            prefetch_ip: 0,
            biu: Biu::Idle,
            eu_tail: None,
            ta: TaCycle::Td,
            fetch: FetchState::Normal,
            queue_status: None,
            bus: BusPins::default(),
            segment_override: None,
            rep_prefix: None,
            nmi_pending: false,
            nmi_prev: false,
            irq_line: false,
            clock: 0,
        }
    }

    /// Returns true when the CPU is between instructions: nothing loaded, so
    /// the next byte the EU takes from the queue will be a First Byte.
    ///
    /// This is not the same as "an instruction just retired": the EU can sit
    /// here for many cycles with an empty queue, waiting for the BIU. Use
    /// [`Self::retired`](I8088::retired) for the edge.
    pub fn at_instruction_boundary(&self) -> bool {
        !self.halted && self.instr_len == 0
    }

    /// Bytes currently in the prefetch queue, out of [`QUEUE_LEN`].
    ///
    /// For diagnostics rather than for emulation: whether the BIU is idle
    /// because it has nothing to do or because the queue is full is the
    /// difference between two very different bugs, and a bus-cycle trace cannot
    /// tell them apart on its own.
    pub fn queue_len(&self) -> usize {
        self.queue_len as usize
    }

    /// Total T-states executed since creation.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Execute one T-state.
    ///
    /// A T-state is one CPU clock, so a board clocking this at 5 MHz calls this
    /// five million times per emulated second.
    ///
    /// Two things run here, and their independence is the whole point of the
    /// design. The EU takes instruction bytes out of the queue, one per
    /// T-state, stalling when the queue is empty. The BIU refills the queue
    /// whenever there is room, through four-T-state CODE bus cycles the EU
    /// knows nothing about. Neither waits on the other except through the
    /// queue, which is why an instruction that arrives prefetched costs no bus
    /// cycles of its own.
    ///
    /// The EU runs first, so a byte the BIU latches on this cycle's T3 is
    /// available to the EU on the next cycle rather than this one.
    ///
    /// What is *not* yet per-cycle: the instruction's own execution, including
    /// its operand reads and writes and the cycles the EU spends computing an
    /// effective address, still happens atomically on the T-state that its last
    /// byte arrives. Until that lands this core undercounts every instruction
    /// that touches memory. See `docs/designs/cycle-accurate-i8088.md`.
    pub fn execute_cycle<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.clock += 1;
        self.queue_status = None;
        self.retired = false;
        // Pins default to passive and idle every cycle, so a cycle that drives
        // nothing reads as Ti rather than as whatever the last bus cycle left
        // behind. Holding stale values here is how an address gets compared on
        // a cycle that is not carrying one.
        self.bus = BusPins::default();

        // A write the EU has already finished with still has a T4 to drive, and
        // it goes out here rather than from a phase, because the EU has moved on
        // and is reading the next instruction's first byte on this very
        // T-state. See [`I8088::eu_tail`].
        if let Some((status, segment)) = self.eu_tail.take() {
            self.drive_bus_cycle(status, TState::T4, segment);
        }

        // A branch taken on the previous T-state flushes here, on its own
        // cycle, and only then has the instruction retired.
        if self.pending_flush {
            self.pending_flush = false;
            self.flush_queue();
            self.retired = true;
            // The request for the reload is made on the flush clock itself, so
            // the bus still runs here: `Tr` is spent now, `Ts` and `T0` on the
            // two after it, and the reload's T1 lands three clocks past the
            // flush.
            self.tick_bus(bus, master);
            return;
        }

        if self.halted && self.servicing.is_none() {
            // A halted 8088 stops prefetching, so the BIU does not run here. An
            // interrupt is what gets it going again, and it runs the same
            // sequence it would have run at an instruction boundary.
            let ints = bus.check_interrupts(master);
            if self.begin_interrupt(ints) {
                self.halted = false;
            }
            return;
        }

        // Interrupts are recognized between instructions, which is the only
        // point the queue can be redirected without discarding a partial fetch.
        // Recognizing one costs this T-state; the acknowledge, the vector read
        // and the pushes follow on the ones after it.
        if self.servicing.is_none() && self.at_instruction_boundary() {
            let ints = bus.check_interrupts(master);
            if self.begin_interrupt(ints) {
                return;
            }
        }

        // There is one bus. The BIU holds its address cycle in `T0` whenever the
        // EU is a clock away from wanting it, so it should never have taken the
        // bus from under an EU phase waiting to drive T1: this is the safety net
        // for the case where it did anyway, and a fetch in flight is not
        // abandoned. The EU spends the T-state waiting, which is what the part
        // does.
        if self.eu_bus_t_state() == Some(1) && !self.bus_free_for_eu() {
            self.tick_bus(bus, master);
            return;
        }

        match self.eu {
            Eu::Loading => self.tick_eu(bus, master),
            // Address arithmetic uses no bus, so the BIU runs alongside it, and
            // only stays off it for the address cycle at the end. Suppressing
            // prefetching for the whole address phase was measured and is
            // wrong: it took the prefetched half of the bus order from 67.09%
            // down to 53.98%, because the part does slip prefetches into an
            // address phase with room for them.
            // See [`I8088::eu_wants_bus`].
            Eu::AddressCalc(remaining) => {
                self.eu = if remaining > 1 {
                    Eu::AddressCalc(remaining - 1)
                } else {
                    self.begin_operand_phase()
                };
                // An instruction that neither reads its operand nor has any
                // modeled execution time runs here. Dropping this made the
                // pipeline fall back to Loading without ever executing, so the
                // loader started a fresh instruction on top of the old one.
                self.execute_if_ready(bus, master);
            }
            // Nor does microcode. This is the phase the hardware traces show
            // the BIU prefetching through.
            Eu::Executing(remaining) => {
                if remaining > 1 {
                    self.eu = Eu::Executing(remaining - 1);
                } else if let Some(acc) = access::port_access(self.opcode())
                    && acc.reads
                {
                    // An `IN` reads its port after the microcode and before the
                    // instruction runs, which is where the recording puts it:
                    // four clocks after the last instruction byte, not on it.
                    self.eu = Eu::PortReading {
                        byte: 0,
                        total: acc.width.bytes(),
                        t: 1,
                    };
                } else {
                    self.eu = Eu::Loading;
                    self.run_execute_step(bus, master);
                }
            }
            // There is one bus, and while the EU is using it the BIU cannot
            // prefetch. That contention is not incidental: it is why an
            // instruction with a memory operand leaves the queue emptier than
            // one without, and why the instruction after it may then stall.
            Eu::Reading { .. } => self.tick_operand_read(bus, master),
            // A string operation's own clocks leave the bus free, so the BIU
            // prefetches through them, as it does through any microcode. No
            // claim is made across the entry, and none should be until the
            // entry itself is measured: the recording fetches twice before
            // `SCASB`'s first read where this core fetches once, so the part is
            // still in its own microcode where this core has already resolved an
            // address, and claiming the bus there would suppress a fetch the
            // part does run.
            Eu::StringEntry(remaining) => {
                self.eu = if remaining > 1 {
                    Eu::StringEntry(remaining - 1)
                } else {
                    self.begin_string_iteration()
                };
            }
            Eu::StringDelay(_) => self.tick_string_delay(bus, master),
            Eu::StringAccess { .. } => self.tick_string(bus, master),
            Eu::Acknowledging { .. } => self.tick_acknowledge(),
            Eu::ReadingVector { .. } => self.tick_vector_read(bus, master),
            Eu::PortReading { .. } => self.tick_port(bus, master, true),
            Eu::PortWriting { .. } => self.tick_port(bus, master, false),
            Eu::Writing { .. } => self.tick_operand_write(bus, master),
            // Microcode, so the BIU runs alongside it exactly as it does
            // through [`Eu::Executing`].
            Eu::StackLeadIn(remaining) => {
                let stack =
                    access::stack_access(self.opcode(), self.instr[self.opcode_at as usize + 1]);
                self.eu = if remaining > 1 {
                    Eu::StackLeadIn(remaining - 1)
                } else {
                    Eu::PoppingStack {
                        word: 0,
                        total: stack.pops,
                        byte: 0,
                        t: 1,
                    }
                };
                // The BIU prefetches through this, and holding it off does not
                // pay: the recorded `POP AX` from a full queue is idle here,
                // but suppressing the fetch across the corpus takes the
                // bus-cycle order from 72.88% to 72.17%. The part slips one in
                // wherever the queue is emptier than that case's.
            }
            Eu::PoppingStack { .. } => self.tick_stack(bus, master, true),
            Eu::PushingStack { .. } => self.tick_stack(bus, master, false),
        }

        self.tick_bus(bus, master);
    }

    /// The execution unit's cycle: take one byte from the queue, if there is
    /// one and the EU still wants one.
    fn tick_eu<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        // Waiting for an instruction to begin is not this instruction's wait.
        if self.instr_len == 0 {
            self.loader_starved = 0;
        }

        // Decoding, not waiting. The part's loader stops for a T-state at points
        // that depend on the opcode, and it does so with bytes sitting in the
        // queue, so this is spent before the queue is even asked. See
        // [`timing::loader_stall`].
        if self.loader_stall > 0 {
            self.loader_stall -= 1;
            return;
        }

        if self.queue_len == 0 {
            // Starved. The EU idles until the BIU delivers, which is the cost
            // the prefetch queue exists to avoid and the reason a jump is
            // expensive.
            self.loader_starved = self.loader_starved.saturating_add(1);
            return;
        }

        let stage_before = self.stage;
        let byte = self.pop_queue();
        // A prefix reads as a First Byte, and so does the opcode behind it. The
        // suite's README is explicit: an instruction's first byte "may be an
        // optional instruction prefix, in which case there will be multiple
        // First Byte statuses until the first byte that is a non-prefixed
        // opcode byte is read". The loader's opcode stage is exactly that span,
        // so the stage is the status.
        self.queue_status = Some((
            if self.stage == Stage::Opcode {
                QueueStatus::First
            } else {
                QueueStatus::Subsequent
            },
            byte,
        ));

        assert!(
            (self.instr_len as usize) < MAX_INSTRUCTION,
            "instruction longer than {MAX_INSTRUCTION} bytes at {:04X}:{:04X}",
            self.cs,
            self.ip
        );
        self.instr[self.instr_len as usize] = byte;
        self.instr_len += 1;

        let was_prefix = stage_before == Stage::Opcode && decode::decode_prefix(byte).is_some();
        let complete = self.advance_stage();
        if !complete {
            // The pause belongs to the byte just read, so it is charged only
            // when another byte of this instruction is still to come.
            let modrm = self.instr[self.opcode_at as usize + 1];
            self.loader_stall = match timing::loader_stall(self.opcode(), modrm) {
                timing::LoaderStall::AfterOpcode(n)
                    if stage_before == Stage::Opcode && self.stage != Stage::Opcode =>
                {
                    n
                }
                timing::LoaderStall::BeforeDisplacement(n) if stage_before == Stage::Modrm => n,
                timing::LoaderStall::BeforeImmediate(n) if stage_before == Stage::Modrm => n,
                _ => 0,
            };
        }
        // **A prefix costs a T-state of its own, after the byte is read.** The
        // recording puts it beyond argument: `3E 8B 3D` from a full queue reads
        // the override on the opening T-state, nothing on the next, and the
        // opcode on the one after, on every case of the file. This core read the
        // opcode immediately and paid the same clock at the far end, as
        // microcode, which is where the manual's second clock per prefix had
        // been going. The total was right and every queue read and every fetch
        // behind it was a T-state early, which the bus schedule then compensated
        // for; when the bus stopped compensating, 140,000 cases of the override
        // population were `+1` for this alone.
        //
        // It is not charged back to the microcode, because it *is* the
        // microcode: the second of the prefix's two documented clocks, moved to
        // the side of the read that it happens on. See
        // [`I8088::begin_execute_phase`], which no longer adds it there.
        if was_prefix {
            self.loader_stall = timing::PREFIX_PAUSE;
        }
        if complete {
            if self.immediate_resuming {
                // The loader has just gone back for the immediate of an
                // instruction whose operand access is already done, so the
                // pipeline picks up where it left off rather than starting the
                // instruction again.
                self.immediate_resuming = false;
                self.eu = self.begin_pre_execute_phase();
                self.execute_if_ready(bus, master);
            } else {
                self.run_loaded_instruction(bus, master);
            }
        }
    }

    /// Whether this instruction's operand is addressed in memory at all,
    /// whether or not it is read or written.
    ///
    /// The loader's queue-length correction asks this rather than
    /// [`I8088::operand_reaches_memory`], and the difference is `LEA`: it has a
    /// memory addressing mode, spends the effective-address clocks, and runs no
    /// bus cycle. Its timing is the address phase's rather than the loader's,
    /// so this predicate counts it and the correction leaves it alone.
    ///
    /// `A0`-`A3` are the control in the other direction. They are four bytes
    /// long under a segment override and exact, because they reach memory too,
    /// and they carry their address in the instruction rather than in a ModR/M
    /// byte: the question has to be put to the access table and not to the
    /// encoding.
    fn addresses_memory(&self) -> bool {
        let opcode = self.opcode();
        if format::format_of(opcode).modrm {
            return self.instr[self.opcode_at as usize + 1] >> 6 != 3;
        }
        // No ModR/M byte: `A0`-`A3` and `XLAT` carry their address another way,
        // and the access table is the only thing that knows.
        let acc = access::operand_access(opcode, 0);
        acc.reads || acc.writes
    }

    /// Whether this instruction reaches memory for its operand, and so will
    /// want the bus once its address is worked out.
    ///
    /// The BIU asks so it can stay off the bus. See [`I8088::tick_biu_while`].
    ///
    /// **A register operand is not a memory operand**, however the table
    /// describes the instruction: `ADC r/m8, imm8` reads and writes, but with
    /// `mod=11` it does both in a register and runs no bus cycle. Missing that
    /// left every register form carrying a segment override a clock short,
    /// uniformly, across the whole ModR/M group: `80 /7`, `81 /2`, `82 /0`,
    /// `83 /0`, `C6`, `F7 /0` and their neighbours, all wrong on every case.
    fn operand_reaches_memory(&self) -> bool {
        let has_modrm = format::format_of(self.opcode()).modrm;
        let modrm = if has_modrm {
            self.instr[self.opcode_at as usize + 1]
        } else {
            0
        };
        if has_modrm && modrm >> 6 == 3 {
            return false;
        }
        let acc = access::operand_access(self.opcode(), modrm);
        acc.reads || acc.writes
    }

    /// One T-state of the bus, which the EU and the BIU share.
    ///
    /// This runs *every* T-state, including the ones the EU spends driving its
    /// own operand access, and that is the point. The address cycle in front of
    /// a code fetch overlaps whatever the bus is already doing (see
    /// [`TaCycle`]), so a fetch decided in the middle of an operand read issues
    /// on the clock after that read's T4 with nothing idle between. A BIU that
    /// freezes while the EU has the bus cannot express that, and this one used
    /// to freeze.
    ///
    /// The order within the clock is the part's: operate the T-state the bus is
    /// already in, take the prefetch decision, then advance the address cycle,
    /// then advance the bus cycle. A byte fetched at T4 is therefore in the
    /// queue before the decision that follows it looks at the queue, so a fetch
    /// that fills it is the one that stops the next one.
    ///
    /// **The part will not start a fetch it would still be holding when the EU
    /// comes for the bus.** A code fetch is four T-states and the EU cannot
    /// interrupt one, so beginning a fetch while an operand access is pending
    /// delays that access by up to a whole bus cycle. The recording shows the
    /// part declining: on `MOV AX, [BP+DI+4]` from an empty queue its BIU goes
    /// idle the cycle after its last fetch completes, with one byte queued and
    /// room for three, and then runs the operand read. See
    /// [`Self::eu_wants_bus`].
    fn tick_bus<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        // The EU has already run this T-state, so the phase it is now in says
        // what it is about to do as well as what it just did.
        let eu_wants_bus = self.eu_wants_bus();
        // What the EU drove this T-state, if anything. Reading it off the pins
        // rather than out of a second copy of the state keeps the two from
        // drifting: the pins are what the recording compares against.
        let eu_bus_t = if self.bus.status == BusStatus::Passive {
            None
        } else {
            Some(self.bus.t_state)
        };

        // -- operate the T-state the bus is in ------------------------------
        match self.biu {
            // T1: the address goes out on the multiplexed pins with ALE.
            Biu::Fetching { t: 1, addr, .. } => {
                self.begin_bus_cycle(BusStatus::Code, addr, SegReg::CS);
            }
            // T2 turns the multiplexed pins around for data. The address is off
            // them by now, which is what the external latch exists for.
            Biu::Fetching { t: 2, .. } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T2, SegReg::CS);
            }
            // T3: the addressed device drives the byte back. It goes on the pins
            // here and is held; it does not reach the queue until T4.
            Biu::Fetching { t: 3, addr, .. } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T3, SegReg::CS);
                let byte = bus.read(master, addr);
                self.bus.data = Some(byte);
                self.prefetch_ip = self.prefetch_ip.wrapping_add(1);
                self.biu = Biu::Fetching { t: 3, addr, byte };
            }
            // T4 completes the transaction, and the byte joins the queue here.
            // The EU runs before the BIU on a tick, so a byte delivered on this
            // T-state is one the EU can take on the *next* one.
            //
            // That one cycle is not a detail. The recorded traces show a fetched
            // byte being read out of the queue on the cycle after T4, never on
            // T4 itself: an instruction fetched into an empty queue has its
            // opcode latched on T3 and read two cycles later. Delivering it on
            // T3, which is what this did first, ran the EU a cycle ahead of the
            // part every time the queue was empty, and made every jump's reload
            // one cycle short.
            //
            // Holding it back one T-state longer when the queue was *already*
            // empty was measured and is wrong, and the measurement is worth
            // keeping because the recording appears to ask for it.
            // `queue_delivery_latency` reads the recorded traces alone, with no
            // replay: over 1.3 million fetches that landed in an already-empty
            // queue, not one is read on the T-state after its T4, and the floor
            // is two. Waiting the second T-state takes the count from 66.59% to
            // 61.90% and the empty-queue bus order from 60.10% to 12.81%.
            //
            // The two cannot both be right, and the gate is the arbiter: the
            // recorded queue-status lines are reported a cycle late, which the
            // suite documents and which that survey cannot correct for, because
            // the bus columns beside them are not. The floor of two is one
            // T-state of reporting on top of the one real T-state.
            Biu::Fetching { byte, .. } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T4, SegReg::CS);
                self.push_queue(byte);
            }
            Biu::Idle | Biu::Starting { .. } => {}
        }

        // -- take the prefetch decision -------------------------------------
        //
        // Which T-state the bus is on, ours or the EU's. There is one bus, so at
        // most one of the two is driving it, and the decision points below are
        // the same wherever the cycle came from: the part's prefetcher does not
        // know whose cycle it is waiting out.
        let bus_t = match self.biu {
            Biu::Fetching { t, .. } => Some(t),
            Biu::Starting { .. } => None,
            Biu::Idle => match eu_bus_t {
                Some(TState::T1) => Some(1),
                Some(TState::T2) => Some(2),
                Some(TState::T3) => Some(3),
                Some(TState::T4) => Some(4),
                _ => None,
            },
        };
        match bus_t {
            // The end of T2 is the decision that lets the address cycle overlap
            // T3 and T4, so a fetch decided here chains with no gap. The one at
            // T4 catches a queue that only made room when this fetch's own byte
            // went in.
            Some(2 | 4) => self.fetch_decision(eu_wants_bus, false),
            Some(_) => {}
            // Ti: an idle bus, where the queue gaining room is the event.
            None => self.fetch_decision(eu_wants_bus, true),
        }

        // -- advance the address cycle --------------------------------------
        self.ta = match self.ta {
            TaCycle::Tr => TaCycle::Ts,
            TaCycle::Ts => TaCycle::T0,
            // **T0 repeats.** The address is ready and the fetch issues on the
            // first clock the bus is free: the T4 of whatever is running, or
            // any idle clock. Holding here rather than issuing regardless is
            // the structure the rest of this unit is built on.
            TaCycle::T0 => {
                let bus_free =
                    !matches!(self.biu, Biu::Starting { .. }) && bus_t.is_none_or(|t| t == 4);
                if !bus_free || eu_wants_bus {
                    TaCycle::T0
                } else if self.queue_has_room() {
                    self.biu = Biu::Starting {
                        addr: Self::physical_addr(self.cs, self.prefetch_ip),
                    };
                    TaCycle::Td
                } else {
                    // The byte this clock's T4 delivered filled the queue after
                    // the decision that scheduled this address cycle was taken.
                    // The cycle is dropped rather than run into a queue with
                    // nowhere to put the byte, and the pause lifts when the EU
                    // takes one out.
                    self.fetch = FetchState::PausedFull;
                    TaCycle::Td
                }
            }
            TaCycle::Td => TaCycle::Td,
        };

        // -- advance the bus cycle ------------------------------------------
        self.biu = match self.biu {
            Biu::Starting { addr } => Biu::Fetching {
                t: 1,
                addr,
                byte: 0,
            },
            Biu::Fetching { t: 4, .. } => Biu::Idle,
            Biu::Fetching { t, addr, byte } => Biu::Fetching {
                t: t + 1,
                addr,
                byte,
            },
            Biu::Idle => Biu::Idle,
        };
    }

    /// Decide whether to begin a code fetch.
    ///
    /// The queue being full is not the same answer as the EU having claimed the
    /// bus, and they are kept apart because they are lifted by different events:
    /// a full queue by the EU taking a byte, a claim by the EU finishing with
    /// the bus.
    fn fetch_decision(&mut self, eu_wants_bus: bool, from_idle: bool) {
        // A transfer's `SUSP` outranks everything else here: the queue's length
        // stops mattering once the bytes it would hold are on the path not
        // taken. Asked before the full-queue test so a suspended prefetcher is
        // not recorded as a paused one, which the EU taking a byte would lift.
        if self.fetch == FetchState::Suspended {
            return;
        }
        if !self.queue_has_room() {
            self.fetch = FetchState::PausedFull;
            return;
        }
        if eu_wants_bus {
            return;
        }
        // **There is no queue-depth throttle here, and that is a measurement.**
        // The rule is that a fetch leaving the queue holding three bytes is not
        // chained into the next one but decided again three clocks later. It has
        // now been rejected four times, the first three against a BIU that
        // decided at T4 with no address cycle, and once here, where the
        // objection to the earlier three does not apply: the decision is at the
        // end of T2 with the byte's arrival still ahead of it, which is exactly
        // the shape the rule is written for. It costs the prefetched bus-cycle
        // order 83.24% to 77.32% and buys nothing, the empty-queue half not
        // moving by a single vector.
        //
        // So the throttle is not a property of this queue's depth. Do not try a
        // fifth encoding of it without a survey that says which fetches it is
        // supposed to move.
        if self.ta == TaCycle::Td {
            self.fetch = FetchState::Normal;
            self.ta = if from_idle {
                IDLE_RESTART_FROM
            } else {
                TaCycle::Tr
            };
        }
    }

    /// Whether the microcode phase hands straight to a bus cycle when it ends.
    ///
    /// The BIU asks [`ADDRESS_CYCLE_CLOCKS`] out, because that is how far in
    /// front of T1 the part's request goes in, and a prefetch begun inside that
    /// window is one the part never runs.
    fn execute_ends_on_the_bus(&self) -> bool {
        // A serviced interrupt always writes its three words.
        if self.servicing.is_some() {
            return true;
        }
        let opcode = self.opcode();
        let modrm = self.instr[self.opcode_at as usize + 1];
        if access::stack_access(opcode, modrm).pushes > 0 {
            return true;
        }
        // A register operand is written in a register, whatever the table says
        // about the instruction: only an operand that resolved to an address
        // reaches the bus.
        self.operand_at.is_some() && access::operand_access(opcode, modrm).writes
    }

    /// The T-state a bus phase is about to drive, if the EU is in one.
    ///
    /// `0` is the lead-in a write spends before its T1, and `1` means T1 has
    /// not gone out yet, so it goes out on the next clock.
    fn eu_bus_t_state(&self) -> Option<u8> {
        match self.eu {
            Eu::Reading { t, .. }
            | Eu::Writing { t, .. }
            | Eu::PoppingStack { t, .. }
            | Eu::PushingStack { t, .. }
            | Eu::ReadingVector { t, .. }
            | Eu::PortReading { t, .. }
            | Eu::PortWriting { t, .. }
            | Eu::StringAccess { t, .. }
            | Eu::Acknowledging { t, .. } => Some(t),
            _ => None,
        }
    }

    /// Whether the EU is about to want the bus, read off the phase it is in
    /// after it has run this T-state.
    ///
    /// **The part claims the bus [`ADDRESS_CYCLE_CLOCKS`] before its T1**, and
    /// that claim takes the address cycle away from the prefetcher before it can
    /// reach one of its own. A claim any later leaves a code fetch in front of
    /// the access that the part does not run, which is what the whole `POP`
    /// family looked like when this was one clock: three bus cycles against the
    /// recording's two, on every case of every file.
    ///
    /// Asking the phase rather than the arm that just ran it is what covers the
    /// clock a phase is *entered* on. A memory operand with no ModR/M byte
    /// enters its address phase on the loader's last clock, and the arm for that
    /// phase does not run until the next one; a fetch begun in that gap is the
    /// one the recording does not have.
    fn eu_wants_bus(&self) -> bool {
        if let Some(t) = self.eu_bus_t_state() {
            // Not yet at T1, so T1 is a clock or two away. This also covers the
            // gap between two transfers of one operand and the gap between an
            // operand read and the write-back behind it.
            return t <= 1;
        }
        // The phases that use no bus but end in one. Each count is what remains
        // *after* this clock, so T1 is one further out than the number says.
        match self.eu {
            Eu::AddressCalc(n) => n < ADDRESS_CYCLE_CLOCKS && self.operand_reaches_memory(),
            // These three clocks are a pop's address cycle under another name.
            Eu::StackLeadIn(n) => n < ADDRESS_CYCLE_CLOCKS,
            Eu::Executing(n) => {
                // An `OUT`'s port cycle does not start on the microcode's last
                // T-state but on the one after it: the instruction has to decide
                // what to write first. Without this an `OUT` from an empty queue
                // drives a code T1 and then an I/O write T1 on consecutive
                // T-states, which no 8088 can do.
                (n < BUS_CYCLE_CLOCKS && access::port_access(self.opcode()).is_some())
                    // A write spends a lead-in of its own before T1, so the
                    // microcode is that much further from the bus than it looks.
                    // Widened, because a divide's microcode runs to 255 clocks
                    // and `n + 2` in a byte is an overflow a release build
                    // wraps in silence.
                    || (u16::from(n) + 2
                        <= u16::from(ADDRESS_CYCLE_CLOCKS) + u16::from(timing::WRITE_LEAD_IN)
                        && self.execute_ends_on_the_bus())
            }
            _ => false,
        }
    }

    /// Whether the EU may drive T1 of a bus cycle on this T-state.
    ///
    /// There is one bus. The BIU will not normally have taken it, because
    /// [`Self::eu_wants_bus`] holds its address cycle in `T0` whenever the EU
    /// is a clock away from wanting it, but a fetch already in flight is not
    /// abandoned and the EU waits it out.
    #[inline]
    fn bus_free_for_eu(&self) -> bool {
        matches!(self.biu, Biu::Idle)
    }

    /// Drive one of the T-states after T1, where the address is no longer on
    /// the pins.
    #[inline]
    fn drive_bus_cycle(&mut self, status: BusStatus, t_state: TState, segment: SegReg) {
        self.bus = BusPins {
            status,
            t_state,
            address: None,
            data: None,
            segment: Some(segment),
        };
    }

    /// Drive T1 of a bus cycle: put the address on the multiplexed pins with
    /// ALE asserted, and say what kind of cycle this is.
    #[inline]
    fn begin_bus_cycle(&mut self, status: BusStatus, addr: u32, segment: SegReg) {
        self.bus = BusPins {
            status,
            t_state: TState::T1,
            address: Some(addr),
            data: None,
            segment: Some(segment),
        };
    }

    /// Whether the BIU may start another fetch. The 8088 prefetches whenever
    /// one byte is free, its bus being one byte wide.
    #[inline]
    fn queue_has_room(&self) -> bool {
        (self.queue_len as usize) < QUEUE_LEN
    }

    /// Take the oldest byte out of the queue.
    #[inline]
    fn pop_queue(&mut self) -> u8 {
        let byte = self.queue[0];
        self.queue.copy_within(1.., 0);
        self.queue_len -= 1;
        byte
    }

    /// Append a freshly fetched byte.
    #[inline]
    fn push_queue(&mut self, byte: u8) {
        self.queue[self.queue_len as usize] = byte;
        self.queue_len += 1;
    }

    /// Transfer control to a new offset, flushing the queue when the
    /// instruction retires.
    ///
    /// Every jump, call, return and interrupt goes through this rather than
    /// assigning `ip` directly, and that distinction is not cosmetic. The first
    /// version of this inferred a transfer by comparing the final CS:IP against
    /// where the instruction stream would have run on to, which is right for
    /// almost every case and wrong for the one the vectors are full of: a
    /// *taken* conditional jump with a displacement of zero lands exactly where
    /// it would have anyway, and the part still flushes. Address equality
    /// cannot see the difference between a branch not taken and a branch taken
    /// to the next instruction. Only the instruction knows.
    #[inline]
    pub(crate) fn set_ip(&mut self, ip: u16) {
        self.ip = ip;
        self.transferred = true;
    }

    /// Transfer control to a new segment. See [`Self::set_ip`].
    #[inline]
    pub(crate) fn set_cs(&mut self, cs: u16) {
        self.cs = cs;
        self.transferred = true;
    }

    /// Install a prefetch queue and point the BIU past it.
    ///
    /// This exists for the per-cycle test vectors, half of which run their
    /// instruction from a queue the hardware had already filled. It is not
    /// something a board does: a real 8088 arrives at a full queue by
    /// prefetching into one.
    ///
    /// IP is left alone, because here it is the architectural pointer to the
    /// next byte the EU has not consumed, and the queue sits in front of it.
    /// The BIU's pointer goes past the installed bytes so the next fetch does
    /// not read them a second time.
    ///
    /// Panics if handed more bytes than the queue holds.
    pub fn load_prefetch_queue(&mut self, bytes: &[u8]) {
        assert!(
            bytes.len() <= QUEUE_LEN,
            "the 8088 queue holds {QUEUE_LEN} bytes, got {}",
            bytes.len()
        );
        self.queue[..bytes.len()].copy_from_slice(bytes);
        self.queue_len = bytes.len() as u8;
        self.prefetch_ip = self.ip.wrapping_add(bytes.len() as u16);
        // A full queue leaves the BIU with nothing to do until the EU takes a
        // byte; a partial one lets it decide on the first clock.
        self.biu = Biu::Idle;
        self.ta = TaCycle::Td;
        self.fetch = if self.queue_has_room() {
            FetchState::Normal
        } else {
            FetchState::PausedFull
        };
    }

    /// The bytes currently queued, oldest first.
    pub fn prefetch_queue(&self) -> &[u8] {
        &self.queue[..self.queue_len as usize]
    }

    /// Whether this core models the execution time of the instruction made of
    /// `bytes`, which begin at its opcode.
    ///
    /// The per-cycle gate uses this to report the modeled and unmodeled
    /// populations apart. Mixing them produces a number that describes neither:
    /// an instruction whose microcode time is not modeled is short by all of
    /// it, and averaging that in hides how close the modeled ones are.
    pub fn models_execution_time(bytes: &[u8]) -> bool {
        let mut i = 0;
        while i < bytes.len() && decode::decode_prefix(bytes[i]).is_some() {
            i += 1;
        }
        match bytes.get(i) {
            Some(&opcode) => timing::is_modeled(opcode, bytes.get(i + 1).copied().unwrap_or(0)),
            None => false,
        }
    }

    /// Throw the queue away and restart prefetching at CS:IP.
    ///
    /// Every control transfer does this: the bytes behind the jump were fetched
    /// from the path not taken. The part reports it on QS0/QS1 as `E`, which is
    /// the only way an outside observer can see a branch being taken.
    pub(crate) fn flush_queue(&mut self) {
        self.queue_len = 0;
        self.prefetch_ip = self.ip;
        // A flush is itself the request for the reload: the address cycle
        // starts here, on the clock the queue is thrown away, rather than
        // waiting for the next decision point.
        self.biu = Biu::Idle;
        self.ta = TaCycle::Tr;
        self.fetch = FetchState::Normal;
        self.instr_len = 0;
        self.instr_pos = 0;
        self.stage = Stage::Opcode;
        // The EU goes back to the start too. Discarding the loaded instruction
        // without discarding the pipeline phase that was operating on it leaves
        // a read or write phase running against an instruction that no longer
        // exists, and it finishes by trying to execute nothing. `reset` found
        // this the hard way: it flushes, and a frame boundary lands mid-phase
        // often enough that the next frame started by executing a
        // zero-length instruction.
        self.eu = Eu::Loading;
        self.operand_at = None;
        self.operand_written = false;
        self.stack_staged = false;
        self.stack_pos = 0;
        // A deferred immediate belongs to the instruction being thrown away.
        // Left set, the loader would take the next instruction's opcode for it
        // and hand a half-loaded instruction to the pipeline.
        self.immediate_deferred = false;
        self.immediate_resuming = false;
        self.port_written = false;
        // Not `servicing`: a flush is what a taken interrupt *ends* with, so
        // clearing it here would tear down the sequence at the moment it
        // succeeds. `finish_instruction` clears it, before the flush.
        self.queue_status = Some((QueueStatus::Emptied, 0));
    }

    /// Decide what the loader fetches next, having just taken delivery of a
    /// byte. Returns true when the instruction is complete.
    fn advance_stage(&mut self) -> bool {
        let just_fetched = self.instr[self.instr_len as usize - 1];

        match self.stage {
            Stage::Opcode => {
                // A prefix is followed by another opcode, so the loader stays
                // where it is. This is also why the opcode's position in the
                // buffer has to be remembered rather than assumed to be zero.
                if decode::decode_prefix(just_fetched).is_some() {
                    return false;
                }
                self.opcode_at = self.instr_len - 1;
                let f = format::format_of(just_fetched);
                if f.modrm {
                    self.stage = Stage::Modrm;
                    false
                } else {
                    self.begin_immediate(f.imm, None)
                }
            }
            Stage::Modrm => {
                let disp = format::displacement_len(just_fetched);
                if disp > 0 {
                    self.stage = Stage::Displacement(disp);
                    false
                } else {
                    let imm = format::format_of(self.opcode()).imm;
                    self.begin_immediate(imm, Some(just_fetched))
                }
            }
            Stage::Displacement(n) if n > 1 => {
                self.stage = Stage::Displacement(n - 1);
                false
            }
            Stage::Displacement(_) => {
                let imm = format::format_of(self.opcode()).imm;
                // The ModR/M byte sits immediately after the opcode, and the
                // only immediates whose length depends on it belong to
                // opcodes that have one.
                let modrm = self.instr[self.opcode_at as usize + 1];
                self.begin_immediate(imm, Some(modrm))
            }
            Stage::Immediate(n) if n > 1 => {
                self.stage = Stage::Immediate(n - 1);
                false
            }
            Stage::Immediate(_) => {
                self.stage = Stage::Opcode;
                true
            }
        }
    }

    /// Enter the immediate stage, or finish the instruction when there is no
    /// immediate to fetch.
    ///
    /// An immediate belonging to an instruction with a *memory* operand is not
    /// fetched here at all. The loader stops, the pipeline computes the address
    /// and runs the operand access, and the loader is sent back for the
    /// immediate afterwards, which is the order the recording shows. See
    /// [`I8088::immediate_deferred`].
    fn begin_immediate(&mut self, imm: format::Imm, modrm: Option<u8>) -> bool {
        match imm.len(modrm) {
            0 => {
                self.stage = Stage::Opcode;
                true
            }
            n => {
                self.stage = Stage::Immediate(n);
                if modrm.is_some_and(|m| m >> 6 != 3) {
                    self.immediate_deferred = true;
                    // Complete enough for the address phase, which is what the
                    // caller does with a `true` here.
                    return true;
                }
                false
            }
        }
    }

    /// The opcode byte of the instruction currently loaded.
    #[inline]
    fn opcode(&self) -> u8 {
        self.instr[self.opcode_at as usize]
    }

    /// Run the instruction the loader has just finished fetching.
    ///
    /// Still atomic: every operand access an instruction makes happens inside
    /// this one call, on a single T-state. The loader above it is per-cycle,
    /// the executor below it is not yet.
    fn run_loaded_instruction<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.transferred = false;
        self.operand_at = None;
        self.operand_bytes = [0; 4];
        self.operand_written = false;
        self.stack_words = [0; 3];
        self.stack_pos = 0;
        self.stack_staged = false;
        self.stack_base = self.sp;
        self.vector_staged = false;
        self.vector_words = (0, 0);
        self.port_bytes = [0; 2];
        self.port_written = false;

        // Resolve the operand before running anything, by walking the loaded
        // bytes exactly as the executor is about to and then rewinding.
        //
        // Rewinding rather than duplicating the addressing logic is the whole
        // trick here. `consume_prefixes`, `fetch_modrm` and `resolve_modrm` are
        // the only code that knows how an effective address is built, and a
        // second copy of that knowledge would drift from the first. They are
        // safe to run twice: the only things they change are IP and
        // `instr_pos`, both restored here, and the prefix state, which is
        // recomputed identically. The address itself is a pure function of
        // registers the instruction has not touched yet.
        let saved_ip = self.ip;
        self.instr_pos = 0;
        let opcode = self.consume_prefixes();
        let operand_access = if format::format_of(opcode).modrm {
            let modrm = self.fetch_modrm();
            let resolved = self.resolve_modrm(modrm);
            if let addressing::Operand::Memory { segment, offset } = resolved {
                self.operand_at = Some((segment, offset));
            }
            access::operand_access(opcode, self.instr[self.opcode_at as usize + 1])
        } else {
            self.operand_at = self.direct_operand(opcode);
            access::operand_access(opcode, 0)
        };
        self.ip = saved_ip;
        self.instr_pos = 0;

        // The string operations are the pipeline's own, from here to the end of
        // the last iteration: the executor is called once per iteration rather
        // than once for the instruction, so `execute` never sees these opcodes.
        // Consuming the prefixes here is what moves IP past them and sets the
        // REP prefix the iterations ask about.
        if access::string_access(opcode).is_some() {
            let _ = self.consume_prefixes();
            let entry = timing::string_entry_cycles(self.rep_prefix.is_some());
            self.eu = if entry > 0 {
                Eu::StringEntry(entry)
            } else {
                self.begin_string_iteration()
            };
            return;
        }

        // A memory operand has to have its address worked out before anything
        // can be done with it, and that arithmetic takes the EU real clocks.
        // The instruction does not run on this cycle at all: the pipeline goes
        // through its address, read and write phases and comes back through
        // `run_execute_step`.
        if self.operand_at.is_some() {
            if format::format_of(opcode).modrm {
                let modrm = self.instr[self.opcode_at as usize + 1];
                let acc = access::operand_access(opcode, modrm);
                // The pause the loader took before the displacement is spent
                // inside the effective address, so it comes off here rather
                // than out of the microcode. Table 1-16 folds the
                // displacement's fetch into `+EA`, which is why
                // `address_phase_cycles` already takes its read time out; this
                // is the rest of the same decomposition.
                let cycles = access::address_phase_cycles(modrm, acc.reads || acc.writes)
                    .saturating_sub(match timing::loader_stall(opcode, modrm) {
                        timing::LoaderStall::BeforeDisplacement(n) => n,
                        _ => 0,
                    });
                self.eu = if cycles > 0 {
                    Eu::AddressCalc(cycles)
                } else {
                    // A short address under a long displacement can leave
                    // nothing to spend, and an `AddressCalc(0)` would burn a
                    // T-state doing nothing.
                    self.begin_operand_phase()
                };
                self.execute_if_ready(bus, master);
                return;
            }
            // The operands with no ModR/M byte have no *effective address* to
            // compute: the direct moves carry theirs as a displacement and
            // XLAT's is one addition, which the manual folds into its clock
            // count rather than quoting as an EA. They still spend the address
            // cycle in front of the bus request, and this core used to spend it
            // behind the access instead, as microcode.
            //
            // `A0` says so to the clock. Its ten are four in the loader, two
            // here and the four of the read, and the recording reads the next
            // instruction's first byte on the T-state after that read's T4 with
            // nothing in between. Charging those two as microcode after the read
            // put the read two T-states early and the retirement two late, and
            // the two errors cancelled in the total until the bus grew an
            // address cycle and the read stopped moving.
            self.eu = match timing::DIRECT_ADDRESS_PHASE {
                0 => self.begin_operand_phase(),
                n => Eu::AddressCalc(n),
            };
            self.execute_if_ready(bus, master);
            return;
        }

        let _ = operand_access;
        self.eu = self.begin_pre_execute_phase();
        self.execute_if_ready(bus, master);
    }

    /// Where an instruction with no ModR/M byte keeps its memory operand.
    ///
    /// Five opcodes address memory without a ModR/M byte, and they are easy to
    /// miss for exactly that reason: the operand table's cross-check only looks
    /// at instructions that have one. `MOV` between the accumulator and a
    /// direct address carries a 16-bit displacement, and `XLAT` computes its
    /// address from BX and AL. Everything else here returns `None`, including
    /// the stack and the string operations, which reach memory through paths of
    /// their own.
    ///
    /// Called with `instr_pos` where the executor will start, so the
    /// displacement comes out of the instruction the same way the executor is
    /// about to read it rather than by indexing into the buffer separately.
    fn direct_operand(&mut self, opcode: u8) -> Option<(u16, u16)> {
        match opcode {
            0xA0..=0xA3 => {
                let offset = self.fetch_word();
                Some((self.effective_segment(SegReg::DS), offset))
            }
            // XLAT reads the byte AL positions into the table at BX.
            0xD7 => Some((
                self.effective_segment(SegReg::DS),
                self.bx.wrapping_add(u16::from(self.al())),
            )),
            _ => None,
        }
    }

    /// Leave the address-calculation phase for whatever the instruction does
    /// with the operand next: a read, or straight to its microcode.
    fn begin_operand_phase(&mut self) -> Eu {
        let acc = access::operand_access(self.opcode(), self.instr[self.opcode_at as usize + 1]);
        if acc.reads {
            Eu::Reading {
                byte: 0,
                total: acc.width.bytes(),
                t: 1,
            }
        } else {
            self.after_operand_access()
        }
    }

    /// Run the instruction now, if the pipeline has nothing left to do before
    /// its microcode.
    ///
    /// `Eu::Loading` means two different things and the difference matters:
    /// either every phase the instruction needed before its microcode is done,
    /// or the loader has been sent back for a deferred immediate and the
    /// instruction is not complete yet. Executing in the second case runs an
    /// instruction whose immediate has not arrived, which the executor
    /// reports as consuming more bytes than the loader fetched.
    fn execute_if_ready<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        if self.eu == Eu::Loading && !self.immediate_resuming {
            self.run_execute_step(bus, master);
        }
    }

    /// What follows an operand access: the deferred immediate if there is one,
    /// and otherwise the stack or the microcode.
    fn after_operand_access(&mut self) -> Eu {
        if self.immediate_deferred {
            self.immediate_deferred = false;
            self.immediate_resuming = true;
            // The part does not take the deferred immediate on the T-state
            // after the operand access, but two later. See
            // [`timing::deferred_immediate_stall`].
            let modrm = self.instr[self.opcode_at as usize + 1];
            self.loader_stall = timing::deferred_immediate_stall(self.opcode(), modrm);
            return Eu::Loading;
        }
        self.begin_pre_execute_phase()
    }

    /// Everything an instruction reads between its operand and its microcode:
    /// words off the stack, or an interrupt vector.
    ///
    /// Both come after any operand read and before execution, which is the
    /// order the instructions need and the order the recording shows. `POP
    /// [mem]` takes its word off the stack and then writes the operand; an
    /// indirect far `CALL` reads its pointer operand before pushing anything;
    /// and `INT 3` reads the four bytes of its vector before it writes the
    /// first of its three words. No instruction does both.
    fn begin_pre_execute_phase(&mut self) -> Eu {
        let stack = access::stack_access(self.opcode(), self.instr[self.opcode_at as usize + 1]);
        self.stack_staged = true;
        self.stack_pos = 0;
        if stack.pops > 0 {
            return match timing::STACK_POP_LEAD_IN {
                0 => Eu::PoppingStack {
                    word: 0,
                    total: stack.pops,
                    byte: 0,
                    t: 1,
                },
                n => Eu::StackLeadIn(n),
            };
        }
        if self.staged_vector().is_some() {
            return Eu::ReadingVector { byte: 0, t: 1 };
        }
        self.begin_execute_phase()
    }

    /// The interrupt vector this instruction is going to take, when the
    /// pipeline can know it before the instruction runs.
    ///
    /// `INT 3` and `INTO` carry theirs in the opcode and `INT n` in its
    /// immediate. The interrupts a fault raises are not here: `DIV`, `IDIV` and
    /// `AAM` take one only on operands that fault, so their vector read stays
    /// inside the executor and their timing rows carry its clocks.
    fn staged_vector(&self) -> Option<u8> {
        // A hardware interrupt is not an instruction and has no opcode to ask.
        if let Some(servicing) = self.servicing {
            return Some(servicing.vector);
        }
        match self.opcode() {
            0xCC => Some(3),
            0xCD => Some(self.instr[self.opcode_at as usize + 1]),
            0xCE if flags::get(self.flags, flags::Flag::OF) => Some(4),
            _ => None,
        }
    }

    /// The unary group's byte operand, wherever it lives.
    ///
    /// `MUL` and `DIV` need their operand's value to know how long they will
    /// take, and by this point the pipeline has it: a memory operand was read
    /// over MEMR cycles into `operand_bytes`, and a register one is just a
    /// register. Nothing here touches the bus.
    fn unary_operand8(&self) -> u8 {
        let modrm = self.instr[self.opcode_at as usize + 1];
        if modrm >> 6 == 3 {
            self.get_reg8(modrm & 7)
        } else {
            self.operand_bytes[0]
        }
    }

    /// The unary group's word operand. See [`Self::unary_operand8`].
    fn unary_operand16(&self) -> u16 {
        let modrm = self.instr[self.opcode_at as usize + 1];
        if modrm >> 6 == 3 {
            self.get_reg16(modrm & 7)
        } else {
            u16::from_le_bytes([self.operand_bytes[0], self.operand_bytes[1]])
        }
    }

    /// Which way an instruction's microcode is going to branch, decided before
    /// it runs because that is when the pipeline has to know how many clocks to
    /// charge.
    ///
    /// For the conditional transfers this is "does it transfer", and it asks
    /// the same question the executor is about to ask, through the same
    /// [`I8088::test_condition`], rather than a second copy of the condition
    /// table. The loop forms are the ones that need care: `CX` is decremented
    /// by the instruction and the transfer turns on the value *after* that, so
    /// the prediction has to decrement too.
    ///
    /// For the three that are not transfers it is the branch the recording
    /// shows their microcode taking: whether `CWD` is extending a negative
    /// value, and whether `AAA` and `AAS` adjust. Reading the flags and the
    /// registers early is safe throughout, because none of these instructions
    /// changes what it branches on before it has branched.
    fn microcode_branch(&self, opcode: u8) -> bool {
        let next_cx = self.cx.wrapping_sub(1);
        match opcode {
            // AAA and AAS adjust when the low nibble is above nine or the
            // auxiliary carry is set.
            0x37 | 0x3F => self.al() & 0x0F > 9 || flags::get(self.flags, flags::Flag::AF),
            0x60..=0x7F => self.test_condition(opcode & 0x0F),
            // CWD, on the sign it is about to extend into DX.
            0x99 => self.ax & 0x8000 != 0,
            0xCE => flags::get(self.flags, flags::Flag::OF),
            // SALC, on the carry it is about to smear across AL.
            0xD6 => flags::get(self.flags, flags::Flag::CF),
            0xE0 => next_cx != 0 && !flags::get(self.flags, flags::Flag::ZF),
            0xE1 => next_cx != 0 && flags::get(self.flags, flags::Flag::ZF),
            0xE2 => next_cx != 0,
            0xE3 => self.cx == 0,
            _ => true,
        }
    }

    /// Enter the microcode phase, or go straight to running the instruction
    /// when nothing is left to spend.
    ///
    /// [`timing::eu_cycles`] is the manual's clock count with the *bus* time
    /// taken out. The instruction's own bytes have to come out too: the EU
    /// spends a cycle pulling each one from the queue, this core already spends
    /// those in [`Self::tick_eu`], and the manual's number includes them. Leave
    /// them in and every instruction runs long by its own length, which is what
    /// the first version of this did: `ADD DX, SP` took five cycles against the
    /// hardware's three, over exactly its two bytes.
    ///
    /// **Every byte except the displacement**, and that exception is the
    /// manual's own. Table 1-16 quotes a memory form as `base + EA`, and the
    /// displacement's fetch is inside the `EA` half rather than the base: the
    /// recording shows the same instruction taking the same total with a
    /// one-byte displacement and a two-byte one, and starting its operand bus
    /// cycle on the same clock in both. So the displacement's pulls are already
    /// paid for by [`access::address_phase_cycles`], and subtracting them here
    /// as well charged them twice, which left every `mod=01` form a clock short
    /// and every `mod=10` form two.
    ///
    /// A prefix costs two clocks, of which the loader already spent one pulling
    /// the byte, so each one adds a clock here. The manual gives the segment
    /// override, `LOCK` and `REP` two clocks apiece, and the recording agrees
    /// exactly: a `MOV` with a segment override runs two clocks longer than the
    /// same `MOV` without one, and it does so on the register forms as much as
    /// on the memory forms. That is why this is charged per prefix byte rather
    /// than inside the effective-address calculation, where it used to be: an
    /// override on `MOV AX, BX` costs the same two clocks and computes no
    /// address at all.
    ///
    /// A shift or rotate by CL adds four clocks a bit on top of its base. That
    /// is the one form whose cost depends on a register rather than on the
    /// encoding, so it is added here, where CL is in hand.
    fn begin_execute_phase(&mut self) -> Eu {
        // A serviced interrupt has no instruction to price. Table 1-16 gives
        // the whole sequence 61 clocks for a maskable interrupt and 50 for NMI,
        // with 7 and 5 transfers; what is left for the EU is what the pipeline
        // does not already spend on the acknowledge pair, the vector read, the
        // three pushes and the reload at the handler.
        if let Some(servicing) = self.servicing {
            /// The T-state the interrupt is recognized on, before any of it
            /// reaches the bus.
            const RECOGNITION: u16 = 1;
            /// Two INTA cycles, for a maskable interrupt only.
            const ACKNOWLEDGE: u16 = 8;
            /// Four MEMR cycles for the vector's offset and segment.
            const VECTOR_READ: u16 = 16;
            /// Six MEMW cycles for the flags, CS and IP.
            const PUSHES: u16 = 24;
            /// The flush, the two-cycle prefetch restart, and the fetch at the
            /// handler, whose byte the EU takes the cycle after T4.
            const FLUSH_AND_RELOAD: u16 = 8;
            let spent = RECOGNITION
                + VECTOR_READ
                + PUSHES
                + FLUSH_AND_RELOAD
                + if servicing.acknowledge {
                    ACKNOWLEDGE
                } else {
                    0
                };
            let documented = if servicing.acknowledge { 61 } else { 50 };
            return match documented - spent {
                0 => Eu::Loading,
                n => Eu::Executing(n as u8),
            };
        }

        let opcode = self.opcode();
        let modrm = self.instr[self.opcode_at as usize + 1];

        // **A control transfer stops prefetching before it does anything else.**
        // `SUSP` is the first or second step of every one of their microcode
        // routines, and it is here because here is where their microcode begins:
        // the queue behind a taken branch holds bytes from the path not taken,
        // and the part does not spend bus cycles filling it with more of them.
        // Only the flush at the end of the same routine lifts it.
        //
        // Without this the recording and this core part company on every one of
        // them, and the count cannot see it: `JMP`, `Jcc` taken and the indirect
        // forms each run one code fetch this core does and the part does not,
        // and it lands in the queue that is about to be thrown away.
        if timing::will_transfer(opcode, modrm, self.microcode_branch(opcode)) {
            self.fetch = FetchState::Suspended;
        }

        let mut cycles = if timing::branches_on_state(opcode) {
            i32::from(timing::branch_cycles(opcode, self.microcode_branch(opcode)))
        } else {
            i32::from(timing::eu_cycles(opcode, modrm))
        };
        if matches!(opcode, 0xD2 | 0xD3) {
            cycles += i32::from(timing::shift_count_cycles(self.cl()));
        }
        // AAM and AAD are a divide and a multiply behind a BCD adjust's name,
        // and their loops run on the immediate byte, which is in the
        // instruction rather than in a register.
        {
            // The byte after the opcode, which for these two is the immediate
            // rather than a ModR/M byte.
            let imm = self.instr[self.opcode_at as usize + 1];
            match opcode {
                0xD4 => cycles += i32::from(timing::aam_cycles(self.al(), imm)),
                0xD5 => cycles += i32::from(timing::aad_cycles(imm)),
                _ => {}
            }
        }
        // The multiplies and divides take a time that is a function of their
        // operands rather than of their encoding, so it is computed here, where
        // the operand is in hand and the pipeline has already read it.
        if matches!(opcode, 0xF6 | 0xF7) {
            let word = opcode == 0xF7;
            let operand = if word {
                u32::from(self.unary_operand16())
            } else {
                u32::from(self.unary_operand8())
            };
            // The same operand read the other way, for the two signed forms.
            // The multiplier and the dividend are the accumulator, one half
            // wide for the byte form and two for the word form.
            let shift = if word { 16 } else { 8 };
            let (signed_operand, signed_accumulator) = if word {
                (i32::from(operand as u16 as i16), i32::from(self.ax as i16))
            } else {
                (i32::from(operand as u8 as i8), i32::from(self.al() as i8))
            };
            let dividend = if word {
                (u32::from(self.dx) << 16) | u32::from(self.ax)
            } else {
                u32::from(self.ax)
            };
            let signed_dividend = if word {
                i64::from(dividend as i32)
            } else {
                i64::from(dividend as u16 as i16)
            };
            cycles += match (modrm >> 3) & 7 {
                4 => {
                    let product = if word {
                        u64::from(self.ax) * u64::from(operand)
                    } else {
                        u64::from(self.al()) * u64::from(operand)
                    };
                    let high_zero = product >> shift == 0;
                    i32::from(timing::multiply_cycles(word, self.ax, high_zero))
                }
                5 => {
                    // The flag branch is the signed form of MUL's: the upper
                    // half carries no information because it is the sign
                    // extension of the lower.
                    let product = signed_operand * signed_accumulator;
                    let sign_extends = (product >> shift) == (product << (32 - shift)) >> 31;
                    i32::from(timing::signed_multiply_cycles(
                        word,
                        signed_operand,
                        signed_accumulator,
                        sign_extends,
                    ))
                }
                6 => i32::from(timing::divide_cycles(word, dividend, operand)),
                7 => i32::from(timing::signed_divide_cycles(
                    word,
                    signed_dividend,
                    i64::from(signed_operand),
                )),
                _ => 0,
            };
        }
        let displacement = if format::format_of(opcode).modrm {
            format::displacement_len(modrm)
        } else {
            0
        };
        cycles -= i32::from(self.instr_len - self.opcode_at - displacement);
        // A prefix's second clock used to be added here. It is now spent where
        // the recording puts it, in the loader, on the T-state after the prefix
        // byte is read. See [`timing::PREFIX_PAUSE`]. The instruction's total is
        // the same either way; what moved is every read and every fetch behind
        // it, by one T-state, onto the clocks the part uses.

        // And the T-states the loader stopped for, for the same reason as the
        // bytes above: they are inside the manual's total, not on top of it.
        // Where the queue is full and the EU is the critical path an
        // instruction takes its documented clocks however its reads fall, so
        // adding the pause without taking it back here would make every one of
        // the 94 files that pause run a clock long. See
        // [`timing::loader_stall`].
        // **A pause the loader took while it would have been waiting anyway
        // cost nothing, so it is not charged back.**
        //
        // `9A` and `EA` are the case that says so. They are five bytes, so from
        // a full queue they drain it and then wait for their last byte. The
        // pause after their opcode is spent inside that wait: the last byte
        // still arrives on the T-state the refill delivers it, and the loader
        // finishes exactly when it would have. Charging the row for it anyway
        // made both of them one clock short on every case, which is how they
        // went from exact to `-1:2489` and `-1:2437` the moment the pauses
        // landed. They had been exact by cancellation before that, one clock
        // early on the byte and one clock long on the row.
        // Subtracting only the part not absorbed by the loader's own starvation,
        // `planned.saturating_sub(self.loader_starved)`, was measured and is
        // wrong: it is right for the full queue, where it took the prefetched
        // count from 92.69% to 92.89%, and badly wrong for the empty one, where
        // it took that half from 54.83% to 49.27% and the total to 71.08%. A
        // pause inside a *chronically* empty queue is not absorbed, because
        // there the pause delays the drain, which delays the next fetch, which
        // delays the byte. Absorption needs the refill to be in flight already.
        let planned = timing::loader_stall(opcode, modrm).charged_to_microcode()
            + timing::deferred_immediate_stall(opcode, modrm);
        cycles -= i32::from(planned);

        // And the clocks a pop spends before its read reaches the bus, which
        // are these same clocks moved to the other side of it. See
        // [`Eu::StackLeadIn`].
        if access::stack_access(opcode, modrm).pops > 0 {
            cycles -= i32::from(timing::STACK_POP_LEAD_IN);
        }

        // And the address cycle in front of a direct memory operand, for the
        // same reason. See [`timing::DIRECT_ADDRESS_PHASE`].
        if self.operand_at.is_some() && !format::format_of(opcode).modrm {
            cycles -= i32::from(timing::DIRECT_ADDRESS_PHASE);
        }

        // An instruction as long as the queue costs one clock more, unless it
        // reaches memory.
        //
        // Measured, and the discriminator is sharp. Under a segment override
        // the accumulator-immediate forms split exactly by the width of their
        // immediate: `04`, `0C` ... `3C`, `A8` and `B0`-`B7` carry an eight-bit
        // one and are exact on every case, while `05`, `0D` ... `3D`, `A9` and
        // `B8`-`BF` carry a sixteen-bit one and are -1 on every case. The
        // override makes the second group four bytes long and leaves the first
        // at three.
        //
        // It does **not** reach `0x81`'s register form, which is -1 with no
        // prefix at all where `0x80`, `0x82` and `0x83` are exact. That form is
        // four bytes for the same reason, and the condition here is satisfied,
        // and the clock is spent: it is simply invisible. A four-byte
        // instruction drains a full queue exactly, so the span runs to the
        // refill rather than to the microcode, and the EU retires with time in
        // hand however much of it is charged. That is why putting the clock in
        // the row did nothing either, and `F7 /0 TEST` is the control: also four
        // bytes, documented five clocks against this one's four, and the part
        // takes seven for both. What is missing there is a clock in the queue
        // refill at an instruction boundary, not in any row. See
        // [`I8088::tick_biu_while`].
        //
        // `A0`-`A3` are the control. They are four bytes under an override too
        // and they are exact, because they reach memory and their timing is the
        // operand path's rather than the loader's.
        // A control transfer is excluded for the same reason: it throws the
        // queue away, so there is no refill behind it to pay for, and the
        // reload at its target is already counted as the seven clocks under
        // the transfer rows.
        // The condition is *addressing*, not access. `LEA` computes a memory
        // address and runs no bus cycle, so asking the access table gets it
        // wrong in both directions at once: it exempted the `0x81` register
        // forms this exists for and caught `LEA`'s `disp16` forms, which were
        // exact before and +1 after.
        if usize::from(self.instr_len) >= QUEUE_LEN
            && !self.addresses_memory()
            && !timing::may_flush_the_queue(opcode)
        {
            cycles += 1;
        }

        match cycles.clamp(0, i32::from(u8::MAX)) as u8 {
            0 => Eu::Loading,
            n => Eu::Executing(n),
        }
    }

    /// Run the instruction proper, with its operand already in hand, and then
    /// hand off to the write-back phase if it produced one.
    fn run_execute_step<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        // A serviced interrupt runs here in place of an instruction, with its
        // vector already read off the bus. What it does is the same three
        // pushes and the same transfer `INT n` does, so it goes through the
        // same `interrupt`, and the pushes it stages leave through the same
        // stack phase. None of the per-instruction cross-checks below apply:
        // there is no opcode, no operand and no loaded length to check.
        if self.servicing.is_some() {
            let vector = self.staged_vector().unwrap_or(0);
            self.stack_ops = (0, 0);
            self.interrupt(bus, master, vector);
            if !self.hand_staged_pushes_to_the_bus() {
                self.finish_instruction();
            }
            return;
        }

        self.instr_pos = 0;
        self.operand_ops = (0, 0);
        self.stack_ops = (0, 0);
        let opcode = self.consume_prefixes();

        // What the pipeline predicted about a conditional transfer, asked again
        // here, where the instruction has not run yet and the registers are
        // still what they were when [`Self::begin_execute_phase`] looked at
        // them. Comparing it against what the instruction actually did is the
        // same discipline the operand and stack tables are held to: the
        // prediction is a second statement of something `execute.rs` already
        // knows, and a pair like that drifts silently. Getting it backwards
        // would charge every taken branch the not-taken time and every
        // fall-through the taken time, and nothing but the gate's aggregate
        // would notice.
        #[cfg(debug_assertions)]
        let predicted =
            timing::is_conditional_transfer(opcode).then(|| self.microcode_branch(opcode));

        self.execute(opcode, bus, master);

        #[cfg(debug_assertions)]
        if let Some(predicted) = predicted {
            debug_assert_eq!(
                predicted,
                self.transferred,
                "opcode {opcode:02X}: the pipeline charged the {} time and the \
                 instruction {} transfer",
                if predicted { "taken" } else { "not taken" },
                if self.transferred { "did" } else { "did not" },
            );
        }

        // Cross-check the stack table the same way, and before anything depends
        // on it. A count fixed by the opcode cannot describe the instructions
        // whose stack use is conditional, so those declare nothing and are
        // exempt: INTO pushes only on overflow, and DIV, IDIV and AAM push only
        // when they fault.
        #[cfg(debug_assertions)]
        {
            let want = access::stack_access(opcode, self.instr[self.opcode_at as usize + 1]);
            let conditional = matches!(opcode, 0xCE | 0xD4 | 0xF6 | 0xF7);
            if !conditional {
                debug_assert_eq!(
                    self.stack_ops,
                    (want.pops, want.pushes),
                    "opcode {opcode:02X}: the stack table says {} pops and {} pushes, \
                     the executor did {} and {}",
                    want.pops,
                    want.pushes,
                    self.stack_ops.0,
                    self.stack_ops.1,
                );
            }
        }

        // Cross-check the operand-access table against what the executor
        // actually did, before anything depends on the table.
        //
        // `access::operand_access` is a second, independent statement of
        // something `execute.rs` already knows implicitly, and a pair like that
        // drifts silently. Rather than trust it, compare: the table predicted
        // this instruction would read and write its ModR/M operand, and here is
        // what it did. A debug build runs this on all 3,007,000 per-cycle
        // vectors, which is what makes the table load-bearing safely.
        //
        // Checked for instructions with a ModR/M byte addressing *memory*, and
        // for the five that address memory without one: the direct-address
        // accumulator moves and XLAT. Those five were the hole this check had
        // in it, and they sat in it for two milestones, running their operand
        // access on a single T-state with no bus cycle at all.
        //
        // What is still outside it: push, pop, the string moves and the
        // interrupt vector reads, which this table does not describe and which
        // the pipeline handles separately or not yet. A register operand costs
        // no bus cycle either way, which is what lets `PUSH SP` take its own
        // path through the 8088's push-the-decremented-value quirk without
        // looking like a disagreement.
        #[cfg(debug_assertions)]
        if (format::format_of(opcode).modrm && self.instr[self.opcode_at as usize + 1] >> 6 != 3)
            || matches!(opcode, 0xA0..=0xA3 | 0xD7)
        {
            let modrm = self.instr[self.opcode_at as usize + 1];
            let want = access::operand_access(opcode, modrm);
            let (reads, writes) = self.operand_ops;
            debug_assert_eq!(
                (reads > 0, writes > 0),
                (want.reads, want.writes),
                "opcode {opcode:02X} modrm {modrm:02X}: the operand table says \
                 reads={} writes={}, the executor did {reads} reads and {writes} writes",
                want.reads,
                want.writes,
            );
        }

        // The loader and the executor have to agree on how long the
        // instruction was, or one of them is reading bytes the other never
        // fetched. Consuming too few leaves IP short, which the 2,577,000-vector
        // state gate reports as an IP mismatch on every affected case;
        // consuming too many is impossible, because `fetch_byte` asserts. This
        // catches the remaining case in a debug build, where the two agree on
        // nothing in particular but the instruction happened to end at the
        // right address anyway.
        debug_assert_eq!(
            self.instr_pos,
            self.instr_len,
            "opcode {:02X}: loader fetched {} bytes, executor consumed {}",
            self.opcode(),
            self.instr_len,
            self.instr_pos,
        );

        if self.hand_staged_pushes_to_the_bus() {
            return;
        }

        self.finish_or_write_operand();
    }

    /// Send whatever the instruction pushed out over the bus, and say whether
    /// there was any.
    ///
    /// Pushes go before an operand write-back, which matters for `PUSH [mem]`:
    /// it reads its operand and pushes it, and that is the order the recorded
    /// traces show.
    fn hand_staged_pushes_to_the_bus(&mut self) -> bool {
        if self.stack_staged && self.stack_pos > 0 && self.stack_ops.1 > 0 {
            let total = self.stack_pos;
            self.stack_pos = 0;
            self.eu = Eu::PushingStack {
                word: 0,
                total,
                byte: 0,
                t: timing::WRITE_LEAD_IN,
            };
            return true;
        }
        false
    }

    /// Send the operand write-back out if the instruction produced one, and
    /// otherwise retire.
    ///
    /// A write goes over the bus after the instruction has decided what to
    /// write, which is another phase rather than another cycle of this one.
    fn finish_or_write_operand(&mut self) {
        if self.operand_written && self.operand_at.is_some() {
            let width =
                access::operand_access(self.opcode(), self.instr[self.opcode_at as usize + 1])
                    .width;
            self.eu = Eu::Writing {
                byte: 0,
                total: width.bytes(),
                t: timing::WRITE_LEAD_IN,
            };
            return;
        }

        // An `OUT`'s port cycle goes out here for the same reason an operand
        // write-back does: the instruction has to decide what to write first.
        if self.port_written
            && let Some(acc) = access::port_access(self.opcode())
        {
            self.port_written = false;
            self.eu = Eu::PortWriting {
                byte: 0,
                total: acc.width.bytes(),
                t: 1,
            };
            return;
        }

        self.finish_instruction();
    }

    /// Retire the instruction: the last thing every path through the pipeline
    /// does.
    fn finish_instruction(&mut self) {
        self.eu = Eu::Loading;
        self.instr_len = 0;
        self.instr_pos = 0;
        // Stop staging stack traffic the moment the instruction is over.
        //
        // An interrupt taken at the next boundary pushes three words through
        // `push16`, and if this flag were still set they would be staged into a
        // buffer no phase is going to write out: the words would simply vanish,
        // and the return address with them. Q*bert takes a VBLANK NMI every
        // frame, so it found this immediately, in the golden frame and in the
        // boot check rather than in any CPU vector.
        self.stack_staged = false;
        self.stack_pos = 0;
        // And the interrupt, if that is what just finished. Left set, the next
        // instruction's execute step would take itself for an interrupt.
        self.servicing = None;

        if self.transferred {
            // The queue holds bytes from the path not taken, and throwing them
            // away is what makes a jump cost what it costs. The flush happens
            // on the next T-state, not this one: see `pending_flush`. Until it
            // does, the instruction has not retired.
            self.pending_flush = true;
        } else {
            self.retired = true;
        }
    }

    /// The port an `IN` or `OUT` addresses: its immediate byte, or DX.
    ///
    /// Known before the instruction runs either way, which is what lets the
    /// pipeline drive the cycle rather than the executor.
    fn port_of(&self, opcode: u8) -> u16 {
        match opcode {
            0xE4..=0xE7 => u16::from(self.instr[self.opcode_at as usize + 1]),
            _ => self.dx,
        }
    }

    /// The microcode clocks one iteration of the current string operation
    /// spends, beyond its bus cycles.
    fn string_iteration_cycles(&self) -> u8 {
        timing::string_cycles(self.opcode(), self.rep_prefix.is_some())
    }

    /// Start a string operation, or the next iteration of one.
    ///
    /// Returns the phase to be in. A repeated operation whose count is already
    /// zero does nothing at all, which is the one case with no iteration and no
    /// bus cycle.
    fn begin_string_iteration(&mut self) -> Eu {
        let opcode = self.opcode();
        let access = access::string_access(opcode).expect("a string operation");
        if self.rep_prefix.is_some() && self.cx == 0 {
            return Eu::StringDelay(0);
        }
        // Both addresses, before the iteration steps the registers they come
        // from.
        let (source, dest) = self.string_addresses();
        self.string_at = [source, dest];
        let part = if access.reads_source {
            StringPart::Source
        } else if access.reads_dest {
            StringPart::Destination
        } else {
            // `STOS` reads nothing, so its iteration has to run before the
            // write rather than after: the write goes out of the buffer the
            // iteration stages the accumulator into.
            self.string_iteration(opcode);
            StringPart::Write
        };
        Eu::StringAccess {
            part,
            byte: 0,
            t: 1,
        }
    }

    /// One T-state of a string operation's bus traffic.
    ///
    /// Every iteration is up to two accesses of up to two bytes each, and the
    /// pipeline walks them one T-state at a time so that a `REP` of a thousand
    /// cycles takes a thousand cycles. The executor is not called between them:
    /// it is called once per iteration, in [`I8088::string_iteration`], with
    /// the reads already done.
    fn tick_string<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let Eu::StringAccess { part, byte, t } = self.eu else {
            return;
        };
        let opcode = self.opcode();
        let access = access::string_access(opcode).expect("a string operation");
        let (segment, offset) = match part {
            StringPart::Source => self.string_at[0],
            _ => self.string_at[1],
        };
        let addr = Self::physical_addr(segment, offset.wrapping_add(u16::from(byte)));
        let writing = part == StringPart::Write;
        let status = if writing {
            BusStatus::MemWrite
        } else {
            BusStatus::MemRead
        };
        // The source is read through DS or an override; everything at the
        // destination goes through ES, which no prefix can change.
        let seg_reg = match part {
            StringPart::Source => self.segment_override.unwrap_or(SegReg::DS),
            _ => SegReg::ES,
        };
        // Where the byte lands: the source in the first half of the buffer,
        // the destination read in the second, and a write comes out of the
        // first, which is where the iteration staged it.
        let slot = match part {
            StringPart::Destination => 2 + byte as usize,
            _ => byte as usize,
        };

        match t {
            1 => {
                self.begin_bus_cycle(status, addr, seg_reg);
                self.eu = Eu::StringAccess { part, byte, t: 2 };
            }
            2 => {
                self.drive_bus_cycle(status, TState::T2, seg_reg);
                self.eu = Eu::StringAccess { part, byte, t: 3 };
            }
            3 => {
                self.drive_bus_cycle(status, TState::T3, seg_reg);
                if writing {
                    let value = self.operand_bytes[slot];
                    self.bus.data = Some(value);
                    bus.write(master, addr, value);
                } else {
                    let value = bus.read(master, addr);
                    self.bus.data = Some(value);
                    self.operand_bytes[slot] = value;
                }
                self.eu = Eu::StringAccess { part, byte, t: 4 };
            }
            _ => {
                self.drive_bus_cycle(status, TState::T4, seg_reg);
                if byte + 1 < access.width {
                    self.eu = Eu::StringAccess {
                        part,
                        byte: byte + 1,
                        t: 1,
                    };
                    return;
                }
                self.eu = match part {
                    // The source is read; the destination may still have to be
                    // read or written, and the iteration runs between the two.
                    StringPart::Source if access.reads_dest => Eu::StringAccess {
                        part: StringPart::Destination,
                        byte: 0,
                        t: 1,
                    },
                    StringPart::Source | StringPart::Destination => {
                        self.string_iteration(opcode);
                        if access.writes_dest {
                            Eu::StringAccess {
                                part: StringPart::Write,
                                byte: 0,
                                t: 1,
                            }
                        } else {
                            Eu::StringDelay(self.string_iteration_cycles())
                        }
                    }
                    StringPart::Write => Eu::StringDelay(self.string_iteration_cycles()),
                };
            }
        }
    }

    /// One T-state of a string iteration's microcode time, and the decision at
    /// the end of it: another iteration, an interrupt, or the end of the
    /// instruction.
    fn tick_string_delay<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let Eu::StringDelay(remaining) = self.eu else {
            return;
        };
        if remaining > 1 {
            self.eu = Eu::StringDelay(remaining - 1);
            return;
        }
        if self.string_repeats_again(self.opcode()) {
            // The one interrupt window inside an instruction. Everywhere else
            // the pipeline recognizes an interrupt between instructions, which
            // is the only point the queue can be redirected without discarding
            // a partial fetch; here the microcode is between iterations and has
            // consumed nothing, so the part checks and this core has to as well.
            //
            // Without it a `REP MOVSW` of 0xFFFF holds off an interrupt for
            // roughly a million clocks. Q*bert takes a VBLANK NMI every frame,
            // and a board whose handler runs a frame late is a board that is
            // wrong in a way no CPU vector can see: the suite records no
            // interrupt anywhere.
            if self.servicing.is_none() {
                let ints = bus.check_interrupts(master);
                if self.restart_for_interrupt(ints) {
                    return;
                }
            }
            self.eu = self.begin_string_iteration();
            return;
        }
        // The executor never ran for this instruction, so the length it would
        // have consumed is settled here instead. IP has already moved past the
        // prefixes and the opcode.
        self.instr_pos = self.instr_len;
        self.rep_prefix = None;
        self.finish_instruction();
    }

    /// Abandon the repeated string operation in progress and take an interrupt,
    /// leaving IP where the instruction started so the handler's `IRET` resumes
    /// the repeat rather than falling out of it.
    ///
    /// Returns false, changing nothing, when there was no interrupt to take.
    ///
    /// **What is restored is the whole instruction, prefixes included.** The
    /// part restores less than that: it remembers one prefix, so a `REP` with a
    /// segment override in front of it comes back without the override and
    /// finishes the copy through the wrong segment. That is a documented defect
    /// of the part rather than a property worth reproducing, and reproducing it
    /// would mean a board's own interrupt rate deciding where its string moves
    /// read from. The divergence is deliberate rather than an oversight, and
    /// nothing in the suite can see it either way: no trace in the
    /// three million vectors records an interrupt at all.
    fn restart_for_interrupt(&mut self, ints: InterruptState) -> bool {
        // IP has moved one byte per prefix and one for the opcode, and
        // `instr_pos` counted them, so it is the distance back to the start.
        let resumed = self.ip.wrapping_sub(u16::from(self.instr_pos));
        let abandoned = self.ip;
        self.ip = resumed;
        if !self.begin_interrupt(ints) {
            self.ip = abandoned;
            return false;
        }
        // The instruction is gone: the loader starts again at `resumed` once
        // the handler returns, and the queue behind it is flushed by the
        // transfer to the handler.
        self.instr_len = 0;
        self.instr_pos = 0;
        self.rep_prefix = None;
        self.segment_override = None;
        true
    }

    /// One T-state of an I/O access: a four-T-state IOR or IOW cycle per byte,
    /// low byte first, at consecutive port numbers.
    ///
    /// The port goes on the address pins the way a memory address does, in the
    /// low sixteen bits, which is what the recording shows: `IN AL, 1Bh`
    /// latches 0001B. No segment register computes it, so the segment status
    /// lines say nothing for these cycles.
    fn tick_port<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
        reading: bool,
    ) {
        let (byte, total, t) = match self.eu {
            Eu::PortReading { byte, total, t } | Eu::PortWriting { byte, total, t } => {
                (byte, total, t)
            }
            _ => return,
        };
        let port = self.port_of(self.opcode()).wrapping_add(u16::from(byte));
        let status = if reading {
            BusStatus::IoRead
        } else {
            BusStatus::IoWrite
        };
        let rebuild = |byte: u8, t: u8| {
            if reading {
                Eu::PortReading { byte, total, t }
            } else {
                Eu::PortWriting { byte, total, t }
            }
        };

        match t {
            1 => {
                self.bus = BusPins {
                    status,
                    t_state: TState::T1,
                    address: Some(u32::from(port)),
                    data: None,
                    segment: None,
                };
                self.eu = rebuild(byte, 2);
            }
            2 => {
                self.drive_port_cycle(status, TState::T2);
                self.eu = rebuild(byte, 3);
            }
            3 => {
                self.drive_port_cycle(status, TState::T3);
                if reading {
                    let value = bus.io_read(master, u32::from(port));
                    self.bus.data = Some(value);
                    self.port_bytes[byte as usize] = value;
                } else {
                    let value = self.port_bytes[byte as usize];
                    self.bus.data = Some(value);
                    bus.io_write(master, u32::from(port), value);
                }
                self.eu = rebuild(byte, 4);
            }
            _ => {
                self.drive_port_cycle(status, TState::T4);
                if byte + 1 < total {
                    self.eu = rebuild(byte + 1, 1);
                } else if reading {
                    // The port's bytes are in hand, so the instruction can run.
                    self.eu = Eu::Loading;
                    self.run_execute_step(bus, master);
                } else {
                    // The recorded `OUT` from an empty queue ends its span on
                    // this T4, where this core ends it a clock later, on every
                    // case of all four files. Letting the loader take the next
                    // First Byte here is measured and wrong: it puts an extra
                    // queue operation inside the span and takes the
                    // queue-operation sequence off 100.00% to 98.67%, which is
                    // the one gate this core has never failed.
                    self.finish_instruction();
                }
            }
        }
    }

    /// Drive a T-state of an I/O cycle, which no segment register addresses.
    #[inline]
    fn drive_port_cycle(&mut self, status: BusStatus, t_state: TState) {
        self.bus = BusPins {
            status,
            t_state,
            address: None,
            data: None,
            segment: None,
        };
    }

    /// One T-state of the interrupt-vector read: four MEMR bus cycles from the
    /// table at the bottom of memory, offset first and then segment.
    ///
    /// The vector table is at physical zero and is addressed through no segment
    /// register at all, which is why this cannot go through the operand
    /// pipeline: `operand_at` is a segment and an offset, and here the segment
    /// really is zero rather than defaulting to DS.
    fn tick_vector_read<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let Eu::ReadingVector { byte, t } = self.eu else {
            return;
        };
        let vector = self.staged_vector().expect("a vector phase has a vector");
        let addr = u32::from(vector) * 4 + u32::from(byte);

        match t {
            1 => {
                self.begin_bus_cycle(BusStatus::MemRead, addr, SegReg::DS);
                self.eu = Eu::ReadingVector { byte, t: 2 };
            }
            2 => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T2, SegReg::DS);
                self.eu = Eu::ReadingVector { byte, t: 3 };
            }
            3 => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T3, SegReg::DS);
                let value = bus.read(master, addr);
                self.bus.data = Some(value);
                let shift = 8 * u32::from(byte & 1);
                let word = if byte < 2 {
                    &mut self.vector_words.0
                } else {
                    &mut self.vector_words.1
                };
                *word = (*word & !(0xFF << shift)) | (u16::from(value) << shift);
                self.eu = Eu::ReadingVector { byte, t: 4 };
            }
            _ => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T4, SegReg::DS);
                if byte + 1 < 4 {
                    self.eu = Eu::ReadingVector {
                        byte: byte + 1,
                        t: 1,
                    };
                } else {
                    self.vector_staged = true;
                    self.eu = self.begin_execute_phase();
                    self.execute_if_ready(bus, master);
                }
            }
        }
    }

    /// One T-state of the operand read phase: a MEMR bus cycle per byte, low
    /// byte first, the 8088's data bus being one byte wide.
    fn tick_operand_read<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let Eu::Reading { byte, total, t } = self.eu else {
            return;
        };
        let (segment, offset) = self.operand_at.expect("a read phase has an operand");
        let addr = Self::physical_addr(segment, offset.wrapping_add(byte.into()));

        match t {
            1 => {
                self.begin_bus_cycle(BusStatus::MemRead, addr, self.operand_segment());
                self.eu = Eu::Reading { byte, total, t: 2 };
            }
            2 => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T2, self.operand_segment());
                self.eu = Eu::Reading { byte, total, t: 3 };
            }
            3 => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T3, self.operand_segment());
                let value = bus.read(master, addr);
                self.bus.data = Some(value);
                self.operand_bytes[byte as usize] = value;
                self.eu = Eu::Reading { byte, total, t: 4 };
            }
            _ => {
                self.drive_bus_cycle(BusStatus::MemRead, TState::T4, self.operand_segment());
                if byte + 1 < total {
                    self.eu = Eu::Reading {
                        byte: byte + 1,
                        total,
                        t: 1,
                    };
                } else {
                    // The operand is in hand. Next comes the immediate, if this
                    // instruction has one the loader was told to leave, and
                    // then the stack and the microcode.
                    self.eu = self.after_operand_access();
                    self.execute_if_ready(bus, master);
                }
            }
        }
    }

    /// One T-state of the stack phases, which are the operand phases over a
    /// different address: MEMR cycles up from SP for a pop, MEMW cycles down
    /// from where the pushes left it.
    ///
    /// `popping` picks the direction. The two are one function because they
    /// differ only in which way the data moves and which status line goes out,
    /// and keeping them apart meant two copies of the same four-T-state walk.
    fn tick_stack<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
        popping: bool,
    ) {
        let (word, total, byte, t) = match self.eu {
            Eu::PoppingStack {
                word,
                total,
                byte,
                t,
            }
            | Eu::PushingStack {
                word,
                total,
                byte,
                t,
            } => (word, total, byte, t),
            _ => return,
        };

        // Pops read upward from SP, oldest first. Pushes write from the base SP
        // downward, and the executor staged them in the order it pushed them,
        // so word 0 is the deepest.
        let offset = if popping {
            self.sp.wrapping_add(u16::from(word) * 2)
        } else {
            self.stack_base.wrapping_sub(u16::from(word + 1) * 2)
        };
        let addr = Self::physical_addr(self.ss, offset.wrapping_add(byte.into()));
        let status = if popping {
            BusStatus::MemRead
        } else {
            BusStatus::MemWrite
        };

        let rebuild = |byte: u8, t: u8| {
            if popping {
                Eu::PoppingStack {
                    word,
                    total,
                    byte,
                    t,
                }
            } else {
                Eu::PushingStack {
                    word,
                    total,
                    byte,
                    t,
                }
            }
        };

        match t {
            // The write's lead-in. `fetch_gap_diff` says the part spends a
            // T-state here, before the first write's T1: every `PUSH` starts
            // that write six T-states after the preceding code fetch's T1 where
            // this core started it at five, uniformly, on all 5000 cases of each
            // of twelve files.
            //
            // Spending it alone was measured and wrong, at 70.39% against
            // 73.76%, and so was spending it with the clock taken back out of
            // the pushing rows, at 73.19%. It is neither: the clock comes off
            // the far end of the same bus cycle, because the EU lets go of a
            // write at T3. See [`timing::WRITE_LEAD_IN`] and [`I8088::eu_tail`].
            0 => self.eu = rebuild(byte, 1),
            1 => {
                self.begin_bus_cycle(status, addr, SegReg::SS);
                self.eu = rebuild(byte, 2);
            }
            2 => {
                self.drive_bus_cycle(status, TState::T2, SegReg::SS);
                self.eu = rebuild(byte, 3);
            }
            3 => {
                self.drive_bus_cycle(status, TState::T3, SegReg::SS);
                let slot = word as usize;
                if popping {
                    let value = bus.read(master, addr);
                    self.bus.data = Some(value);
                    let shift = 8 * u32::from(byte);
                    self.stack_words[slot] =
                        (self.stack_words[slot] & !(0xFF << shift)) | (u16::from(value) << shift);
                } else {
                    let value = (self.stack_words[slot] >> (8 * u32::from(byte))) as u8;
                    self.bus.data = Some(value);
                    bus.write(master, addr, value);
                }
                // The last byte of the last word is a write the EU has nothing
                // left to wait for, so it moves on here and the T4 goes out
                // behind it. See [`I8088::eu_tail`].
                if !popping && byte == 1 && word + 1 == total {
                    self.eu_tail = Some((status, SegReg::SS));
                    self.finish_or_write_operand();
                } else {
                    self.eu = rebuild(byte, 4);
                }
            }
            _ => {
                self.drive_bus_cycle(status, TState::T4, SegReg::SS);
                if byte == 0 {
                    // The high byte of the same word.
                    self.eu = rebuild(1, 1);
                } else if word + 1 < total {
                    self.eu = if popping {
                        Eu::PoppingStack {
                            word: word + 1,
                            total,
                            byte: 0,
                            t: 1,
                        }
                    } else {
                        Eu::PushingStack {
                            word: word + 1,
                            total,
                            byte: 0,
                            t: 1,
                        }
                    };
                } else {
                    // Everything the instruction will pop is in hand. A push
                    // never arrives here: its last T4 goes out from `eu_tail`
                    // after the EU has already moved on at T3.
                    debug_assert!(popping, "a push should have retired at T3");
                    self.stack_pos = 0;
                    self.eu = self.begin_execute_phase();
                    self.execute_if_ready(bus, master);
                }
            }
        }
    }

    /// One T-state of the operand write-back phase: a MEMW bus cycle per byte.
    fn tick_operand_write<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let Eu::Writing { byte, total, t } = self.eu else {
            return;
        };
        let (segment, offset) = self.operand_at.expect("a write phase has an operand");
        let addr = Self::physical_addr(segment, offset.wrapping_add(byte.into()));

        match t {
            // The lead-in. See [`timing::WRITE_LEAD_IN`].
            0 => self.eu = Eu::Writing { byte, total, t: 1 },
            1 => {
                self.begin_bus_cycle(BusStatus::MemWrite, addr, self.operand_segment());
                self.eu = Eu::Writing { byte, total, t: 2 };
            }
            2 => {
                self.drive_bus_cycle(BusStatus::MemWrite, TState::T2, self.operand_segment());
                self.eu = Eu::Writing { byte, total, t: 3 };
            }
            3 => {
                self.drive_bus_cycle(BusStatus::MemWrite, TState::T3, self.operand_segment());
                let value = self.operand_bytes[byte as usize];
                self.bus.data = Some(value);
                bus.write(master, addr, value);
                if byte + 1 < total {
                    self.eu = Eu::Writing { byte, total, t: 4 };
                } else {
                    // The last byte of the write-back is on the pins and the EU
                    // has nothing left to wait for, so it retires here and the
                    // T4 goes out behind it. See [`I8088::eu_tail`].
                    self.eu_tail = Some((BusStatus::MemWrite, self.operand_segment()));
                    self.finish_instruction();
                }
            }
            _ => {
                self.drive_bus_cycle(BusStatus::MemWrite, TState::T4, self.operand_segment());
                self.eu = Eu::Writing {
                    byte: byte + 1,
                    total,
                    t: 1,
                };
            }
        }
    }

    /// Which segment register the operand's address was computed from, for the
    /// S3/S4 status lines. An override picks it; otherwise it is whatever the
    /// addressing mode defaults to.
    fn operand_segment(&self) -> SegReg {
        let modrm = self.instr[self.opcode_at as usize + 1];
        self.segment_override
            .unwrap_or_else(|| self.default_segment_for_rm(modrm & 7, modrm >> 6))
    }

    /// Check for pending interrupts, and start servicing one if there is one.
    ///
    /// Returns true when the pipeline has taken it over, which is the caller's
    /// signal that this T-state belongs to the interrupt rather than to an
    /// instruction. What follows is a sequence of bus cycles rather than a
    /// single call: the acknowledge pair for a maskable interrupt, then the
    /// vector read, then the three pushes. See [`Servicing`].
    fn begin_interrupt(&mut self, ints: InterruptState) -> bool {
        // NMI is edge-triggered
        let nmi_edge = crate::cpu::flags::detect_rising_edge(ints.nmi, &mut self.nmi_prev);
        if nmi_edge {
            self.nmi_pending = true;
        }

        // NMI takes priority over IRQ, and runs no acknowledge cycles: nothing
        // on the bus has to tell the part which vector it is.
        let servicing = if self.nmi_pending {
            self.nmi_pending = false;
            Servicing {
                vector: 2,
                acknowledge: false,
            }
        } else if ints.irq && flags::get(self.flags, flags::Flag::IF) {
            // Level-triggered and masked by IF. The vector comes from the
            // board, which is what the acknowledge cycles would fetch from the
            // interrupting device on a real one.
            Servicing {
                vector: ints.irq_vector,
                acknowledge: true,
            }
        } else {
            return false;
        };

        self.servicing = Some(servicing);
        self.stack_staged = true;
        self.stack_pos = 0;
        self.stack_base = self.sp;
        self.vector_staged = false;
        self.eu = if servicing.acknowledge {
            Eu::Acknowledging { cycle: 0, t: 1 }
        } else {
            Eu::ReadingVector { byte: 0, t: 1 }
        };
        true
    }

    /// One T-state of the interrupt-acknowledge pair.
    ///
    /// Two INTA bus cycles back to back, which is what the part runs and what
    /// an interrupt controller on the board is watching for. **Nothing in the
    /// test suite records one**: no trace contains an INTA cycle and INTR is
    /// never asserted on any cycle of any file, so unlike every other bus cycle
    /// this core drives, these are built from the manual and checked only by
    /// this crate's own tests. Table 1-16 gives the whole sequence 61 clocks
    /// and 7 transfers, of which these are two.
    fn tick_acknowledge(&mut self) {
        let Eu::Acknowledging { cycle, t } = self.eu else {
            return;
        };
        let vector = self.servicing.map_or(0, |s| s.vector);
        match t {
            1 => {
                self.bus = BusPins {
                    status: BusStatus::Inta,
                    t_state: TState::T1,
                    address: None,
                    data: None,
                    segment: None,
                };
                self.eu = Eu::Acknowledging { cycle, t: 2 };
            }
            2 => {
                self.drive_port_cycle(BusStatus::Inta, TState::T2);
                self.eu = Eu::Acknowledging { cycle, t: 3 };
            }
            3 => {
                self.drive_port_cycle(BusStatus::Inta, TState::T3);
                // The vector number is on the data pins during the second
                // cycle, put there by the interrupting device.
                if cycle == 1 {
                    self.bus.data = Some(vector);
                }
                self.eu = Eu::Acknowledging { cycle, t: 4 };
            }
            _ => {
                self.drive_port_cycle(BusStatus::Inta, TState::T4);
                self.eu = if cycle == 0 {
                    Eu::Acknowledging { cycle: 1, t: 1 }
                } else {
                    Eu::ReadingVector { byte: 0, t: 1 }
                };
            }
        }
    }

    /// Default segment for a given addressing mode base register.
    /// BP-based addressing uses SS; everything else uses DS.
    #[inline]
    pub fn default_segment_for_rm(&self, rm: u8, mod_bits: u8) -> SegReg {
        match rm & 7 {
            // [BP+SI], [BP+DI], [BP+disp]
            2 | 3 => SegReg::SS,
            // [BP] only when mod != 00 (mod=00 rm=110 is direct addressing, uses DS)
            6 if mod_bits != 0 => SegReg::SS,
            _ => SegReg::DS,
        }
    }

    /// Resolve the effective segment: use override if active, else the default.
    #[inline]
    pub fn effective_segment(&self, default: SegReg) -> u16 {
        self.get_seg(self.segment_override.unwrap_or(default))
    }
}

// ---------------------------------------------------------------------------
// Trait implementations
// ---------------------------------------------------------------------------

impl BusMasterComponent for I8088 {
    type Address = u32;
    type Data = u8;

    fn tick_with_bus<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) -> bool {
        self.execute_cycle(bus, master);
        self.retired
    }
}

impl Cpu for I8088 {
    fn reset<B: Bus<Address = Self::Address, Data = Self::Data> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.bp = 0;
        self.sp = 0;
        self.cs = 0xFFFF;
        self.ds = 0;
        self.es = 0;
        self.ss = 0;
        self.ip = 0;
        self.flags = flags::normalize(0);
        self.halted = false;
        self.opcode_at = 0;
        self.retired = false;
        self.pending_flush = false;
        // Reset flushes the instruction queue, which is exactly what the test
        // suite's setup routine relies on before it installs a queue state.
        self.flush_queue();
        self.queue_status = None;
        self.segment_override = None;
        self.rep_prefix = None;
        self.nmi_pending = false;
        self.nmi_prev = false;
        self.irq_line = false;

        // The 8088 starts executing at CS:IP = FFFF:0000 (physical 0xFFFF0).
        // Unlike 6502/6809 which read a reset vector, the 8088 simply begins
        // execution at the fixed address. The ROM at that address typically
        // contains a far JMP to the actual entry point.
        //
        // Read the first byte to verify the bus is alive (matches hardware
        // behavior of the first fetch cycle after reset).
        let _first = bus.read(master, 0xFFFF0);
        // IP stays at 0; CS stays at 0xFFFF. Execution will proceed from FFFF:0000.
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {
        // External interrupt lines are handled in check_interrupts via the bus
    }

    fn is_sleeping(&self) -> bool {
        self.halted
    }
}

// ---------------------------------------------------------------------------
// State snapshot
// ---------------------------------------------------------------------------

/// I8088 CPU state snapshot for debugging and save states.
#[derive(Debug, Clone, PartialEq)]
pub struct I8088State {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub sp: u16,
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub ip: u16,
    pub flags: u16,
}

impl CpuStateTrait for I8088 {
    type Snapshot = I8088State;

    fn snapshot(&self) -> I8088State {
        I8088State {
            ax: self.ax,
            bx: self.bx,
            cx: self.cx,
            dx: self.dx,
            si: self.si,
            di: self.di,
            bp: self.bp,
            sp: self.sp,
            cs: self.cs,
            ds: self.ds,
            es: self.es,
            ss: self.ss,
            ip: self.ip,
            flags: self.flags,
        }
    }
}

// ---------------------------------------------------------------------------
// Debug support
// ---------------------------------------------------------------------------

use crate::core::debug::{DebugRegister, Debuggable};

impl I8088State {
    pub fn debug_registers(&self) -> Vec<DebugRegister> {
        vec![
            DebugRegister {
                name: "CS:IP",
                value: ((self.cs as u64) << 16) | self.ip as u64,
                width: 32,
            },
            DebugRegister {
                name: "AX",
                value: self.ax as u64,
                width: 16,
            },
            DebugRegister {
                name: "BX",
                value: self.bx as u64,
                width: 16,
            },
            DebugRegister {
                name: "CX",
                value: self.cx as u64,
                width: 16,
            },
            DebugRegister {
                name: "DX",
                value: self.dx as u64,
                width: 16,
            },
            DebugRegister {
                name: "SI",
                value: self.si as u64,
                width: 16,
            },
            DebugRegister {
                name: "DI",
                value: self.di as u64,
                width: 16,
            },
            DebugRegister {
                name: "BP",
                value: self.bp as u64,
                width: 16,
            },
            DebugRegister {
                name: "SP",
                value: self.sp as u64,
                width: 16,
            },
            DebugRegister {
                name: "DS",
                value: self.ds as u64,
                width: 16,
            },
            DebugRegister {
                name: "ES",
                value: self.es as u64,
                width: 16,
            },
            DebugRegister {
                name: "SS",
                value: self.ss as u64,
                width: 16,
            },
            DebugRegister {
                name: "FLAGS",
                value: self.flags as u64,
                width: 16,
            },
        ]
    }
}

impl Debuggable for I8088 {
    fn debug_registers(&self) -> Vec<DebugRegister> {
        self.snapshot().debug_registers()
    }
}

impl crate::core::debug::DebugCpu for I8088 {
    fn debug_pc(&self) -> u32 {
        u32::from(self.ip)
    }

    fn debug_at_instruction_boundary(&self) -> bool {
        self.at_instruction_boundary()
    }

    fn debug_disassemble(
        &self,
        _addr: u32,
        bytes: &[u8],
    ) -> crate::cpu::disasm::DisassembledInstruction {
        // Stub disassembler: show raw opcode byte. Full x86 disassembly TBD.
        let opcode = if bytes.is_empty() { 0 } else { bytes[0] };
        crate::cpu::disasm::DisassembledInstruction {
            mnemonic: "DB",
            operands: format!("${opcode:02X}"),
            byte_len: 1,
            bytes: [opcode, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            target_addr: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A bus that answers I/O separately from memory and records what the CPU
    /// did to its ports, which is what an `IN` or `OUT` needs to be visible at
    /// all: the trait's default sends I/O to memory.
    struct PortBus {
        mem: Box<[u8; 0x10_0000]>,
        reads: Vec<u32>,
        writes: Vec<(u32, u8)>,
        answer: u8,
    }

    impl PortBus {
        fn new() -> Self {
            Self {
                mem: Box::new([0x90; 0x10_0000]),
                reads: Vec::new(),
                writes: Vec::new(),
                answer: 0,
            }
        }
    }

    impl Bus for PortBus {
        type Address = u32;
        type Data = u8;

        fn read(&mut self, _master: BusMaster, addr: u32) -> u8 {
            self.mem[(addr & 0xF_FFFF) as usize]
        }

        fn write(&mut self, _master: BusMaster, addr: u32, data: u8) {
            self.mem[(addr & 0xF_FFFF) as usize] = data;
        }

        fn io_read(&mut self, _master: BusMaster, addr: u32) -> u8 {
            self.reads.push(addr);
            self.answer
        }

        fn io_write(&mut self, _master: BusMaster, addr: u32, data: u8) {
            self.writes.push((addr, data));
        }

        fn is_halted_for(&self, _master: BusMaster) -> bool {
            false
        }

        fn check_interrupts(&mut self, _target: BusMaster) -> InterruptState {
            InterruptState::default()
        }
    }

    /// Run one instruction from `CS:IP` and collect the bus cycles it drove, as
    /// (status, address) taken off T1, which is the only T-state carrying one.
    fn run_one(cpu: &mut I8088, bus: &mut PortBus) -> Vec<(BusStatus, u32)> {
        let mut seen = Vec::new();
        for _ in 0..200 {
            let retired = cpu.tick_with_bus(bus, BusMaster::Cpu(0));
            if let Some(addr) = cpu.bus.address {
                seen.push((cpu.bus.status, addr));
            }
            if retired {
                break;
            }
        }
        seen
    }

    /// `IN AL, imm8` drives one IOR cycle at the port in its immediate, and
    /// the byte the port answered with lands in AL. The pins are the point:
    /// before M4 this instruction reached the outside world through the
    /// trait's default, which sends I/O to memory, and drove no I/O cycle at
    /// all.
    #[test]
    fn in_drives_an_io_read_cycle_at_its_port() {
        let mut cpu = I8088::new();
        let mut bus = PortBus::new();
        cpu.cs = 0;
        cpu.ip = 0x100;
        // The BIU fetches from its own pointer, which `new` leaves at zero.
        cpu.load_prefetch_queue(&[]);
        bus.answer = 0xA5;
        bus.mem[0x100] = 0xE4;
        bus.mem[0x101] = 0x42;

        let cycles = run_one(&mut cpu, &mut bus);
        assert_eq!(bus.reads, vec![0x42], "one read, at the port");
        assert_eq!(cpu.al(), 0xA5, "and its answer reaches AL");
        assert!(
            cycles.contains(&(BusStatus::IoRead, 0x42)),
            "an IOR cycle with the port on the address pins: {cycles:?}"
        );
    }

    /// `OUT DX, AX` is two IOW cycles at consecutive ports, low half first.
    #[test]
    fn a_word_out_drives_two_io_write_cycles() {
        let mut cpu = I8088::new();
        let mut bus = PortBus::new();
        cpu.cs = 0;
        cpu.ip = 0x100;
        cpu.dx = 0x0300;
        cpu.ax = 0x1234;
        cpu.load_prefetch_queue(&[]);
        bus.mem[0x100] = 0xEF;

        let cycles = run_one(&mut cpu, &mut bus);
        assert_eq!(bus.writes, vec![(0x300, 0x34), (0x301, 0x12)]);
        assert!(
            cycles.contains(&(BusStatus::IoWrite, 0x300))
                && cycles.contains(&(BusStatus::IoWrite, 0x301)),
            "two IOW cycles: {cycles:?}"
        );
    }

    /// A bus that asserts an interrupt line, so the acknowledge sequence can be
    /// watched. Nothing in the test suite records one: no trace contains an
    /// INTA cycle and neither INTR nor NMI is ever asserted, so these tests are
    /// the only check this sequence has.
    struct IrqBus {
        mem: Box<[u8; 0x10_0000]>,
        irq: bool,
        nmi: bool,
        vector: u8,
    }

    impl IrqBus {
        fn new() -> Self {
            Self {
                mem: Box::new([0x90; 0x10_0000]),
                irq: false,
                nmi: false,
                vector: 0x40,
            }
        }
    }

    impl Bus for IrqBus {
        type Address = u32;
        type Data = u8;

        fn read(&mut self, _master: BusMaster, addr: u32) -> u8 {
            self.mem[(addr & 0xF_FFFF) as usize]
        }

        fn write(&mut self, _master: BusMaster, addr: u32, data: u8) {
            self.mem[(addr & 0xF_FFFF) as usize] = data;
        }

        fn is_halted_for(&self, _master: BusMaster) -> bool {
            false
        }

        fn check_interrupts(&mut self, _target: BusMaster) -> InterruptState {
            InterruptState {
                irq: self.irq,
                nmi: self.nmi,
                irq_vector: self.vector,
                ..InterruptState::default()
            }
        }
    }

    /// Run until the CPU has transferred to the handler, collecting the bus
    /// cycles it drove and how many T-states it took.
    /// Runs to the end of the sequence rather than to the transfer: CS changes
    /// partway through, while three words are still to be pushed, so stopping
    /// there would miss half the bus cycles.
    fn service_interrupt(cpu: &mut I8088, bus: &mut IrqBus) -> (Vec<BusStatus>, usize) {
        let mut kinds = Vec::new();
        let mut ticks = 0;
        let mut started = false;
        for _ in 0..400 {
            ticks += 1;
            cpu.tick_with_bus(bus, BusMaster::Cpu(0));
            if cpu.bus.t_state == TState::T1 {
                kinds.push(cpu.bus.status);
            }
            started |= cpu.servicing.is_some();
            if started && cpu.servicing.is_none() {
                break;
            }
        }
        (kinds, ticks)
    }

    /// A maskable interrupt runs two acknowledge cycles, then reads its vector,
    /// then pushes flags, CS and IP, and the handler's address comes out of the
    /// vector table.
    #[test]
    fn a_maskable_interrupt_acknowledges_then_reads_its_vector() {
        let mut cpu = I8088::new();
        let mut bus = IrqBus::new();
        cpu.cs = 0;
        cpu.ip = 0x100;
        cpu.ss = 0;
        cpu.sp = 0x200;
        cpu.load_prefetch_queue(&[]);
        flags::set(&mut cpu.flags, flags::Flag::IF, true);
        bus.irq = true;
        // Vector 0x40 lives at 0x100 in the table: handler at 9000:1234.
        bus.mem[0x100] = 0x34;
        bus.mem[0x101] = 0x12;
        bus.mem[0x102] = 0x00;
        bus.mem[0x103] = 0x90;

        let (kinds, _) = service_interrupt(&mut cpu, &mut bus);
        assert_eq!(cpu.cs, 0x9000, "the handler's segment");
        assert_eq!(cpu.ip, 0x1234, "and its offset");
        assert_eq!(
            kinds.iter().filter(|k| **k == BusStatus::Inta).count(),
            2,
            "two acknowledge cycles: {kinds:?}"
        );
        // The four bytes of the vector, then the three words pushed.
        assert_eq!(
            kinds.iter().filter(|k| **k == BusStatus::MemRead).count(),
            4,
            "{kinds:?}"
        );
        assert_eq!(
            kinds.iter().filter(|k| **k == BusStatus::MemWrite).count(),
            6,
            "{kinds:?}"
        );
        assert_eq!(cpu.sp, 0x200 - 6, "three words deeper");
        assert!(
            !flags::get(cpu.flags, flags::Flag::IF),
            "and interrupts are off inside the handler"
        );
    }

    /// NMI runs no acknowledge cycles: nothing on the bus has to tell the part
    /// which vector it is. It is also not maskable, so a clear IF does not stop
    /// it.
    #[test]
    fn nmi_takes_no_acknowledge_and_ignores_the_interrupt_flag() {
        let mut cpu = I8088::new();
        let mut bus = IrqBus::new();
        cpu.cs = 0;
        cpu.ip = 0x400;
        cpu.ss = 0;
        cpu.sp = 0x200;
        cpu.load_prefetch_queue(&[]);
        flags::set(&mut cpu.flags, flags::Flag::IF, false);
        bus.nmi = true;
        // Vector 2 is at 0x008: handler at 7000:5678.
        bus.mem[0x008] = 0x78;
        bus.mem[0x009] = 0x56;
        bus.mem[0x00A] = 0x00;
        bus.mem[0x00B] = 0x70;

        let (kinds, _) = service_interrupt(&mut cpu, &mut bus);
        assert_eq!((cpu.cs, cpu.ip), (0x7000, 0x5678));
        assert!(
            !kinds.contains(&BusStatus::Inta),
            "no acknowledge for NMI: {kinds:?}"
        );
    }

    /// What the sequence costs. Table 1-16 gives a maskable interrupt 61 clocks
    /// and NMI 50, and this is the one number in the core that no recording can
    /// check, so it is pinned here instead.
    ///
    /// Measured from the cycle the interrupt is recognized to the cycle the
    /// handler's first byte is read, which is the same span the per-cycle gate
    /// measures for an instruction.
    #[test]
    fn an_interrupt_costs_what_the_manual_says() {
        for (nmi, documented) in [(false, 61), (true, 50)] {
            let mut cpu = I8088::new();
            let mut bus = IrqBus::new();
            cpu.cs = 0;
            cpu.ip = 0x100;
            cpu.ss = 0;
            cpu.sp = 0x200;
            cpu.load_prefetch_queue(&[]);
            flags::set(&mut cpu.flags, flags::Flag::IF, true);
            bus.irq = !nmi;
            bus.nmi = nmi;

            let mut ticks = 0;
            let mut started = false;
            for _ in 0..400 {
                cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
                if !started {
                    started = cpu.servicing.is_some();
                    if started {
                        ticks = 1;
                    }
                    continue;
                }
                ticks += 1;
                // The handler's first byte coming out of the queue ends the
                // sequence, exactly as a First Byte ends an instruction's span.
                if matches!(cpu.queue_status, Some((QueueStatus::First, _))) {
                    break;
                }
            }
            let what = if nmi { "NMI" } else { "INTR" };
            assert_eq!(ticks, documented, "{what} should take {documented} clocks");
        }
    }

    /// A long `REP MOVSB` does not hold an interrupt off until it finishes.
    ///
    /// The part recognizes one between iterations, and the count here is what
    /// says so with room to spare: 0x4000 iterations would be well over a
    /// hundred thousand clocks, and the interrupt has to be taken inside a few
    /// dozen. The pushed return address is the `REP` prefix, not the byte after
    /// the opcode, so `IRET` resumes the copy rather than dropping out of it
    /// with CX part-way down.
    #[test]
    fn a_repeated_string_operation_lets_an_interrupt_in_between_iterations() {
        let mut cpu = I8088::new();
        let mut bus = IrqBus::new();
        cpu.cs = 0;
        cpu.ip = 0x100;
        cpu.ss = 0;
        cpu.sp = 0x200;
        cpu.ds = 0;
        cpu.es = 0;
        cpu.si = 0x1000;
        cpu.di = 0x2000;
        cpu.cx = 0x4000;
        cpu.load_prefetch_queue(&[]);
        // REP MOVSB at 0000:0100.
        bus.mem[0x100] = 0xF3;
        bus.mem[0x101] = 0xA4;
        // Vector 2 at 0x008: handler at 7000:5678.
        bus.mem[0x008] = 0x78;
        bus.mem[0x009] = 0x56;
        bus.mem[0x00A] = 0x00;
        bus.mem[0x00B] = 0x70;

        // Let a few iterations run before the pin goes high, so the interrupt
        // is recognized in the middle of the repeat rather than in front of it.
        for _ in 0..60 {
            cpu.tick_with_bus(&mut bus, BusMaster::Cpu(0));
        }
        assert!(
            cpu.cx < 0x4000 && cpu.cx > 0,
            "mid-repeat: cx={:04X}",
            cpu.cx
        );
        bus.nmi = true;

        // `service_interrupt` gives up after 400 clocks, which is the whole
        // assertion: the rest of this repeat is a quarter of a million.
        let (_, ticks) = service_interrupt(&mut cpu, &mut bus);
        assert_eq!(
            (cpu.cs, cpu.ip),
            (0x7000, 0x5678),
            "the handler was never reached in {ticks} clocks"
        );
        assert!(
            cpu.cx > 0,
            "the repeat was abandoned part-way, not finished"
        );
        // Flags, CS and IP, pushed downwards from 0x200: IP is the last word.
        let pushed_ip = u16::from_le_bytes([bus.mem[0x1FA], bus.mem[0x1FB]]);
        assert_eq!(
            pushed_ip, 0x100,
            "IRET must come back to the prefix, not to the byte after the opcode"
        );
    }

    #[test]
    fn new_reset_state() {
        let cpu = I8088::new();
        assert_eq!(cpu.cs, 0xFFFF);
        assert_eq!(cpu.ip, 0x0000);
        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.ds, 0);
        assert_eq!(cpu.sp, 0);
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn flags_normalized_on_new() {
        let cpu = I8088::new();
        // Always-one bits should be set
        assert_ne!(cpu.flags & 0x0002, 0); // bit 1
        assert_eq!(cpu.flags & 0xF000, 0xF000); // bits 12-15
    }

    #[test]
    fn snapshot_round_trip() {
        let mut cpu = I8088::new();
        cpu.ax = 0x1234;
        cpu.bx = 0x5678;
        cpu.cs = 0xABCD;
        cpu.ip = 0xEF01;
        let snap = cpu.snapshot();
        assert_eq!(snap.ax, 0x1234);
        assert_eq!(snap.bx, 0x5678);
        assert_eq!(snap.cs, 0xABCD);
        assert_eq!(snap.ip, 0xEF01);
    }

    #[test]
    fn default_segment_for_bp() {
        let cpu = I8088::new();
        // rm=6 with mod=01 or mod=10 (BP-based) → SS
        assert_eq!(cpu.default_segment_for_rm(6, 1), SegReg::SS);
        assert_eq!(cpu.default_segment_for_rm(6, 2), SegReg::SS);
        // rm=6 with mod=00 → direct addressing → DS
        assert_eq!(cpu.default_segment_for_rm(6, 0), SegReg::DS);
        // rm=2 ([BP+SI]) → SS regardless of mod
        assert_eq!(cpu.default_segment_for_rm(2, 0), SegReg::SS);
        assert_eq!(cpu.default_segment_for_rm(2, 1), SegReg::SS);
        // rm=7 ([BX]) → DS
        assert_eq!(cpu.default_segment_for_rm(7, 0), SegReg::DS);
    }

    #[test]
    fn effective_segment_default() {
        let mut cpu = I8088::new();
        cpu.ds = 0x1000;
        cpu.ss = 0x2000;
        cpu.segment_override = None;
        assert_eq!(cpu.effective_segment(SegReg::DS), 0x1000);
        assert_eq!(cpu.effective_segment(SegReg::SS), 0x2000);
    }

    #[test]
    fn effective_segment_override() {
        let mut cpu = I8088::new();
        cpu.ds = 0x1000;
        cpu.es = 0x3000;
        cpu.segment_override = Some(SegReg::ES);
        // Override forces ES regardless of default
        assert_eq!(cpu.effective_segment(SegReg::DS), 0x3000);
    }

    #[test]
    fn is_sleeping_when_halted() {
        let mut cpu = I8088::new();
        assert!(!cpu.is_sleeping());
        cpu.halted = true;
        assert!(cpu.is_sleeping());
    }

    // --- The loader ---------------------------------------------------------

    /// Walk the loader by hand over the stages of one instruction, without a
    /// bus, to check that the stage machine visits what the encoding says it
    /// should. `advance_stage` reads the byte the loader just took delivery of,
    /// so pushing bytes and calling it is the whole of the interface.
    fn load(bytes: &[u8]) -> (I8088, Vec<Stage>) {
        let mut cpu = I8088::new();
        let mut seen = vec![cpu.stage];
        for &b in bytes {
            cpu.instr[cpu.instr_len as usize] = b;
            cpu.instr_len += 1;
            if cpu.advance_stage() {
                // A memory operand's immediate is deferred rather than
                // finished: the pipeline runs the operand access and sends the
                // loader back for it, which this stands in for.
                if cpu.immediate_deferred {
                    cpu.immediate_deferred = false;
                    seen.push(cpu.stage);
                    continue;
                }
                break;
            }
            seen.push(cpu.stage);
        }
        (cpu, seen)
    }

    /// The sample instruction from the test suite's own README:
    /// `add byte [ss:bp+di-64h], cl`, encoded 00 75 9C with an SS override.
    /// Opcode, ModR/M, one displacement byte, no immediate.
    #[test]
    fn the_loader_walks_opcode_modrm_and_a_byte_displacement() {
        let (cpu, seen) = load(&[0x00, 0x75, 0x9C]);
        assert_eq!(
            seen,
            vec![Stage::Opcode, Stage::Modrm, Stage::Displacement(1)]
        );
        assert_eq!(cpu.instr_len, 3, "three bytes and no more");
        assert_eq!(cpu.opcode_at, 0);
    }

    /// A prefix keeps the loader in the opcode stage and moves where the opcode
    /// lands. Getting `opcode_at` wrong would look up the format of the prefix
    /// byte instead of the instruction's.
    #[test]
    fn a_prefix_keeps_the_loader_in_the_opcode_stage() {
        // 36 = SS: override, then ADD r/m8,r8 with a direct address.
        let (cpu, seen) = load(&[0x36, 0x00, 0x06, 0x34, 0x12]);
        assert_eq!(
            seen,
            vec![
                Stage::Opcode,
                Stage::Opcode,
                Stage::Modrm,
                // The displacement counts down as its bytes arrive, so a
                // two-byte one is visible in both of its states.
                Stage::Displacement(2),
                Stage::Displacement(1),
            ]
        );
        assert_eq!(cpu.opcode_at, 1, "the opcode is behind the prefix");
        assert_eq!(cpu.instr_len, 5);
    }

    /// mod=00 rm=110 is a bare 16-bit address, so it takes two displacement
    /// bytes where every other mod=00 form takes none.
    #[test]
    fn the_direct_address_form_takes_two_displacement_bytes() {
        let (cpu, _) = load(&[0x8A, 0x06, 0x34, 0x12]);
        assert_eq!(cpu.instr_len, 4, "MOV AL, [1234h] is four bytes");
    }

    /// An instruction carrying both a ModR/M byte and an immediate has to reach
    /// the immediate stage after the displacement rather than instead of it.
    #[test]
    fn a_displacement_and_an_immediate_are_both_fetched() {
        // 81 /0 with mod=10: ADD word [bx+1234h], 5678h.
        let (cpu, seen) = load(&[0x81, 0x87, 0x34, 0x12, 0x78, 0x56]);
        assert_eq!(
            seen,
            vec![
                Stage::Opcode,
                Stage::Modrm,
                Stage::Displacement(2),
                Stage::Displacement(1),
                Stage::Immediate(2),
                Stage::Immediate(1),
            ]
        );
        assert_eq!(cpu.instr_len, 6);
    }

    /// A memory operand's immediate is left for after the operand access, and
    /// the loader says so by stopping with the stage still set to it. An
    /// immediate belonging to a *register* operand is fetched straight through,
    /// because there is no operand access to wait for.
    #[test]
    fn a_memory_operands_immediate_is_deferred_and_a_registers_is_not() {
        let mut cpu = I8088::new();
        for &b in &[0x81u8, 0x87, 0x34, 0x12] {
            cpu.instr[cpu.instr_len as usize] = b;
            cpu.instr_len += 1;
            cpu.advance_stage();
        }
        assert!(cpu.immediate_deferred, "ADD word [bx+1234h], imm16");
        assert_eq!(cpu.stage, Stage::Immediate(2), "and it is still owed");
        assert_eq!(cpu.instr_len, 4, "the loader stopped before it");

        // The same opcode with mod=11: ADD BX, imm16, which has no operand
        // access and so nothing to wait for.
        let mut reg = I8088::new();
        for &b in &[0x81u8, 0xC3] {
            reg.instr[reg.instr_len as usize] = b;
            reg.instr_len += 1;
            reg.advance_stage();
        }
        assert!(!reg.immediate_deferred, "ADD BX, imm16");
    }

    /// The unary group's immediate depends on the ModR/M reg field, which is
    /// the one place the loader has to look at a byte it already fetched to
    /// decide how many more to take.
    #[test]
    fn the_unary_group_fetches_an_immediate_only_for_test() {
        // F6 /0: TEST byte [bx], 42h. Opcode, ModR/M, immediate.
        let (test, _) = load(&[0xF6, 0x07, 0x42]);
        assert_eq!(test.instr_len, 3);

        // F6 /2: NOT byte [bx]. Opcode and ModR/M, nothing more.
        let (not, _) = load(&[0xF6, 0x17, 0x42]);
        assert_eq!(not.instr_len, 2, "NOT takes no immediate");
    }

    /// A far pointer is four bytes of immediate, and the loader must not stop
    /// after two.
    #[test]
    fn a_far_jump_fetches_all_four_pointer_bytes() {
        let (cpu, _) = load(&[0xEA, 0x00, 0x10, 0x00, 0x20]);
        assert_eq!(cpu.instr_len, 5);
    }

    /// A one-byte instruction completes on the first byte, without entering any
    /// further stage.
    #[test]
    fn a_bare_opcode_is_complete_the_moment_it_arrives() {
        let (cpu, seen) = load(&[0x90]);
        assert_eq!(seen, vec![Stage::Opcode], "no stage after the opcode");
        assert_eq!(cpu.instr_len, 1);
        assert_eq!(cpu.stage, Stage::Opcode, "reset for the next instruction");
    }

    // --- The prefetch queue -------------------------------------------------

    #[test]
    fn an_installed_queue_puts_the_prefetch_pointer_past_it() {
        let mut cpu = I8088::new();
        cpu.cs = 0x1000;
        cpu.ip = 0x0100;
        cpu.load_prefetch_queue(&[0x90, 0x91, 0x92]);

        assert_eq!(cpu.prefetch_queue(), &[0x90, 0x91, 0x92]);
        // IP still points at the first byte the EU has not consumed. The BIU
        // fetches from past the bytes already queued, or it would read them a
        // second time.
        assert_eq!(cpu.ip, 0x0100);
        assert_eq!(cpu.prefetch_ip, 0x0103);
    }

    #[test]
    #[should_panic(expected = "queue holds 4 bytes")]
    fn a_queue_longer_than_the_hardware_has_is_rejected() {
        I8088::new().load_prefetch_queue(&[0, 1, 2, 3, 4]);
    }

    /// A flush is what a taken branch costs, and it has to leave the BIU
    /// fetching from the new CS:IP rather than from wherever it had got to.
    #[test]
    fn a_flush_restarts_prefetching_at_the_new_address() {
        let mut cpu = I8088::new();
        cpu.cs = 0x1000;
        cpu.ip = 0x0100;
        cpu.load_prefetch_queue(&[0x90, 0x91, 0x92, 0x93]);
        assert_eq!(
            cpu.biu,
            Biu::Idle,
            "a full queue leaves the BIU nothing to do"
        );

        cpu.set_ip(0x0200);
        cpu.flush_queue();

        assert!(cpu.prefetch_queue().is_empty());
        assert_eq!(cpu.prefetch_ip, 0x0200);
        assert_eq!(cpu.biu, Biu::Idle);
        assert_eq!(
            cpu.queue_status,
            Some((QueueStatus::Emptied, 0)),
            "a flush is reported on QS0/QS1 as E"
        );
    }

    /// The queue is a FIFO, and the EU takes from the end the BIU is not
    /// filling. Getting this backwards would execute the instruction stream in
    /// reverse within each four bytes.
    #[test]
    fn the_queue_is_first_in_first_out() {
        let mut cpu = I8088::new();
        cpu.load_prefetch_queue(&[0x11, 0x22]);
        cpu.push_queue(0x33);

        assert_eq!(cpu.pop_queue(), 0x11);
        assert_eq!(cpu.pop_queue(), 0x22);
        assert_eq!(cpu.pop_queue(), 0x33);
        assert_eq!(cpu.queue_len, 0);
    }

    /// The BIU prefetches whenever a single byte is free, the 8088's bus being
    /// one byte wide.
    #[test]
    fn the_biu_wants_to_fetch_whenever_one_byte_is_free() {
        let mut cpu = I8088::new();
        cpu.load_prefetch_queue(&[0, 1, 2, 3]);
        assert!(!cpu.queue_has_room(), "a full queue has no room");
        cpu.pop_queue();
        assert!(cpu.queue_has_room(), "one byte free is enough");
    }

    /// Transferring control is something the instruction says, not something
    /// the address says. A taken jump with a displacement of zero lands on the
    /// address execution would have reached anyway, and the part still flushes:
    /// the hardware traces show `F`, `S`, then `E` for exactly that case.
    #[test]
    fn a_branch_to_the_next_instruction_still_counts_as_a_transfer() {
        let mut cpu = I8088::new();
        cpu.ip = 0x0100;
        cpu.transferred = false;

        cpu.set_ip(0x0100);

        assert_eq!(cpu.ip, 0x0100, "the address did not move");
        assert!(cpu.transferred, "but control was transferred");
    }
}
