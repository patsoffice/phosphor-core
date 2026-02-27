//! Intel 8088 instruction execution.
//!
//! Main opcode dispatch and instruction implementations for data transfer
//! instructions. Future steps add ALU, control flow, shifts, string ops,
//! interrupts, and I/O.

use super::I8088;
use super::addressing::Operand;
use super::registers::SegReg;
use crate::core::{Bus, BusMaster};

impl I8088 {
    /// Dispatch and execute a single instruction given its opcode byte.
    /// The opcode has already been fetched; IP points to the first operand
    /// byte (ModR/M, immediate, or displacement).
    pub(crate) fn execute<B: Bus<Address = u32, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match opcode {
            // =============================================================
            // PUSH segment register: ES=0x06, CS=0x0E, SS=0x16, DS=0x1E
            // =============================================================
            0x06 | 0x0E | 0x16 | 0x1E => {
                let seg = I8088::decode_seg((opcode >> 3) & 3);
                let val = self.get_seg(seg);
                self.push16(bus, master, val);
            }

            // =============================================================
            // POP segment register: ES=0x07, CS=0x0F, SS=0x17, DS=0x1F
            // (POP CS is valid on 8088, removed in 286+)
            // =============================================================
            0x07 | 0x0F | 0x17 | 0x1F => {
                let seg = I8088::decode_seg((opcode >> 3) & 3);
                let val = self.pop16(bus, master);
                self.set_seg(seg, val);
                // TODO: MOV/POP to SS inhibits interrupts until after next
                // instruction (Step 1.8)
            }

            // =============================================================
            // PUSH reg16 (0x50-0x57)
            // =============================================================
            0x50..=0x57 => {
                let reg = opcode & 7;
                let val = self.get_reg16(reg);
                self.push16(bus, master, val);
            }

            // =============================================================
            // POP reg16 (0x58-0x5F)
            // =============================================================
            0x58..=0x5F => {
                let reg = opcode & 7;
                let val = self.pop16(bus, master);
                self.set_reg16(reg, val);
            }

            // =============================================================
            // XCHG r/m8, reg8 (0x86) | XCHG r/m16, reg16 (0x87)
            // =============================================================
            0x86 => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let a = self.get_reg8(modrm.reg);
                let b = self.read_operand8(operand, bus, master);
                self.set_reg8(modrm.reg, b);
                self.write_operand8(operand, bus, master, a);
            }
            0x87 => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let a = self.get_reg16(modrm.reg);
                let b = self.read_operand16(operand, bus, master);
                self.set_reg16(modrm.reg, b);
                self.write_operand16(operand, bus, master, a);
            }

            // =============================================================
            // MOV r/m, reg | MOV reg, r/m (0x88-0x8B)
            //   bit 0 (w): 0=byte, 1=word
            //   bit 1 (d): 0=reg→r/m, 1=r/m→reg
            // =============================================================
            0x88..=0x8B => {
                let w = opcode & 1 != 0;
                let d = opcode & 2 != 0;
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                if w {
                    if d {
                        let val = self.read_operand16(operand, bus, master);
                        self.set_reg16(modrm.reg, val);
                    } else {
                        let val = self.get_reg16(modrm.reg);
                        self.write_operand16(operand, bus, master, val);
                    }
                } else if d {
                    let val = self.read_operand8(operand, bus, master);
                    self.set_reg8(modrm.reg, val);
                } else {
                    let val = self.get_reg8(modrm.reg);
                    self.write_operand8(operand, bus, master, val);
                }
            }

            // =============================================================
            // MOV r/m16, segreg (0x8C)
            // =============================================================
            0x8C => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let seg = I8088::decode_seg(modrm.reg & 3);
                let val = self.get_seg(seg);
                self.write_operand16(operand, bus, master, val);
            }

            // =============================================================
            // LEA reg16, mem (0x8D)
            // =============================================================
            0x8D => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                if let Operand::Memory { offset, .. } = operand {
                    self.set_reg16(modrm.reg, offset);
                }
                // LEA with mod=11 is undefined; we treat it as a NOP.
            }

            // =============================================================
            // MOV segreg, r/m16 (0x8E)
            // =============================================================
            0x8E => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let val = self.read_operand16(operand, bus, master);
                let seg = I8088::decode_seg(modrm.reg & 3);
                self.set_seg(seg, val);
                // TODO: MOV to SS inhibits interrupts until after next
                // instruction (Step 1.8)
            }

            // =============================================================
            // POP r/m16 (0x8F /0)
            // =============================================================
            0x8F => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                if modrm.reg == 0 {
                    let val = self.pop16(bus, master);
                    self.write_operand16(operand, bus, master, val);
                }
            }

            // =============================================================
            // XCHG AX, reg16 (0x90-0x97)  — 0x90 = NOP (XCHG AX, AX)
            // =============================================================
            0x90..=0x97 => {
                let reg = opcode & 7;
                let temp = self.ax;
                self.ax = self.get_reg16(reg);
                self.set_reg16(reg, temp);
            }

            // =============================================================
            // SAHF (0x9E): AH → FLAGS low byte (SF, ZF, AF, PF, CF)
            // LAHF (0x9F): FLAGS low byte → AH
            // =============================================================
            0x9E => {
                // SAHF: load SF, ZF, AF, PF, CF from AH
                // Defined flag bits in low byte: bits 7,6,4,2,0 = mask 0xD5
                // Bit 1 is always 1
                let ah = self.ah() as u16;
                self.flags = (self.flags & 0xFF00) | (ah & 0x00D5) | 0x0002;
            }
            0x9F => {
                // LAHF: store flags low byte into AH
                self.set_ah(self.flags as u8);
            }

            // =============================================================
            // MOV AL/AX, [moffs] (0xA0-0xA1)
            // MOV [moffs], AL/AX (0xA2-0xA3)
            // =============================================================
            0xA0 => {
                let offset = self.fetch_word(bus, master);
                let seg = self.effective_segment(SegReg::DS);
                let val = self.read_byte(bus, master, seg, offset);
                self.set_al(val);
            }
            0xA1 => {
                let offset = self.fetch_word(bus, master);
                let seg = self.effective_segment(SegReg::DS);
                let val = self.read_word(bus, master, seg, offset);
                self.ax = val;
            }
            0xA2 => {
                let offset = self.fetch_word(bus, master);
                let seg = self.effective_segment(SegReg::DS);
                self.write_byte(bus, master, seg, offset, self.al());
            }
            0xA3 => {
                let offset = self.fetch_word(bus, master);
                let seg = self.effective_segment(SegReg::DS);
                self.write_word(bus, master, seg, offset, self.ax);
            }

            // =============================================================
            // MOV reg8, imm8 (0xB0-0xB7)
            // =============================================================
            0xB0..=0xB7 => {
                let reg = opcode & 7;
                let imm = self.fetch_byte(bus, master);
                self.set_reg8(reg, imm);
            }

            // =============================================================
            // MOV reg16, imm16 (0xB8-0xBF)
            // =============================================================
            0xB8..=0xBF => {
                let reg = opcode & 7;
                let imm = self.fetch_word(bus, master);
                self.set_reg16(reg, imm);
            }

            // =============================================================
            // LES reg16, mem32 (0xC4): load far pointer into ES:reg
            // LDS reg16, mem32 (0xC5): load far pointer into DS:reg
            // =============================================================
            0xC4 | 0xC5 => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                if let Operand::Memory { segment, offset } = operand {
                    let new_offset = self.read_word(bus, master, segment, offset);
                    let new_seg = self.read_word(bus, master, segment, offset.wrapping_add(2));
                    self.set_reg16(modrm.reg, new_offset);
                    if opcode == 0xC4 {
                        self.es = new_seg;
                    } else {
                        self.ds = new_seg;
                    }
                }
            }

            // =============================================================
            // MOV r/m8, imm8 (0xC6 /0)
            // =============================================================
            0xC6 => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let imm = self.fetch_byte(bus, master);
                if modrm.reg == 0 {
                    self.write_operand8(operand, bus, master, imm);
                }
            }

            // =============================================================
            // MOV r/m16, imm16 (0xC7 /0)
            // =============================================================
            0xC7 => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                let imm = self.fetch_word(bus, master);
                if modrm.reg == 0 {
                    self.write_operand16(operand, bus, master, imm);
                }
            }

            // =============================================================
            // XLAT (0xD7): AL = [DS:BX + unsigned AL]
            // =============================================================
            0xD7 => {
                let seg = self.effective_segment(SegReg::DS);
                let offset = self.bx.wrapping_add(self.al() as u16);
                let val = self.read_byte(bus, master, seg, offset);
                self.set_al(val);
            }

            // =============================================================
            // 0xFF group — only /6 (PUSH r/m16) in this step
            // =============================================================
            0xFF => {
                let modrm = self.fetch_modrm(bus, master);
                let operand = self.resolve_modrm(modrm, bus, master);
                if modrm.reg == 6 {
                    // PUSH r/m16
                    let val = self.read_operand16(operand, bus, master);
                    self.push16(bus, master, val);
                }
                // INC/DEC/CALL/JMP handled in later steps
            }

            // =============================================================
            // Unimplemented opcode — silently skip
            // =============================================================
            _ => {}
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

    /// 1 MB test bus (heap-allocated to avoid stack overflow).
    struct TestBus {
        mem: Box<[u8; 0x10_0000]>,
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

    const M: BusMaster = BusMaster::Cpu(0);

    /// Create a CPU ready for testing: CS=0, IP=0x100, DS=0x2000,
    /// SS=0x3000, ES=0x4000, SP=0x0200.
    fn setup() -> (I8088, TestBus) {
        let mut cpu = I8088::new();
        cpu.cs = 0x0000;
        cpu.ip = 0x0100;
        cpu.ds = 0x2000;
        cpu.ss = 0x3000;
        cpu.es = 0x4000;
        cpu.sp = 0x0200;
        (cpu, TestBus::new())
    }

    // =====================================================================
    // MOV reg8, imm8 (0xB0-0xB7)
    // =====================================================================

    #[test]
    fn mov_al_imm8() {
        let (mut cpu, mut bus) = setup();
        bus.mem[0x100] = 0x42; // imm8
        cpu.execute(0xB0, &mut bus, M);
        assert_eq!(cpu.al(), 0x42);
        assert_eq!(cpu.ip, 0x101);
    }

    #[test]
    fn mov_all_reg8_imm8() {
        let (mut cpu, mut bus) = setup();
        // Test all 8 registers: AL,CL,DL,BL,AH,CH,DH,BH
        for reg in 0..8u8 {
            cpu.ip = 0x100;
            bus.mem[0x100] = 0x10 + reg; // distinct value per register
            cpu.execute(0xB0 + reg, &mut bus, M);
            assert_eq!(cpu.get_reg8(reg), 0x10 + reg);
        }
    }

    // =====================================================================
    // MOV reg16, imm16 (0xB8-0xBF)
    // =====================================================================

    #[test]
    fn mov_ax_imm16() {
        let (mut cpu, mut bus) = setup();
        bus.mem[0x100] = 0x34; // low byte
        bus.mem[0x101] = 0x12; // high byte
        cpu.execute(0xB8, &mut bus, M); // MOV AX, 0x1234
        assert_eq!(cpu.ax, 0x1234);
        assert_eq!(cpu.ip, 0x102);
    }

    #[test]
    fn mov_all_reg16_imm16() {
        let (mut cpu, mut bus) = setup();
        for reg in 0..8u8 {
            cpu.ip = 0x100;
            let val = 0x1000 + reg as u16;
            bus.mem[0x100] = val as u8;
            bus.mem[0x101] = (val >> 8) as u8;
            cpu.execute(0xB8 + reg, &mut bus, M);
            assert_eq!(cpu.get_reg16(reg), val);
        }
    }

    // =====================================================================
    // MOV r/m, reg | MOV reg, r/m (0x88-0x8B)
    // =====================================================================

    #[test]
    fn mov_rm8_reg8() {
        let (mut cpu, mut bus) = setup();
        cpu.set_al(0xAB);
        cpu.bx = 0x0050;
        // ModR/M: mod=00 reg=000(AL) rm=111([BX]) = 0x07
        bus.mem[0x100] = 0x07;
        cpu.execute(0x88, &mut bus, M); // MOV [BX], AL
        // DS:BX = 0x20000 + 0x50 = 0x20050
        assert_eq!(bus.mem[0x20050], 0xAB);
    }

    #[test]
    fn mov_reg8_rm8() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0050;
        bus.mem[0x20050] = 0xCD; // value at DS:BX
        // ModR/M: mod=00 reg=001(CL) rm=111([BX]) = 0x0F
        bus.mem[0x100] = 0x0F;
        cpu.execute(0x8A, &mut bus, M); // MOV CL, [BX]
        assert_eq!(cpu.cl(), 0xCD);
    }

    #[test]
    fn mov_rm16_reg16() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x1234;
        cpu.si = 0x0080;
        // ModR/M: mod=00 reg=000(AX) rm=100([SI]) = 0x04
        bus.mem[0x100] = 0x04;
        cpu.execute(0x89, &mut bus, M); // MOV [SI], AX
        // DS:SI = 0x20080
        assert_eq!(bus.mem[0x20080], 0x34);
        assert_eq!(bus.mem[0x20081], 0x12);
    }

    #[test]
    fn mov_reg16_rm16() {
        let (mut cpu, mut bus) = setup();
        cpu.di = 0x0060;
        bus.mem[0x20060] = 0x78;
        bus.mem[0x20061] = 0x56;
        // ModR/M: mod=00 reg=011(BX) rm=101([DI]) = 0x1D
        bus.mem[0x100] = 0x1D;
        cpu.execute(0x8B, &mut bus, M); // MOV BX, [DI]
        assert_eq!(cpu.bx, 0x5678);
    }

    #[test]
    fn mov_reg_reg() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0xAABB;
        // MOV CX, AX: 0x89 ModR/M=0xC1 (mod=11 reg=000(AX) rm=001(CX))
        bus.mem[0x100] = 0xC1;
        cpu.execute(0x89, &mut bus, M);
        assert_eq!(cpu.cx, 0xAABB);
    }

    // =====================================================================
    // MOV r/m16, segreg (0x8C) | MOV segreg, r/m16 (0x8E)
    // =====================================================================

    #[test]
    fn mov_rm16_segreg() {
        let (mut cpu, mut bus) = setup();
        cpu.ds = 0x1234;
        // ModR/M: mod=11 reg=011(DS) rm=000(AX) = 0xD8
        bus.mem[0x100] = 0xD8;
        cpu.execute(0x8C, &mut bus, M); // MOV AX, DS
        assert_eq!(cpu.ax, 0x1234);
    }

    #[test]
    fn mov_segreg_rm16() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x5000;
        // ModR/M: mod=11 reg=000(ES) rm=000(AX) = 0xC0
        bus.mem[0x100] = 0xC0;
        cpu.execute(0x8E, &mut bus, M); // MOV ES, AX
        assert_eq!(cpu.es, 0x5000);
    }

    // =====================================================================
    // MOV AL/AX, [moffs] (0xA0-0xA1)
    // MOV [moffs], AL/AX (0xA2-0xA3)
    // =====================================================================

    #[test]
    fn mov_al_moffs() {
        let (mut cpu, mut bus) = setup();
        // moffs = 0x0050
        bus.mem[0x100] = 0x50;
        bus.mem[0x101] = 0x00;
        // Value at DS:0x0050 = 0x20050
        bus.mem[0x20050] = 0xEF;
        cpu.execute(0xA0, &mut bus, M); // MOV AL, [0x0050]
        assert_eq!(cpu.al(), 0xEF);
    }

    #[test]
    fn mov_ax_moffs() {
        let (mut cpu, mut bus) = setup();
        bus.mem[0x100] = 0x60;
        bus.mem[0x101] = 0x00;
        bus.mem[0x20060] = 0x34;
        bus.mem[0x20061] = 0x12;
        cpu.execute(0xA1, &mut bus, M); // MOV AX, [0x0060]
        assert_eq!(cpu.ax, 0x1234);
    }

    #[test]
    fn mov_moffs_al() {
        let (mut cpu, mut bus) = setup();
        cpu.set_al(0x42);
        bus.mem[0x100] = 0x70;
        bus.mem[0x101] = 0x00;
        cpu.execute(0xA2, &mut bus, M); // MOV [0x0070], AL
        assert_eq!(bus.mem[0x20070], 0x42);
    }

    #[test]
    fn mov_moffs_ax() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0xABCD;
        bus.mem[0x100] = 0x80;
        bus.mem[0x101] = 0x00;
        cpu.execute(0xA3, &mut bus, M); // MOV [0x0080], AX
        assert_eq!(bus.mem[0x20080], 0xCD);
        assert_eq!(bus.mem[0x20081], 0xAB);
    }

    // =====================================================================
    // MOV r/m, imm (0xC6, 0xC7)
    // =====================================================================

    #[test]
    fn mov_rm8_imm8() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0040;
        // ModR/M: mod=00 reg=000 rm=111([BX]) = 0x07
        bus.mem[0x100] = 0x07;
        bus.mem[0x101] = 0xFF; // imm8
        cpu.execute(0xC6, &mut bus, M); // MOV BYTE [BX], 0xFF
        assert_eq!(bus.mem[0x20040], 0xFF);
    }

    #[test]
    fn mov_rm16_imm16() {
        let (mut cpu, mut bus) = setup();
        cpu.si = 0x0040;
        // ModR/M: mod=00 reg=000 rm=100([SI]) = 0x04
        bus.mem[0x100] = 0x04;
        bus.mem[0x101] = 0x34; // imm16 low
        bus.mem[0x102] = 0x12; // imm16 high
        cpu.execute(0xC7, &mut bus, M); // MOV WORD [SI], 0x1234
        assert_eq!(bus.mem[0x20040], 0x34);
        assert_eq!(bus.mem[0x20041], 0x12);
    }

    // =====================================================================
    // PUSH/POP reg16 (0x50-0x5F)
    // =====================================================================

    #[test]
    fn push_pop_reg16() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x1234;
        cpu.bx = 0x5678;

        cpu.execute(0x50, &mut bus, M); // PUSH AX
        assert_eq!(cpu.sp, 0x01FE);
        cpu.execute(0x53, &mut bus, M); // PUSH BX
        assert_eq!(cpu.sp, 0x01FC);

        // Pop in reverse order (LIFO)
        cpu.execute(0x59, &mut bus, M); // POP CX
        assert_eq!(cpu.cx, 0x5678);
        assert_eq!(cpu.sp, 0x01FE);
        cpu.execute(0x5A, &mut bus, M); // POP DX
        assert_eq!(cpu.dx, 0x1234);
        assert_eq!(cpu.sp, 0x0200);
    }

    #[test]
    fn push_sp_pushes_old_value() {
        let (mut cpu, mut bus) = setup();
        // On 8088, PUSH SP pushes the value of SP before decrement
        let old_sp = cpu.sp;
        cpu.execute(0x54, &mut bus, M); // PUSH SP
        let pushed_val = cpu.read_word(&mut bus, M, cpu.ss, cpu.sp);
        assert_eq!(pushed_val, old_sp);
    }

    // =====================================================================
    // PUSH/POP segment registers
    // =====================================================================

    #[test]
    fn push_pop_es() {
        let (mut cpu, mut bus) = setup();
        cpu.es = 0xABCD;
        cpu.execute(0x06, &mut bus, M); // PUSH ES
        cpu.es = 0x0000; // Clear it
        cpu.execute(0x07, &mut bus, M); // POP ES
        assert_eq!(cpu.es, 0xABCD);
    }

    #[test]
    fn push_pop_ds() {
        let (mut cpu, mut bus) = setup();
        cpu.ds = 0x1234;
        cpu.execute(0x1E, &mut bus, M); // PUSH DS
        cpu.ds = 0x0000;
        cpu.execute(0x1F, &mut bus, M); // POP DS
        assert_eq!(cpu.ds, 0x1234);
    }

    #[test]
    fn push_cs() {
        let (mut cpu, mut bus) = setup();
        cpu.cs = 0xF000;
        cpu.execute(0x0E, &mut bus, M); // PUSH CS
        let val = cpu.read_word(&mut bus, M, cpu.ss, cpu.sp);
        assert_eq!(val, 0xF000);
    }

    // =====================================================================
    // PUSH/POP r/m16 (0xFF /6, 0x8F /0)
    // =====================================================================

    #[test]
    fn push_rm16_mem() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0020;
        // Store 0xBEEF at DS:BX
        bus.mem[0x20020] = 0xEF;
        bus.mem[0x20021] = 0xBE;
        // ModR/M: mod=00 reg=110(/6) rm=111([BX]) = 0x37
        bus.mem[0x100] = 0x37;
        cpu.execute(0xFF, &mut bus, M); // PUSH WORD [BX]
        let val = cpu.read_word(&mut bus, M, cpu.ss, cpu.sp);
        assert_eq!(val, 0xBEEF);
    }

    #[test]
    fn pop_rm16_mem() {
        let (mut cpu, mut bus) = setup();
        // Push a value first
        cpu.push16(&mut bus, M, 0xDEAD);
        cpu.bx = 0x0030;
        // ModR/M: mod=00 reg=000(/0) rm=111([BX]) = 0x07
        bus.mem[0x100] = 0x07;
        cpu.execute(0x8F, &mut bus, M); // POP WORD [BX]
        assert_eq!(bus.mem[0x20030], 0xAD);
        assert_eq!(bus.mem[0x20031], 0xDE);
    }

    // =====================================================================
    // XCHG AX, reg16 (0x90-0x97)
    // =====================================================================

    #[test]
    fn nop() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x1234;
        let old_ip = cpu.ip;
        cpu.execute(0x90, &mut bus, M); // NOP = XCHG AX, AX
        assert_eq!(cpu.ax, 0x1234);
        assert_eq!(cpu.ip, old_ip); // No operand bytes consumed
    }

    #[test]
    fn xchg_ax_cx() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x1111;
        cpu.cx = 0x2222;
        cpu.execute(0x91, &mut bus, M); // XCHG AX, CX
        assert_eq!(cpu.ax, 0x2222);
        assert_eq!(cpu.cx, 0x1111);
    }

    // =====================================================================
    // XCHG r/m, reg (0x86-0x87)
    // =====================================================================

    #[test]
    fn xchg_rm8_reg8() {
        let (mut cpu, mut bus) = setup();
        cpu.set_al(0xAA);
        cpu.set_bl(0xBB);
        // ModR/M: mod=11 reg=000(AL) rm=011(BL) = 0xC3
        bus.mem[0x100] = 0xC3;
        cpu.execute(0x86, &mut bus, M); // XCHG AL, BL
        assert_eq!(cpu.al(), 0xBB);
        assert_eq!(cpu.bl(), 0xAA);
    }

    #[test]
    fn xchg_rm16_reg16_mem() {
        let (mut cpu, mut bus) = setup();
        cpu.ax = 0x1234;
        cpu.bx = 0x0050;
        bus.mem[0x20050] = 0x78;
        bus.mem[0x20051] = 0x56;
        // ModR/M: mod=00 reg=000(AX) rm=111([BX]) = 0x07
        bus.mem[0x100] = 0x07;
        cpu.execute(0x87, &mut bus, M); // XCHG AX, [BX]
        assert_eq!(cpu.ax, 0x5678);
        assert_eq!(bus.mem[0x20050], 0x34);
        assert_eq!(bus.mem[0x20051], 0x12);
    }

    // =====================================================================
    // LEA (0x8D)
    // =====================================================================

    #[test]
    fn lea_reg16_bx_si() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x1000;
        cpu.si = 0x0234;
        // ModR/M: mod=00 reg=001(CX) rm=000([BX+SI]) = 0x08
        bus.mem[0x100] = 0x08;
        cpu.execute(0x8D, &mut bus, M); // LEA CX, [BX+SI]
        assert_eq!(cpu.cx, 0x1234);
    }

    #[test]
    fn lea_reg16_bp_disp8() {
        let (mut cpu, mut bus) = setup();
        cpu.bp = 0x0100;
        // ModR/M: mod=01 reg=010(DX) rm=110([BP+disp8]) = 0x56
        bus.mem[0x100] = 0x56;
        bus.mem[0x101] = 0x10; // disp8 = +16
        cpu.execute(0x8D, &mut bus, M); // LEA DX, [BP+16]
        assert_eq!(cpu.dx, 0x0110);
    }

    #[test]
    fn lea_direct() {
        let (mut cpu, mut bus) = setup();
        // ModR/M: mod=00 reg=000(AX) rm=110(direct) = 0x06
        bus.mem[0x100] = 0x06;
        bus.mem[0x101] = 0x00; // disp16 low
        bus.mem[0x102] = 0x80; // disp16 high = 0x8000
        cpu.execute(0x8D, &mut bus, M); // LEA AX, [0x8000]
        assert_eq!(cpu.ax, 0x8000);
    }

    // =====================================================================
    // LES (0xC4) / LDS (0xC5)
    // =====================================================================

    #[test]
    fn les_reg16() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0040;
        // Far pointer at DS:BX = 0x20040: offset=0x1234, segment=0x5000
        bus.mem[0x20040] = 0x34;
        bus.mem[0x20041] = 0x12;
        bus.mem[0x20042] = 0x00;
        bus.mem[0x20043] = 0x50;
        // ModR/M: mod=00 reg=001(CX) rm=111([BX]) = 0x0F
        bus.mem[0x100] = 0x0F;
        cpu.execute(0xC4, &mut bus, M); // LES CX, [BX]
        assert_eq!(cpu.cx, 0x1234);
        assert_eq!(cpu.es, 0x5000);
    }

    #[test]
    fn lds_reg16() {
        let (mut cpu, mut bus) = setup();
        cpu.si = 0x0060;
        // Far pointer at DS:SI = 0x20060: offset=0xABCD, segment=0x6000
        bus.mem[0x20060] = 0xCD;
        bus.mem[0x20061] = 0xAB;
        bus.mem[0x20062] = 0x00;
        bus.mem[0x20063] = 0x60;
        // ModR/M: mod=00 reg=010(DX) rm=100([SI]) = 0x14
        bus.mem[0x100] = 0x14;
        cpu.execute(0xC5, &mut bus, M); // LDS DX, [SI]
        assert_eq!(cpu.dx, 0xABCD);
        assert_eq!(cpu.ds, 0x6000);
    }

    // =====================================================================
    // SAHF (0x9E) / LAHF (0x9F)
    // =====================================================================

    #[test]
    fn sahf_loads_flags_from_ah() {
        let (mut cpu, mut bus) = setup();
        use super::super::flags::{self, Flag};
        // Set AH with CF=1, PF=1, ZF=1, SF=1 (bits 0,2,6,7 = 0xC5)
        cpu.set_ah(0xC5);
        cpu.execute(0x9E, &mut bus, M); // SAHF
        assert!(flags::get(cpu.flags, Flag::CF));
        assert!(flags::get(cpu.flags, Flag::PF));
        assert!(flags::get(cpu.flags, Flag::ZF));
        assert!(flags::get(cpu.flags, Flag::SF));
        assert!(!flags::get(cpu.flags, Flag::AF)); // bit 4 not set in 0xC5
        // Bit 1 always 1
        assert_ne!(cpu.flags & 0x0002, 0);
    }

    #[test]
    fn lahf_stores_flags_to_ah() {
        let (mut cpu, mut bus) = setup();
        use super::super::flags::{self, Flag};
        flags::set(&mut cpu.flags, Flag::CF, true);
        flags::set(&mut cpu.flags, Flag::ZF, true);
        cpu.execute(0x9F, &mut bus, M); // LAHF
        let ah = cpu.ah();
        assert_ne!(ah & 0x01, 0); // CF
        assert_ne!(ah & 0x40, 0); // ZF
    }

    // =====================================================================
    // XLAT (0xD7)
    // =====================================================================

    #[test]
    fn xlat_table_lookup() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0100;
        cpu.set_al(0x05);
        // Translation table at DS:BX = 0x20100
        bus.mem[0x20105] = 0x42; // table[5] = 0x42
        cpu.execute(0xD7, &mut bus, M); // XLAT
        assert_eq!(cpu.al(), 0x42);
    }

    // =====================================================================
    // Segment override integration
    // =====================================================================

    #[test]
    fn mov_with_segment_override() {
        let (mut cpu, mut bus) = setup();
        cpu.segment_override = Some(SegReg::ES);
        cpu.es = 0x5000;
        cpu.bx = 0x0010;
        bus.mem[0x50010] = 0xAB; // Value at ES:BX
        // ModR/M: mod=00 reg=000(AL) rm=111([BX]) = 0x07
        bus.mem[0x100] = 0x07;
        cpu.execute(0x8A, &mut bus, M); // MOV AL, ES:[BX]
        assert_eq!(cpu.al(), 0xAB);
    }

    // =====================================================================
    // MOV with displacement addressing modes
    // =====================================================================

    #[test]
    fn mov_reg_mem_disp8() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0100;
        // Value at DS:(BX+0x10) = DS:0x0110 = 0x20110
        bus.mem[0x20110] = 0x99;
        // ModR/M: mod=01 reg=000(AL) rm=111([BX+disp8]) = 0x47
        bus.mem[0x100] = 0x47;
        bus.mem[0x101] = 0x10; // disp8
        cpu.execute(0x8A, &mut bus, M); // MOV AL, [BX+0x10]
        assert_eq!(cpu.al(), 0x99);
    }

    #[test]
    fn mov_reg_mem_disp16() {
        let (mut cpu, mut bus) = setup();
        cpu.bx = 0x0100;
        // Value at DS:(BX+0x1000) = DS:0x1100 = 0x21100
        bus.mem[0x21100] = 0x77;
        // ModR/M: mod=10 reg=001(CL) rm=111([BX+disp16]) = 0x8F
        bus.mem[0x100] = 0x8F;
        bus.mem[0x101] = 0x00; // disp16 low
        bus.mem[0x102] = 0x10; // disp16 high = 0x1000
        cpu.execute(0x8A, &mut bus, M); // MOV CL, [BX+0x1000]
        assert_eq!(cpu.cl(), 0x77);
    }

    #[test]
    fn mov_mem_direct_imm16() {
        let (mut cpu, mut bus) = setup();
        // ModR/M: mod=00 reg=000 rm=110(direct) = 0x06
        bus.mem[0x100] = 0x06;
        bus.mem[0x101] = 0x50; // address low
        bus.mem[0x102] = 0x00; // address high = 0x0050
        bus.mem[0x103] = 0xEF; // imm16 low
        bus.mem[0x104] = 0xBE; // imm16 high = 0xBEEF
        cpu.execute(0xC7, &mut bus, M); // MOV WORD [0x0050], 0xBEEF
        assert_eq!(bus.mem[0x20050], 0xEF);
        assert_eq!(bus.mem[0x20051], 0xBE);
    }
}
