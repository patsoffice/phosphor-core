//! Common disassembly types and trait for all CPU implementations.

use std::fmt;

/// Result of disassembling a single instruction.
#[derive(Debug, Clone, PartialEq)]
pub struct DisassembledInstruction {
    /// The instruction mnemonic (e.g., "LDA", "LD", "MOV")
    pub mnemonic: &'static str,
    /// Formatted operand string (e.g., "#$42", "$1234,X", "(HL)")
    pub operands: String,
    /// Total byte length of the instruction (opcode + operands)
    pub byte_len: u8,
    /// The raw bytes of the instruction (valid entries: 0..byte_len)
    pub bytes: [u8; 6],
    /// Resolved absolute address for branches, jumps, and calls
    pub target_addr: Option<u16>,
}

impl DisassembledInstruction {
    /// Format with symbol substitution. If the resolver returns a name for
    /// `target_addr`, it replaces the hex address in the output.
    pub fn format_with_symbols<'a>(&self, resolve: impl Fn(u16) -> Option<&'a str>) -> String {
        if let Some(target) = self.target_addr
            && let Some(name) = resolve(target)
        {
            let hex4 = format!("${:04X}", target);
            let substituted = self.operands.replace(&hex4, name);
            // If 4-digit hex didn't match, try 2-digit for direct/zero-page addressing
            let substituted = if substituted == self.operands && target <= 0xFF {
                let hex2 = format!("${:02X}", target);
                self.operands.replace(&hex2, name)
            } else {
                substituted
            };
            if self.operands.is_empty() {
                return self.mnemonic.to_string();
            }
            return format!("{:<6}{}", self.mnemonic, substituted);
        }
        format!("{}", self)
    }
}

impl fmt::Display for DisassembledInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.operands.is_empty() {
            write!(f, "{}", self.mnemonic)
        } else {
            write!(f, "{:<6}{}", self.mnemonic, self.operands)
        }
    }
}

/// Trait for CPU disassemblers. Takes a byte slice starting at the instruction
/// and returns a structured disassembly result.
///
/// The `bytes` slice must contain enough data to decode one instruction.
/// A minimum of 6 bytes is sufficient for all supported CPUs.
/// If the slice is too short, the disassembler returns a partial result
/// with the mnemonic set to `"???"` and `byte_len` set to 1.
pub trait Disassemble {
    /// Disassemble one instruction from the given byte slice.
    /// `addr` is the address of the first byte (needed for relative branch targets).
    fn disassemble(addr: u16, bytes: &[u8]) -> DisassembledInstruction;
}
