//! Intel 8088 addressing mode resolution.
//!
//! The 8088 uses a ModR/M byte to encode operand addressing. There are 8
//! base memory addressing modes (determined by the R/M field when mod != 11),
//! each with optional 8-bit or 16-bit displacement. When mod == 11, the R/M
//! field selects a register directly.
//!
//! The 8 memory modes (mod=00, no displacement unless direct):
//!
//! ```text
//! rm=000: [BX+SI]
//! rm=001: [BX+DI]
//! rm=010: [BP+SI]  (default segment SS)
//! rm=011: [BP+DI]  (default segment SS)
//! rm=100: [SI]
//! rm=101: [DI]
//! rm=110: [disp16] (direct addressing; default segment DS)
//! rm=111: [BX]
//! ```
//!
//! mod=01: add sign-extended 8-bit displacement
//! mod=10: add 16-bit displacement
//! mod=11: register operand (not a memory address)

use super::I8088;
use super::decode::ModRM;
use super::registers::SegReg;
use crate::core::{Bus, BusMaster};

/// A resolved operand location: either a memory address (segment:offset) or a
/// register encoding.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Operand {
    /// Memory operand at segment:offset.
    Memory { segment: u16, offset: u16 },
    /// Register operand (the rm field, used with get_reg8/get_reg16).
    Register(u8),
}

impl I8088 {
    /// Resolve a ModR/M byte into an `Operand`, fetching any displacement
    /// bytes from the instruction stream.
    ///
    /// For mod=11, returns `Operand::Register(rm)`.
    /// For mod=00/01/10, computes the effective address and returns
    /// `Operand::Memory { segment, offset }`.
    pub(crate) fn resolve_modrm(&mut self, modrm: ModRM) -> Operand {
        if modrm.is_reg() {
            return Operand::Register(modrm.rm);
        }

        // Compute base effective address from R/M field
        let (base_offset, default_seg) = self.compute_ea_base(modrm.rm, modrm.mod_bits);

        // Add displacement
        let offset = match modrm.mod_bits {
            0 => base_offset,
            1 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte() as i8 as i16 as u16;
                base_offset.wrapping_add(disp)
            }
            2 => {
                // 16-bit displacement
                let disp = self.fetch_word();
                base_offset.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        let segment = self.effective_segment(default_seg);
        Operand::Memory { segment, offset }
    }

    /// Compute the base effective address for a given R/M field and mod bits.
    /// Returns (offset, default_segment).
    ///
    /// Special case: mod=00, rm=110 is direct addressing — the offset is a
    /// 16-bit immediate fetched from the instruction stream.
    fn compute_ea_base(&mut self, rm: u8, mod_bits: u8) -> (u16, SegReg) {
        match rm & 7 {
            0 => (self.bx.wrapping_add(self.si), SegReg::DS), // [BX+SI]
            1 => (self.bx.wrapping_add(self.di), SegReg::DS), // [BX+DI]
            2 => (self.bp.wrapping_add(self.si), SegReg::SS), // [BP+SI]
            3 => (self.bp.wrapping_add(self.di), SegReg::SS), // [BP+DI]
            4 => (self.si, SegReg::DS),                       // [SI]
            5 => (self.di, SegReg::DS),                       // [DI]
            6 => {
                if mod_bits == 0 {
                    // Direct addressing: 16-bit offset from instruction stream
                    let offset = self.fetch_word();
                    (offset, SegReg::DS)
                } else {
                    // [BP+disp] — displacement is added by caller
                    (self.bp, SegReg::SS)
                }
            }
            7 => (self.bx, SegReg::DS), // [BX]
            _ => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Operand read/write helpers
    // -----------------------------------------------------------------------

    /// Read an 8-bit value from a resolved operand.
    pub(crate) fn read_operand8<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &self,
        operand: Operand,
        bus: &mut B,
        master: BusMaster,
    ) -> u8 {
        match operand {
            Operand::Register(rm) => self.get_reg8(rm),
            Operand::Memory { segment, offset } => self.read_byte(bus, master, segment, offset),
        }
    }

    /// Write an 8-bit value to a resolved operand.
    pub(crate) fn write_operand8<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        operand: Operand,
        bus: &mut B,
        master: BusMaster,
        val: u8,
    ) {
        match operand {
            Operand::Register(rm) => self.set_reg8(rm, val),
            Operand::Memory { segment, offset } => {
                self.write_byte(bus, master, segment, offset, val);
            }
        }
    }

    /// Read a 16-bit value from a resolved operand.
    pub(crate) fn read_operand16<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &self,
        operand: Operand,
        bus: &mut B,
        master: BusMaster,
    ) -> u16 {
        match operand {
            Operand::Register(rm) => self.get_reg16(rm),
            Operand::Memory { segment, offset } => self.read_word(bus, master, segment, offset),
        }
    }

    /// Write a 16-bit value to a resolved operand.
    pub(crate) fn write_operand16<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        operand: Operand,
        bus: &mut B,
        master: BusMaster,
        val: u16,
    ) {
        match operand {
            Operand::Register(rm) => self.set_reg16(rm, val),
            Operand::Memory { segment, offset } => {
                self.write_word(bus, master, segment, offset, val);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bus::InterruptState;
    use crate::cpu::i8088::RepPrefix;

    /// Minimal test bus for addressing tests.
    struct TestBus {
        mem: Box<[u8; 0x10_0000]>, // 1 MB (heap-allocated)
    }

    impl TestBus {
        fn new() -> Self {
            Self {
                mem: Box::new([0; 0x10_0000]),
            }
        }
    }

    impl Bus for TestBus {
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
            InterruptState::default()
        }
    }

    fn setup() -> (I8088, TestBus) {
        let mut cpu = I8088::new();
        cpu.cs = 0x0000;
        cpu.ip = 0x0100;
        cpu.ds = 0x2000;
        cpu.ss = 0x3000;
        cpu.es = 0x4000;
        (cpu, TestBus::new())
    }

    const MASTER: BusMaster = BusMaster::Cpu(0);

    /// Stage displacement bytes as though the loader had already fetched them.
    ///
    /// Address resolution no longer reads the instruction stream off the bus:
    /// the loader in [`super::super`] fetches every byte of the instruction
    /// over four T-states each, and `resolve_modrm` takes its displacement out
    /// of that buffer. These tests call `resolve_modrm` on its own, so the
    /// buffer holds the displacement alone with nothing consumed ahead of it.
    fn stage(cpu: &mut I8088, bytes: &[u8]) {
        cpu.instr[..bytes.len()].copy_from_slice(bytes);
        cpu.instr_len = bytes.len() as u8;
        cpu.instr_pos = 0;
    }

    // -- mod=11 register mode --

    #[test]
    fn resolve_reg_mode() {
        let (mut cpu, _bus) = setup();
        let modrm = ModRM {
            mod_bits: 3,
            reg: 0,
            rm: 5,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(op, Operand::Register(5));
    }

    // -- mod=00 memory modes --

    #[test]
    fn resolve_bx_si() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0100;
        cpu.si = 0x0050;
        // mod=00 rm=000 → [BX+SI]
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 0,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0150,
            }
        );
    }

    #[test]
    fn resolve_bx_di() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0200;
        cpu.di = 0x0030;
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 1,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0230,
            }
        );
    }

    #[test]
    fn resolve_bp_si_uses_ss() {
        let (mut cpu, _bus) = setup();
        cpu.bp = 0x0100;
        cpu.si = 0x0010;
        // rm=010 → [BP+SI], default SS
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 2,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x3000,
                offset: 0x0110,
            }
        );
    }

    #[test]
    fn resolve_bp_di_uses_ss() {
        let (mut cpu, _bus) = setup();
        cpu.bp = 0x0200;
        cpu.di = 0x0020;
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 3,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x3000,
                offset: 0x0220,
            }
        );
    }

    #[test]
    fn resolve_si() {
        let (mut cpu, _bus) = setup();
        cpu.si = 0x0300;
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 4,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0300,
            }
        );
    }

    #[test]
    fn resolve_di() {
        let (mut cpu, _bus) = setup();
        cpu.di = 0x0400;
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 5,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0400,
            }
        );
    }

    #[test]
    fn resolve_direct_addressing() {
        let (mut cpu, _bus) = setup();
        // mod=00 rm=110 → direct address, a 16-bit offset in the instruction
        // stream rather than a displacement.
        stage(&mut cpu, &[0x34, 0x12]);

        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 6,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x1234,
            }
        );
        // IP advances as the executor consumes the bytes, not as the loader
        // fetches them, so it moves by two here.
        assert_eq!(cpu.ip, 0x0102);
        assert_eq!(cpu.instr_pos, 2, "both offset bytes were consumed");
    }

    #[test]
    fn resolve_bx() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0500;
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 7,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0500,
            }
        );
    }

    // -- mod=01 8-bit displacement --

    #[test]
    fn resolve_bx_si_disp8() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0100;
        cpu.si = 0x0050;
        stage(&mut cpu, &[0x10]); // signed displacement +0x10

        let modrm = ModRM {
            mod_bits: 1,
            reg: 0,
            rm: 0,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0160, // 0x100 + 0x50 + 0x10
            }
        );
    }

    #[test]
    fn resolve_bp_disp8_uses_ss() {
        let (mut cpu, _bus) = setup();
        cpu.bp = 0x0200;
        // mod=01 rm=110 → [BP+disp8], uses SS
        stage(&mut cpu, &[0x04]);

        let modrm = ModRM {
            mod_bits: 1,
            reg: 0,
            rm: 6,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x3000,
                offset: 0x0204,
            }
        );
    }

    #[test]
    fn resolve_disp8_negative() {
        let (mut cpu, _bus) = setup();
        cpu.si = 0x0100;
        // Signed displacement -0x10 (= 0xF0 as i8)
        stage(&mut cpu, &[0xF0]);

        let modrm = ModRM {
            mod_bits: 1,
            reg: 0,
            rm: 4, // [SI+disp8]
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x00F0, // 0x100 + (-0x10) = 0x100 - 0x10 = 0xF0
            }
        );
    }

    // -- mod=10 16-bit displacement --

    #[test]
    fn resolve_bx_disp16() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0100;
        stage(&mut cpu, &[0x34, 0x12]); // 16-bit displacement, little-endian

        let modrm = ModRM {
            mod_bits: 2,
            reg: 0,
            rm: 7, // [BX+disp16]
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x1334, // 0x100 + 0x1234
            }
        );
    }

    #[test]
    fn resolve_bp_disp16_uses_ss() {
        let (mut cpu, _bus) = setup();
        cpu.bp = 0x0500;
        stage(&mut cpu, &[0x02, 0x00]);

        let modrm = ModRM {
            mod_bits: 2,
            reg: 0,
            rm: 6, // [BP+disp16]
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x3000,
                offset: 0x0502,
            }
        );
    }

    // -- Segment override --

    #[test]
    fn resolve_with_segment_override() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0x0100;
        cpu.segment_override = Some(SegReg::ES);

        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 7, // [BX]
        };
        let op = cpu.resolve_modrm(modrm);
        // Should use ES (0x4000) instead of default DS (0x2000)
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x4000,
                offset: 0x0100,
            }
        );
    }

    #[test]
    fn resolve_bp_with_cs_override() {
        let (mut cpu, _bus) = setup();
        cpu.bp = 0x0100;
        cpu.si = 0x0010;
        cpu.cs = 0x5000;
        cpu.segment_override = Some(SegReg::CS);

        // rm=010 → [BP+SI], normally SS, but CS override
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 2,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x5000,
                offset: 0x0110,
            }
        );
    }

    // -- Operand read/write --

    #[test]
    fn read_write_operand8_register() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x0042;
        let op = Operand::Register(0); // AL
        assert_eq!(cpu.read_operand8(op, &mut bus, MASTER), 0x42);

        cpu.write_operand8(op, &mut bus, MASTER, 0xFF);
        assert_eq!(cpu.al(), 0xFF);
    }

    #[test]
    fn read_write_operand8_memory() {
        let (mut cpu, mut bus) = setup();
        let op = Operand::Memory {
            segment: 0x2000,
            offset: 0x0050,
        };
        // Write then read
        cpu.write_operand8(op, &mut bus, MASTER, 0xAB);
        assert_eq!(cpu.read_operand8(op, &mut bus, MASTER), 0xAB);
        // Verify at physical address 0x20050
        assert_eq!(bus.mem[0x20050], 0xAB);
    }

    #[test]
    fn read_write_operand16_register() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x1234;
        let op = Operand::Register(3); // BX
        assert_eq!(cpu.read_operand16(op, &mut bus, MASTER), 0x1234);

        cpu.write_operand16(op, &mut bus, MASTER, 0xABCD);
        assert_eq!(cpu.bx, 0xABCD);
    }

    #[test]
    fn read_write_operand16_memory() {
        let (mut cpu, mut bus) = setup();
        let op = Operand::Memory {
            segment: 0x2000,
            offset: 0x0060,
        };
        cpu.write_operand16(op, &mut bus, MASTER, 0x1234);
        assert_eq!(cpu.read_operand16(op, &mut bus, MASTER), 0x1234);
        // Verify little-endian at physical address 0x20060
        assert_eq!(bus.mem[0x20060], 0x34);
        assert_eq!(bus.mem[0x20061], 0x12);
    }

    // -- Prefix consumption --
    //
    // Prefixes are part of the loaded instruction now: the loader fetches them
    // off the bus along with the opcode, staying in its opcode stage for as
    // long as prefixes keep arriving, and `consume_prefixes` reads them back
    // out of the buffer. So these stage bytes rather than writing memory.

    #[test]
    fn consume_no_prefixes() {
        let (mut cpu, _bus) = setup();
        stage(&mut cpu, &[0x90]); // NOP
        let opcode = cpu.consume_prefixes();
        assert_eq!(opcode, 0x90);
        assert_eq!(cpu.segment_override, None);
        assert_eq!(cpu.rep_prefix, None);
    }

    #[test]
    fn consume_segment_override_prefix() {
        let (mut cpu, _bus) = setup();
        stage(&mut cpu, &[0x26, 0x90]); // ES: then NOP
        let opcode = cpu.consume_prefixes();
        assert_eq!(opcode, 0x90);
        assert_eq!(cpu.segment_override, Some(SegReg::ES));
    }

    #[test]
    fn consume_rep_prefix() {
        let (mut cpu, _bus) = setup();
        stage(&mut cpu, &[0xF3, 0xA4]); // REP then MOVSB
        let opcode = cpu.consume_prefixes();
        assert_eq!(opcode, 0xA4);
        assert_eq!(cpu.rep_prefix, Some(RepPrefix::Rep));
    }

    #[test]
    fn consume_multiple_prefixes() {
        let (mut cpu, _bus) = setup();
        // LOCK, ES:, REPNZ, then CMPSB.
        stage(&mut cpu, &[0xF0, 0x26, 0xF2, 0xA6]);
        let opcode = cpu.consume_prefixes();
        assert_eq!(opcode, 0xA6);
        assert_eq!(cpu.segment_override, Some(SegReg::ES));
        assert_eq!(cpu.rep_prefix, Some(RepPrefix::Repnz));
    }

    #[test]
    fn consume_last_segment_override_wins() {
        let (mut cpu, _bus) = setup();
        // ES: then SS: then NOP. The last override is the one that takes.
        stage(&mut cpu, &[0x26, 0x36, 0x90]);
        let opcode = cpu.consume_prefixes();
        assert_eq!(opcode, 0x90);
        assert_eq!(cpu.segment_override, Some(SegReg::SS));
    }

    // -- Push/pop --

    #[test]
    fn push_pop_round_trip() {
        let (mut cpu, mut bus) = setup();
        cpu.ss = 0x0000;
        cpu.sp = 0x0100;

        cpu.push16(&mut bus, MASTER, 0x1234);
        assert_eq!(cpu.sp, 0x00FE);
        cpu.push16(&mut bus, MASTER, 0x5678);
        assert_eq!(cpu.sp, 0x00FC);

        let val1 = cpu.pop16(&mut bus, MASTER);
        assert_eq!(val1, 0x5678); // LIFO
        assert_eq!(cpu.sp, 0x00FE);

        let val2 = cpu.pop16(&mut bus, MASTER);
        assert_eq!(val2, 0x1234);
        assert_eq!(cpu.sp, 0x0100);
    }

    // -- Offset wrapping --

    #[test]
    fn ea_offset_wraps_16bit() {
        let (mut cpu, _bus) = setup();
        cpu.bx = 0xFFF0;
        cpu.si = 0x0020;
        // [BX+SI] = 0xFFF0 + 0x0020 = 0x10010 → wraps to 0x0010 (u16 wrapping)
        let modrm = ModRM {
            mod_bits: 0,
            reg: 0,
            rm: 0,
        };
        let op = cpu.resolve_modrm(modrm);
        assert_eq!(
            op,
            Operand::Memory {
                segment: 0x2000,
                offset: 0x0010,
            }
        );
    }

    // -- Fetch helpers --

    #[test]
    fn fetch_word_little_endian() {
        let (mut cpu, _bus) = setup();
        let ip = cpu.ip;
        stage(&mut cpu, &[0x34, 0x12]);
        let val = cpu.fetch_word();
        assert_eq!(val, 0x1234);
        assert_eq!(cpu.ip, ip + 2);
    }

    /// The executor cannot read past what the loader fetched. This is the guard
    /// on [`super::super::format`] disagreeing with an instruction's
    /// implementation about how long it is: without it, the executor would
    /// quietly read whatever the previous instruction left in the buffer.
    #[test]
    #[should_panic(expected = "consumed more bytes than the loader fetched")]
    fn fetching_past_the_loaded_instruction_panics() {
        let (mut cpu, _bus) = setup();
        stage(&mut cpu, &[0x34]);
        let _ = cpu.fetch_byte();
        let _ = cpu.fetch_byte();
    }

    #[test]
    fn read_write_word_memory() {
        let (cpu, mut bus) = setup();
        cpu.write_word(&mut bus, MASTER, 0x1000, 0x0050, 0xABCD);
        let val = cpu.read_word(&mut bus, MASTER, 0x1000, 0x0050);
        assert_eq!(val, 0xABCD);
        // Physical address = 0x10050
        assert_eq!(bus.mem[0x10050], 0xCD); // low byte
        assert_eq!(bus.mem[0x10051], 0xAB); // high byte
    }
}
