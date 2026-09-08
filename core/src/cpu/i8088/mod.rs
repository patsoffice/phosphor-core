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

/// Idle cycles between the queue gaining room and the T1 of the fetch that
/// refills it.
///
/// From the test suite's README, which states it as an observable rather than
/// as a design note: "It takes two cycles to begin a fetch after reading from a
/// full queue, therefore tests that specify an initial queue state will start
/// with two 'Ti' cycle states." The sample trace bears that out exactly, with
/// queue reads on cycles 0 and 1 and T1 on cycle 2.
///
/// This applies only to restarting from idle. A fetch that ends with room still
/// in the queue is followed immediately by the next T1, back to back with no
/// idle cycle between, which the same trace shows at cycles 5 and 6.
const PREFETCH_RESTART_CYCLES: u8 = 2;

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

/// The bus interface unit's prefetch state machine, which runs alongside the
/// EU and independently of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Biu {
    /// Not fetching, because the queue is full.
    #[default]
    Idle,
    /// The queue has room and the BIU is counting the idle cycles before it can
    /// drive T1. See [`PREFETCH_RESTART_CYCLES`].
    Restarting(u8),
    /// Inside a CODE bus cycle: `t` is 1 through 4, `addr` the address latched
    /// on T1. The address is held here rather than recomputed, because the EU
    /// consuming bytes while a fetch is in flight must not move an address the
    /// BIU has already put on the pins.
    Fetching { t: u8, addr: u32 },
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
    /// Running the instruction's microcode, with the clocks still to go.
    ///
    /// The bus is free throughout, so the BIU prefetches through it. That is
    /// what the recorded traces show the hardware doing between an operand read
    /// and its write-back, and it is why this phase sits where it does.
    Executing(u8),
    /// Writing the memory operand back after the instruction has run.
    Writing { byte: u8, total: u8, t: u8 },
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
            stack_staged: false,
            stack_base: 0,
            operand_ops: (0, 0),
            stack_ops: (0, 0),
            retired: false,
            transferred: false,
            pending_flush: false,
            queue: [0; QUEUE_LEN],
            queue_len: 0,
            prefetch_ip: 0,
            biu: Biu::Idle,
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

        // A branch taken on the previous T-state flushes here, on its own
        // cycle, and only then has the instruction retired.
        if self.pending_flush {
            self.pending_flush = false;
            self.flush_queue();
            self.retired = true;
            return;
        }

        if self.halted {
            // A halted 8088 stops prefetching, so the BIU does not run here.
            let ints = bus.check_interrupts(master);
            let nmi_edge = crate::cpu::flags::detect_rising_edge(ints.nmi, &mut self.nmi_prev);
            if nmi_edge {
                self.nmi_pending = true;
            }
            if self.nmi_pending {
                self.nmi_pending = false;
                self.halted = false;
                self.interrupt(bus, master, 2);
                self.flush_queue();
            } else if ints.irq && flags::get(self.flags, flags::Flag::IF) {
                self.halted = false;
                self.interrupt(bus, master, ints.irq_vector);
                self.flush_queue();
            }
            return;
        }

        // Interrupts are recognized between instructions, which is the only
        // point the queue can be redirected without discarding a partial fetch.
        if self.at_instruction_boundary() {
            let ints = bus.check_interrupts(master);
            if self.handle_interrupts(ints, bus, master) {
                self.flush_queue();
                return;
            }
        }

        match self.eu {
            Eu::Loading => {
                self.tick_eu(bus, master);
                self.tick_biu(bus, master);
            }
            // Address arithmetic uses no bus, so the BIU runs alongside it.
            Eu::AddressCalc(remaining) => {
                self.eu = if remaining > 1 {
                    Eu::AddressCalc(remaining - 1)
                } else {
                    self.begin_operand_phase()
                };
                self.tick_biu(bus, master);
                // An instruction that neither reads its operand nor has any
                // modeled execution time runs here. Dropping this made the
                // pipeline fall back to Loading without ever executing, so the
                // loader started a fresh instruction on top of the old one.
                if self.eu == Eu::Loading {
                    self.run_execute_step(bus, master);
                }
            }
            // Nor does microcode. This is the phase the hardware traces show
            // the BIU prefetching through.
            Eu::Executing(remaining) => {
                if remaining > 1 {
                    self.eu = Eu::Executing(remaining - 1);
                    self.tick_biu(bus, master);
                } else {
                    self.eu = Eu::Loading;
                    self.tick_biu(bus, master);
                    self.run_execute_step(bus, master);
                }
            }
            // There is one bus, and while the EU is using it the BIU cannot
            // prefetch. That contention is not incidental: it is why an
            // instruction with a memory operand leaves the queue emptier than
            // one without, and why the instruction after it may then stall.
            Eu::Reading { .. } => self.tick_operand_read(bus, master),
            Eu::Writing { .. } => self.tick_operand_write(bus, master),
            Eu::PoppingStack { .. } => self.tick_stack(bus, master, true),
            Eu::PushingStack { .. } => self.tick_stack(bus, master, false),
        }
    }

    /// The execution unit's cycle: take one byte from the queue, if there is
    /// one and the EU still wants one.
    fn tick_eu<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        if self.queue_len == 0 {
            // Starved. The EU idles until the BIU delivers, which is the cost
            // the prefetch queue exists to avoid and the reason a jump is
            // expensive.
            return;
        }

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

        if self.advance_stage() {
            self.run_loaded_instruction(bus, master);
        }
    }

    /// The bus interface unit's cycle: keep the queue full.
    fn tick_biu<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        // `Biu::Fetching { t }` is the T-state driven on *this* cycle, and
        // `Restarting(0)` means "drive T1 next time you are ticked". Written
        // any other way a state transition eats a clock: the first version of
        // this spent a cycle moving from `Restarting(0)` into the fetch before
        // driving T1, which made every queue refill five cycles long instead of
        // four. Nothing caught it, because M2 compared the order of queue
        // operations and not their position.
        match self.biu {
            Biu::Idle => {
                // Ti. Start counting the moment there is somewhere to put a
                // byte.
                if self.queue_has_room() {
                    self.biu = Biu::Restarting(PREFETCH_RESTART_CYCLES - 1);
                }
            }
            // Ti, waiting out the restart delay.
            Biu::Restarting(n) if n > 0 => self.biu = Biu::Restarting(n - 1),
            // The delay is spent: drive T1 now.
            Biu::Restarting(_) => {
                let addr = Self::physical_addr(self.cs, self.prefetch_ip);
                self.begin_bus_cycle(BusStatus::Code, addr, SegReg::CS);
                self.biu = Biu::Fetching { t: 2, addr };
            }
            // T2 turns the multiplexed pins around for data. The address is off
            // them by now, which is what the external latch exists for.
            Biu::Fetching { t: 2, addr } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T2, SegReg::CS);
                self.biu = Biu::Fetching { t: 3, addr };
            }
            // T3: the addressed device drives the byte back.
            Biu::Fetching { t: 3, addr } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T3, SegReg::CS);
                let byte = bus.read(master, addr);
                self.bus.data = Some(byte);
                self.push_queue(byte);
                self.prefetch_ip = self.prefetch_ip.wrapping_add(1);
                self.biu = Biu::Fetching { t: 4, addr };
            }
            // T4 completes the transaction. A fetch that ends with room left in
            // the queue runs straight into the next T1, back to back, with no
            // idle cycle between: `Restarting(0)` drives T1 on the very next
            // tick.
            Biu::Fetching { .. } => {
                self.drive_bus_cycle(BusStatus::Code, TState::T4, SegReg::CS);
                self.biu = if self.queue_has_room() {
                    Biu::Restarting(0)
                } else {
                    Biu::Idle
                };
            }
        }
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
        // A full queue leaves the BIU with nothing to do; a partial one lets it
        // start counting down to its next fetch.
        self.biu = Biu::Idle;
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
        self.biu = Biu::Idle;
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
    fn begin_immediate(&mut self, imm: format::Imm, modrm: Option<u8>) -> bool {
        match imm.len(modrm) {
            0 => {
                self.stage = Stage::Opcode;
                true
            }
            n => {
                self.stage = Stage::Immediate(n);
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
            access::operand_access(opcode, 0)
        };
        self.ip = saved_ip;
        self.instr_pos = 0;

        // A memory operand has to have its address worked out before anything
        // can be done with it, and that arithmetic takes the EU real clocks.
        // The instruction does not run on this cycle at all: the pipeline goes
        // through its address, read and write phases and comes back through
        // `run_execute_step`.
        if self.operand_at.is_some() {
            let modrm = self.instr[self.opcode_at as usize + 1];
            let cycles = access::ea_cycles(modrm, self.segment_override.is_some());
            self.eu = Eu::AddressCalc(cycles);
            return;
        }

        let _ = operand_access;
        self.eu = self.begin_stack_pop_phase();
        if self.eu == Eu::Loading {
            self.run_execute_step(bus, master);
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
            self.begin_stack_pop_phase()
        }
    }

    /// Enter the stack-pop phase if this instruction takes words off the stack,
    /// otherwise go on to the microcode.
    ///
    /// Pops come after any operand read and before execution, which is the
    /// order the instructions need: `POP [mem]` takes its word off the stack
    /// and then writes the operand, and an indirect far `CALL` reads its
    /// pointer operand before pushing anything.
    fn begin_stack_pop_phase(&mut self) -> Eu {
        let stack = access::stack_access(self.opcode(), self.instr[self.opcode_at as usize + 1]);
        self.stack_staged = true;
        self.stack_pos = 0;
        match stack.pops {
            0 => self.begin_execute_phase(),
            total => Eu::PoppingStack {
                word: 0,
                total,
                byte: 0,
                t: 1,
            },
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
    /// Prefix bytes are not subtracted, because the manual's counts do not
    /// include them. A segment override's cost is the two extra clocks
    /// [`access::ea_cycles`] already adds.
    ///
    /// A shift or rotate by CL adds four clocks a bit on top of its base. That
    /// is the one form whose cost depends on a register rather than on the
    /// encoding, so it is added here, where CL is in hand.
    fn begin_execute_phase(&mut self) -> Eu {
        let opcode = self.opcode();
        let modrm = self.instr[self.opcode_at as usize + 1];
        let mut cycles = i32::from(timing::eu_cycles(opcode, modrm));
        if matches!(opcode, 0xD2 | 0xD3) {
            cycles += i32::from(timing::shift_count_cycles(self.cl()));
        }
        cycles -= i32::from(self.instr_len - self.opcode_at);

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
        self.instr_pos = 0;
        self.operand_ops = (0, 0);
        self.stack_ops = (0, 0);
        let opcode = self.consume_prefixes();
        self.execute(opcode, bus, master);

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
        // Only instructions with a ModR/M byte addressing *memory* are checked.
        // The others reach memory through push, pop, the string moves and the
        // interrupt vector reads, which this table does not describe and the
        // pipeline will have to handle separately; and a register operand
        // costs no bus cycle either way, which is what lets `PUSH SP` take its
        // own path through the 8088's push-the-decremented-value quirk without
        // looking like a disagreement.
        #[cfg(debug_assertions)]
        if format::format_of(opcode).modrm && self.instr[self.opcode_at as usize + 1] >> 6 != 3 {
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

        // Words the instruction pushed go out first, then any operand
        // write-back. That order matters for `PUSH [mem]`, which reads its
        // operand and pushes it, and it is the order the recorded traces show.
        if self.stack_staged && self.stack_pos > 0 && self.stack_ops.1 > 0 {
            let total = self.stack_pos;
            self.stack_pos = 0;
            self.eu = Eu::PushingStack {
                word: 0,
                total,
                byte: 0,
                t: 1,
            };
            return;
        }

        self.finish_or_write_operand();
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
                    // The operand is in hand; the stack comes next, then the
                    // microcode.
                    self.eu = self.begin_stack_pop_phase();
                    if self.eu == Eu::Loading {
                        self.run_execute_step(bus, master);
                    }
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
                self.eu = rebuild(byte, 4);
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
                } else if popping {
                    // Everything the instruction will pop is in hand.
                    self.stack_pos = 0;
                    self.eu = self.begin_execute_phase();
                    if self.eu == Eu::Loading {
                        self.run_execute_step(bus, master);
                    }
                } else {
                    // Nothing in this instruction set both pushes and writes a
                    // memory operand, but the write-back is checked rather than
                    // assumed away: losing one silently would be a wrong answer
                    // rather than a wrong cycle count.
                    self.finish_or_write_operand();
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
                self.eu = Eu::Writing { byte, total, t: 4 };
            }
            _ => {
                self.drive_bus_cycle(BusStatus::MemWrite, TState::T4, self.operand_segment());
                if byte + 1 < total {
                    self.eu = Eu::Writing {
                        byte: byte + 1,
                        total,
                        t: 1,
                    };
                } else {
                    self.finish_instruction();
                }
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

    /// Check for pending interrupts. Returns true if an interrupt was taken.
    fn handle_interrupts<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        ints: InterruptState,
        bus: &mut B,
        master: BusMaster,
    ) -> bool {
        // NMI is edge-triggered
        let nmi_edge = crate::cpu::flags::detect_rising_edge(ints.nmi, &mut self.nmi_prev);
        if nmi_edge {
            self.nmi_pending = true;
        }

        // NMI takes priority over IRQ
        if self.nmi_pending {
            self.nmi_pending = false;
            self.interrupt(bus, master, 2); // NMI = vector 2
            return true;
        }

        // IRQ: level-triggered, masked by IF
        if ints.irq && flags::get(self.flags, flags::Flag::IF) {
            self.interrupt(bus, master, ints.irq_vector);
            return true;
        }

        false
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
