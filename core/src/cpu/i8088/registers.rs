//! Intel 8088 register definitions and accessors.
//!
//! The 8088 has four 16-bit general-purpose registers (AX, BX, CX, DX) that
//! can be accessed as 8-bit halves (AH/AL, BH/BL, CH/CL, DH/DL), plus index
//! registers (SI, DI), pointer registers (BP, SP), four segment registers
//! (CS, DS, ES, SS), and the instruction pointer (IP).

use super::I8088;

/// Segment register selector.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SegReg {
    ES = 0,
    CS = 1,
    SS = 2,
    DS = 3,
}

/// 8-bit register encoding as used in ModR/M `reg` field (for byte operations).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Reg8 {
    AL = 0,
    CL = 1,
    DL = 2,
    BL = 3,
    AH = 4,
    CH = 5,
    DH = 6,
    BH = 7,
}

/// 16-bit register encoding as used in ModR/M `reg` field (for word operations).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Reg16 {
    AX = 0,
    CX = 1,
    DX = 2,
    BX = 3,
    SP = 4,
    BP = 5,
    SI = 6,
    DI = 7,
}

impl I8088 {
    // -----------------------------------------------------------------------
    // 8-bit register halves
    // -----------------------------------------------------------------------

    /// Read an 8-bit register by encoding (0=AL..7=BH).
    #[inline]
    pub fn get_reg8(&self, reg: u8) -> u8 {
        match reg & 7 {
            0 => self.ax as u8,        // AL
            1 => self.cx as u8,        // CL
            2 => self.dx as u8,        // DL
            3 => self.bx as u8,        // BL
            4 => (self.ax >> 8) as u8, // AH
            5 => (self.cx >> 8) as u8, // CH
            6 => (self.dx >> 8) as u8, // DH
            7 => (self.bx >> 8) as u8, // BH
            _ => unreachable!(),
        }
    }

    /// Write an 8-bit register by encoding (0=AL..7=BH).
    #[inline]
    pub fn set_reg8(&mut self, reg: u8, val: u8) {
        match reg & 7 {
            0 => self.ax = (self.ax & 0xFF00) | val as u16, // AL
            1 => self.cx = (self.cx & 0xFF00) | val as u16, // CL
            2 => self.dx = (self.dx & 0xFF00) | val as u16, // DL
            3 => self.bx = (self.bx & 0xFF00) | val as u16, // BL
            4 => self.ax = (self.ax & 0x00FF) | (val as u16) << 8, // AH
            5 => self.cx = (self.cx & 0x00FF) | (val as u16) << 8, // CH
            6 => self.dx = (self.dx & 0x00FF) | (val as u16) << 8, // DH
            7 => self.bx = (self.bx & 0x00FF) | (val as u16) << 8, // BH
            _ => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // 16-bit registers
    // -----------------------------------------------------------------------

    /// Read a 16-bit register by encoding (0=AX..7=DI).
    #[inline]
    pub fn get_reg16(&self, reg: u8) -> u16 {
        match reg & 7 {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    /// Write a 16-bit register by encoding (0=AX..7=DI).
    #[inline]
    pub fn set_reg16(&mut self, reg: u8, val: u16) {
        match reg & 7 {
            0 => self.ax = val,
            1 => self.cx = val,
            2 => self.dx = val,
            3 => self.bx = val,
            4 => self.sp = val,
            5 => self.bp = val,
            6 => self.si = val,
            7 => self.di = val,
            _ => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Segment registers
    // -----------------------------------------------------------------------

    /// Read a segment register by encoding (0=ES, 1=CS, 2=SS, 3=DS).
    #[inline]
    pub fn get_seg(&self, seg: SegReg) -> u16 {
        match seg {
            SegReg::ES => self.es,
            SegReg::CS => self.cs,
            SegReg::SS => self.ss,
            SegReg::DS => self.ds,
        }
    }

    /// Write a segment register by encoding (0=ES, 1=CS, 2=SS, 3=DS).
    #[inline]
    pub fn set_seg(&mut self, seg: SegReg, val: u16) {
        match seg {
            SegReg::ES => self.es = val,
            SegReg::CS => self.cs = val,
            SegReg::SS => self.ss = val,
            SegReg::DS => self.ds = val,
        }
    }

    /// Decode a 2-bit segment register field from an instruction byte.
    #[inline]
    pub fn decode_seg(bits: u8) -> SegReg {
        match bits & 3 {
            0 => SegReg::ES,
            1 => SegReg::CS,
            2 => SegReg::SS,
            3 => SegReg::DS,
            _ => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Convenience accessors for 8-bit halves by name
    // -----------------------------------------------------------------------

    #[inline]
    pub fn al(&self) -> u8 {
        self.ax as u8
    }
    #[inline]
    pub fn ah(&self) -> u8 {
        (self.ax >> 8) as u8
    }
    #[inline]
    pub fn set_al(&mut self, v: u8) {
        self.ax = (self.ax & 0xFF00) | v as u16;
    }
    #[inline]
    pub fn set_ah(&mut self, v: u8) {
        self.ax = (self.ax & 0x00FF) | (v as u16) << 8;
    }

    #[inline]
    pub fn bl(&self) -> u8 {
        self.bx as u8
    }
    #[inline]
    pub fn bh(&self) -> u8 {
        (self.bx >> 8) as u8
    }
    #[inline]
    pub fn set_bl(&mut self, v: u8) {
        self.bx = (self.bx & 0xFF00) | v as u16;
    }
    #[inline]
    pub fn set_bh(&mut self, v: u8) {
        self.bx = (self.bx & 0x00FF) | (v as u16) << 8;
    }

    #[inline]
    pub fn cl(&self) -> u8 {
        self.cx as u8
    }
    #[inline]
    pub fn ch(&self) -> u8 {
        (self.cx >> 8) as u8
    }
    #[inline]
    pub fn set_cl(&mut self, v: u8) {
        self.cx = (self.cx & 0xFF00) | v as u16;
    }
    #[inline]
    pub fn set_ch(&mut self, v: u8) {
        self.cx = (self.cx & 0x00FF) | (v as u16) << 8;
    }

    #[inline]
    pub fn dl(&self) -> u8 {
        self.dx as u8
    }
    #[inline]
    pub fn dh(&self) -> u8 {
        (self.dx >> 8) as u8
    }
    #[inline]
    pub fn set_dl(&mut self, v: u8) {
        self.dx = (self.dx & 0xFF00) | v as u16;
    }
    #[inline]
    pub fn set_dh(&mut self, v: u8) {
        self.dx = (self.dx & 0x00FF) | (v as u16) << 8;
    }

    // -----------------------------------------------------------------------
    // Physical address calculation
    // -----------------------------------------------------------------------

    /// Compute a 20-bit physical address from segment:offset.
    #[inline]
    pub fn physical_addr(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4).wrapping_add(offset as u32) & 0xF_FFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_cpu() -> I8088 {
        I8088::new()
    }

    // -- 8-bit register access --

    #[test]
    fn reg8_al_ah() {
        let mut cpu = new_cpu();
        cpu.ax = 0x1234;
        assert_eq!(cpu.al(), 0x34);
        assert_eq!(cpu.ah(), 0x12);
        assert_eq!(cpu.get_reg8(0), 0x34); // AL
        assert_eq!(cpu.get_reg8(4), 0x12); // AH

        cpu.set_al(0xAB);
        assert_eq!(cpu.ax, 0x12AB);
        cpu.set_ah(0xCD);
        assert_eq!(cpu.ax, 0xCDAB);
    }

    #[test]
    fn reg8_all_halves() {
        let mut cpu = new_cpu();
        cpu.ax = 0xAABB;
        cpu.cx = 0xCCDD;
        cpu.dx = 0xEEFF;
        cpu.bx = 0x1122;

        // Low halves: AL=0, CL=1, DL=2, BL=3
        assert_eq!(cpu.get_reg8(0), 0xBB);
        assert_eq!(cpu.get_reg8(1), 0xDD);
        assert_eq!(cpu.get_reg8(2), 0xFF);
        assert_eq!(cpu.get_reg8(3), 0x22);
        // High halves: AH=4, CH=5, DH=6, BH=7
        assert_eq!(cpu.get_reg8(4), 0xAA);
        assert_eq!(cpu.get_reg8(5), 0xCC);
        assert_eq!(cpu.get_reg8(6), 0xEE);
        assert_eq!(cpu.get_reg8(7), 0x11);
    }

    #[test]
    fn set_reg8_preserves_other_half() {
        let mut cpu = new_cpu();
        cpu.ax = 0x0000;
        cpu.set_reg8(0, 0xFF); // AL
        assert_eq!(cpu.ax, 0x00FF);
        cpu.set_reg8(4, 0xAA); // AH
        assert_eq!(cpu.ax, 0xAAFF);
    }

    // -- 16-bit register access --

    #[test]
    fn reg16_round_trip() {
        let mut cpu = new_cpu();
        for reg in 0..8u8 {
            cpu.set_reg16(reg, 0x1234 + reg as u16);
            assert_eq!(cpu.get_reg16(reg), 0x1234 + reg as u16);
        }
    }

    // -- Segment registers --

    #[test]
    fn seg_round_trip() {
        let mut cpu = new_cpu();
        for &seg in &[SegReg::ES, SegReg::CS, SegReg::SS, SegReg::DS] {
            cpu.set_seg(seg, 0xABCD);
            assert_eq!(cpu.get_seg(seg), 0xABCD);
        }
    }

    #[test]
    fn decode_seg_all() {
        assert_eq!(I8088::decode_seg(0), SegReg::ES);
        assert_eq!(I8088::decode_seg(1), SegReg::CS);
        assert_eq!(I8088::decode_seg(2), SegReg::SS);
        assert_eq!(I8088::decode_seg(3), SegReg::DS);
    }

    // -- Physical address --

    #[test]
    fn physical_addr_basic() {
        // 0x1000:0x0234 = 0x10234
        assert_eq!(I8088::physical_addr(0x1000, 0x0234), 0x10234);
    }

    #[test]
    fn physical_addr_wrap_20bit() {
        // 0xFFFF:0x0010 = 0xFFFF0 + 0x10 = 0x100000 → wraps to 0x00000
        assert_eq!(I8088::physical_addr(0xFFFF, 0x0010), 0x00000);
    }

    #[test]
    fn physical_addr_reset_vector() {
        // 8088 reset: CS=0xFFFF, IP=0x0000 → physical 0xFFFF0
        assert_eq!(I8088::physical_addr(0xFFFF, 0x0000), 0xFFFF0);
    }

    #[test]
    fn physical_addr_zero() {
        assert_eq!(I8088::physical_addr(0x0000, 0x0000), 0x00000);
    }
}
