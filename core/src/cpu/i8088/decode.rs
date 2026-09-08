//! Intel 8088 instruction decoding.
//!
//! Handles prefix byte recognition (segment overrides, REP, LOCK) and ModR/M
//! byte parsing. The decoder consumes bytes from the instruction stream via
//! the bus and produces decoded operand information used by the execute stage.

use super::registers::SegReg;
use super::{I8088, RepPrefix};
use crate::core::{Bus, BusMaster};

// ---------------------------------------------------------------------------
// Prefix bytes
// ---------------------------------------------------------------------------

/// Prefix byte classification.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Prefix {
    SegmentOverride(SegReg),
    Rep,
    Repnz,
    Lock,
}

/// Try to classify a byte as a prefix. Returns `None` if it's not a prefix.
pub(crate) fn decode_prefix(byte: u8) -> Option<Prefix> {
    match byte {
        0x26 => Some(Prefix::SegmentOverride(SegReg::ES)),
        0x2E => Some(Prefix::SegmentOverride(SegReg::CS)),
        0x36 => Some(Prefix::SegmentOverride(SegReg::SS)),
        0x3E => Some(Prefix::SegmentOverride(SegReg::DS)),
        0xF0 => Some(Prefix::Lock),
        0xF2 => Some(Prefix::Repnz),
        0xF3 => Some(Prefix::Rep),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ModR/M byte decoding
// ---------------------------------------------------------------------------

/// Decoded ModR/M byte fields.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ModRM {
    /// Mode field (bits 7:6): 0=memory, 1=mem+disp8, 2=mem+disp16, 3=register
    pub mod_bits: u8,
    /// Register/opcode field (bits 5:3)
    pub reg: u8,
    /// Register/memory field (bits 2:0)
    pub rm: u8,
}

impl ModRM {
    /// Decode a ModR/M byte into its three fields.
    #[inline]
    pub fn decode(byte: u8) -> Self {
        Self {
            mod_bits: (byte >> 6) & 3,
            reg: (byte >> 3) & 7,
            rm: byte & 7,
        }
    }

    /// Returns true if the R/M field refers to a register (mod == 3).
    #[inline]
    pub fn is_reg(&self) -> bool {
        self.mod_bits == 3
    }
}

// ---------------------------------------------------------------------------
// Bus fetch helpers on I8088
// ---------------------------------------------------------------------------

impl I8088 {
    /// Take the next byte of the instruction and advance IP.
    ///
    /// This no longer touches the bus. The loader in [`super`] has already
    /// fetched every byte of the instruction over four T-states each, and this
    /// hands them to the executor in order. IP moves here rather than in the
    /// loader, which is what makes a partly loaded instruction safe to throw
    /// away and refetch.
    ///
    /// Panics if the executor asks for a byte the loader did not fetch, which
    /// means [`super::format`] disagrees with this instruction's implementation
    /// about how long it is. That is a bug in the table, and a loud one is much
    /// better than reading whatever was left in the buffer from the previous
    /// instruction.
    #[inline]
    pub(crate) fn fetch_byte(&mut self) -> u8 {
        assert!(
            self.instr_pos < self.instr_len,
            "opcode {:02X} consumed more bytes than the loader fetched ({})",
            self.instr[self.opcode_at as usize],
            self.instr_len,
        );
        let byte = self.instr[self.instr_pos as usize];
        self.instr_pos += 1;
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    /// Take the next 16-bit word of the instruction (little-endian).
    #[inline]
    pub(crate) fn fetch_word(&mut self) -> u16 {
        let lo = self.fetch_byte() as u16;
        let hi = self.fetch_byte() as u16;
        (hi << 8) | lo
    }

    /// Take and decode the ModR/M byte.
    #[inline]
    pub(crate) fn fetch_modrm(&mut self) -> ModRM {
        ModRM::decode(self.fetch_byte())
    }

    /// Consume all prefix bytes from the loaded instruction, updating
    /// `segment_override` and `rep_prefix`. Returns the first non-prefix
    /// opcode byte.
    pub(crate) fn consume_prefixes(&mut self) -> u8 {
        self.segment_override = None;
        self.rep_prefix = None;

        loop {
            let byte = self.fetch_byte();
            match decode_prefix(byte) {
                Some(Prefix::SegmentOverride(seg)) => {
                    self.segment_override = Some(seg);
                }
                Some(Prefix::Rep) => {
                    self.rep_prefix = Some(RepPrefix::Rep);
                }
                Some(Prefix::Repnz) => {
                    self.rep_prefix = Some(RepPrefix::Repnz);
                }
                Some(Prefix::Lock) => {
                    // LOCK is acknowledged but has no behavioral effect in
                    // our single-bus-master model.
                }
                None => return byte,
            }
        }
    }

    /// Read a 16-bit word from memory at segment:offset (little-endian).
    ///
    /// The byte-wide pair this used to sit beside is gone: the string
    /// operations were its last callers, and they reach memory through the
    /// pipeline's bus cycles now. What is left of this pair reads and writes
    /// the interrupt vector for a fault, which is the one access still made
    /// from inside the executor.
    #[inline]
    pub(crate) fn read_word<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &self,
        bus: &mut B,
        master: BusMaster,
        segment: u16,
        offset: u16,
    ) -> u16 {
        let lo = bus.read(master, Self::physical_addr(segment, offset)) as u16;
        let hi = bus.read(master, Self::physical_addr(segment, offset.wrapping_add(1))) as u16;
        (hi << 8) | lo
    }

    /// Write a 16-bit word to memory at segment:offset (little-endian).
    #[inline]
    pub(crate) fn write_word<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &self,
        bus: &mut B,
        master: BusMaster,
        segment: u16,
        offset: u16,
        data: u16,
    ) {
        bus.write(master, Self::physical_addr(segment, offset), data as u8);
        bus.write(
            master,
            Self::physical_addr(segment, offset.wrapping_add(1)),
            (data >> 8) as u8,
        );
    }

    /// Push a 16-bit value onto the stack (SS:SP).
    #[inline]
    /// Push a 16-bit value onto the stack (SS:SP).
    ///
    /// Staged rather than written, when the pipeline predicted this push: the
    /// word goes into `stack_words` and the write-back phase sends it out over
    /// MEMW bus cycles once the instruction has finished. SP still moves here,
    /// so an instruction that pushes several words leaves it where it should
    /// be and the pipeline knows every address.
    ///
    /// Falls back to writing directly when the push was *not* predicted, which
    /// is the conditional set: `INTO` when the overflow flag is set, and the
    /// divide error inside `DIV`, `IDIV` and `AAM`. Those cost no bus cycles
    /// yet, which undercounts them, and they are declared unmodeled for timing
    /// rather than pretended about.
    pub(crate) fn push16<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
        value: u16,
    ) {
        self.stack_ops.1 += 1;
        self.sp = self.sp.wrapping_sub(2);
        if (self.stack_pos as usize) < self.stack_words.len() && self.stack_staged {
            self.stack_words[self.stack_pos as usize] = value;
            self.stack_pos += 1;
        } else {
            self.write_word(bus, master, self.ss, self.sp, value);
        }
    }

    /// Pop a 16-bit value from the stack (SS:SP).
    #[inline]
    /// Pop a 16-bit value from the stack (SS:SP).
    ///
    /// Taken from `stack_words` when the pipeline predicted this pop and read
    /// it over MEMR bus cycles beforehand. See [`Self::push16`] for the
    /// unpredicted case.
    pub(crate) fn pop16<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) -> u16 {
        self.stack_ops.0 += 1;
        let val = if (self.stack_pos as usize) < self.stack_words.len() && self.stack_staged {
            let v = self.stack_words[self.stack_pos as usize];
            self.stack_pos += 1;
            v
        } else {
            self.read_word(bus, master, self.ss, self.sp)
        };
        self.sp = self.sp.wrapping_add(2);
        val
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modrm_decode_reg_mode() {
        // mod=11 reg=010 rm=001 = 0b11_010_001 = 0xD1
        let m = ModRM::decode(0xD1);
        assert_eq!(m.mod_bits, 3);
        assert_eq!(m.reg, 2);
        assert_eq!(m.rm, 1);
        assert!(m.is_reg());
    }

    #[test]
    fn modrm_decode_mem_mode() {
        // mod=00 reg=111 rm=110 = 0b00_111_110 = 0x3E
        let m = ModRM::decode(0x3E);
        assert_eq!(m.mod_bits, 0);
        assert_eq!(m.reg, 7);
        assert_eq!(m.rm, 6);
        assert!(!m.is_reg());
    }

    #[test]
    fn modrm_decode_disp8_mode() {
        // mod=01 reg=000 rm=100 = 0b01_000_100 = 0x44
        let m = ModRM::decode(0x44);
        assert_eq!(m.mod_bits, 1);
        assert_eq!(m.reg, 0);
        assert_eq!(m.rm, 4);
        assert!(!m.is_reg());
    }

    #[test]
    fn modrm_decode_disp16_mode() {
        // mod=10 reg=101 rm=011 = 0b10_101_011 = 0xAB
        let m = ModRM::decode(0xAB);
        assert_eq!(m.mod_bits, 2);
        assert_eq!(m.reg, 5);
        assert_eq!(m.rm, 3);
        assert!(!m.is_reg());
    }

    #[test]
    fn decode_prefix_segment_overrides() {
        assert_eq!(
            decode_prefix(0x26),
            Some(Prefix::SegmentOverride(SegReg::ES))
        );
        assert_eq!(
            decode_prefix(0x2E),
            Some(Prefix::SegmentOverride(SegReg::CS))
        );
        assert_eq!(
            decode_prefix(0x36),
            Some(Prefix::SegmentOverride(SegReg::SS))
        );
        assert_eq!(
            decode_prefix(0x3E),
            Some(Prefix::SegmentOverride(SegReg::DS))
        );
    }

    #[test]
    fn decode_prefix_rep_lock() {
        assert_eq!(decode_prefix(0xF0), Some(Prefix::Lock));
        assert_eq!(decode_prefix(0xF2), Some(Prefix::Repnz));
        assert_eq!(decode_prefix(0xF3), Some(Prefix::Rep));
    }

    #[test]
    fn decode_prefix_non_prefix() {
        assert_eq!(decode_prefix(0x90), None); // NOP
        assert_eq!(decode_prefix(0x00), None); // ADD
        assert_eq!(decode_prefix(0xFF), None);
    }

    #[test]
    fn modrm_all_reg_encodings() {
        // Verify all 8 reg encodings are decoded correctly
        for reg in 0..8u8 {
            let byte = 0xC0 | (reg << 3); // mod=11 reg=N rm=0
            let m = ModRM::decode(byte);
            assert_eq!(m.reg, reg);
            assert_eq!(m.mod_bits, 3);
            assert_eq!(m.rm, 0);
        }
    }

    #[test]
    fn modrm_all_rm_encodings() {
        // Verify all 8 rm encodings are decoded correctly
        for rm in 0..8u8 {
            let byte = rm; // mod=00 reg=0 rm=N
            let m = ModRM::decode(byte);
            assert_eq!(m.rm, rm);
            assert_eq!(m.mod_bits, 0);
            assert_eq!(m.reg, 0);
        }
    }
}
