//! Debug inspection trait for interactive debugging of emulated machines.
//!
//! Machines that implement `Debuggable` expose cycle-level stepping, register
//! inspection, side-effect-free memory reads, and disassembly — everything
//! needed for a frontend debugger UI.

/// A single CPU register for display in the debug panel.
pub struct DebugRegister {
    /// Register name (e.g., "A", "PC", "SP").
    pub name: &'static str,
    /// Register value (all register widths fit in u64).
    pub value: u64,
    /// Display width in bits (8 or 16).
    pub width: u8,
}

/// Result of disassembling one instruction at a given address.
pub struct DebugDisassembly {
    /// Address of the instruction.
    pub addr: u16,
    /// Raw instruction bytes.
    pub bytes: Vec<u8>,
    /// Formatted instruction text (e.g., "LDA  #$42").
    pub text: String,
    /// Total byte length of the instruction.
    pub byte_len: u8,
}

/// Trait for machines that support interactive debugging.
///
/// Provides cycle-level and instruction-level stepping, register/memory
/// inspection, and disassembly. Machines implement this alongside `Machine`.
///
/// The frontend accesses this via `Machine::as_debuggable()`.
pub trait Debuggable {
    /// Advance exactly one CPU cycle. Returns true if an instruction
    /// boundary was crossed (the CPU is ready to fetch the next opcode).
    fn debug_tick(&mut self) -> bool;

    /// Read the current program counter.
    fn debug_pc(&self) -> u16;

    /// Get CPU registers for display.
    fn debug_registers(&self) -> Vec<DebugRegister>;

    /// Read a byte from the address space without side effects.
    ///
    /// Unlike `Bus::read()`, this must not trigger hardware behavior
    /// (PIA register clears, watchdog resets, etc.). Returns `None`
    /// for unmapped addresses.
    fn debug_read(&self, addr: u16) -> Option<u8>;

    /// Write a byte to the address space (for the memory editor).
    fn debug_write(&mut self, addr: u16, data: u8);

    /// Disassemble `count` instructions starting at `addr`.
    fn debug_disassemble(&self, addr: u16, count: usize) -> Vec<DebugDisassembly>;

    /// Number of CPUs in this machine (e.g., 2 for Joust: M6809 + M6800).
    fn debug_cpu_count(&self) -> usize {
        1
    }

    /// Select which CPU to inspect (0-based index).
    fn debug_select_cpu(&mut self, _index: usize) {}

    /// Human-readable name of the currently selected CPU (e.g., "M6809 Main").
    fn debug_cpu_name(&self) -> &str {
        "CPU"
    }
}
