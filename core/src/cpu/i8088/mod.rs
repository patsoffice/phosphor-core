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

pub mod addressing;
pub mod alu;
pub mod decode;
pub mod execute;
pub mod flags;
pub(crate) mod format;
pub mod registers;

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
    /// T-state within the current fetch bus cycle, 1 through 4.
    #[save_skip(default = 1)]
    pub(crate) t: u8,

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
            t: 1,
            segment_override: None,
            rep_prefix: None,
            nmi_pending: false,
            nmi_prev: false,
            irq_line: false,
            clock: 0,
        }
    }

    /// Returns true when the CPU is at an instruction boundary: nothing loaded,
    /// and the next T-state will be the T1 of the next opcode fetch.
    pub fn at_instruction_boundary(&self) -> bool {
        !self.halted && self.instr_len == 0 && self.t == 1
    }

    /// Total T-states executed since creation.
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Execute one T-state.
    ///
    /// A T-state is one CPU clock, so a board clocking this at 5 MHz calls this
    /// five million times per emulated second. An instruction-fetch bus cycle
    /// is four of them: the address goes out on T1, the byte comes back on T3,
    /// and T4 completes the transaction. Every byte of the instruction stream
    /// costs one such cycle.
    ///
    /// What is *not* yet per-cycle: the instruction's own execution, including
    /// its operand reads and writes, still happens atomically on the T4 of its
    /// last fetched byte. That is the next step of the conversion, and until it
    /// lands this core undercounts every instruction that touches memory. See
    /// `docs/designs/cycle-accurate-i8088.md`.
    pub fn execute_cycle<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.clock += 1;

        if self.halted {
            // Check for an interrupt that can wake us.
            let ints = bus.check_interrupts(master);
            let nmi_edge = crate::cpu::flags::detect_rising_edge(ints.nmi, &mut self.nmi_prev);
            if nmi_edge {
                self.nmi_pending = true;
            }
            if self.nmi_pending {
                self.nmi_pending = false;
                self.halted = false;
                self.interrupt(bus, master, 2);
            } else if ints.irq && flags::get(self.flags, flags::Flag::IF) {
                self.halted = false;
                self.interrupt(bus, master, ints.irq_vector);
            }
            return;
        }

        // Interrupts are recognized between instructions, which is the only
        // point the loader can be redirected without discarding a partial
        // fetch.
        if self.at_instruction_boundary() {
            let ints = bus.check_interrupts(master);
            if self.handle_interrupts(ints, bus, master) {
                return;
            }
        }

        match self.t {
            // T1 puts the address on the multiplexed bus and T2 turns it
            // around for data. Neither is observable through a `Bus` that
            // resolves an access in one call, but both are real clocks and the
            // part cannot deliver a byte in fewer than four of them.
            1 | 2 => self.t += 1,
            // T3 is when the addressed device drives the byte back.
            3 => {
                let addr =
                    Self::physical_addr(self.cs, self.ip.wrapping_add(self.instr_len.into()));
                let byte = bus.read(master, addr);
                assert!(
                    (self.instr_len as usize) < MAX_INSTRUCTION,
                    "instruction longer than {MAX_INSTRUCTION} bytes at {:04X}:{:04X}",
                    self.cs,
                    self.ip
                );
                self.instr[self.instr_len as usize] = byte;
                self.instr_len += 1;
                self.t = 4;
            }
            // T4 completes the transaction. If that was the instruction's last
            // byte, it runs here.
            _ => {
                self.t = 1;
                if self.advance_stage() {
                    self.run_loaded_instruction(bus, master);
                }
            }
        }
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
        self.instr_pos = 0;
        let opcode = self.consume_prefixes();
        self.execute(opcode, bus, master);

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

        self.instr_len = 0;
        self.instr_pos = 0;
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
        self.at_instruction_boundary()
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
        self.instr_len = 0;
        self.instr_pos = 0;
        self.opcode_at = 0;
        self.stage = Stage::Opcode;
        self.t = 1;
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
}
