//! Intel 8088 ALU (Arithmetic Logic Unit) operations.
//!
//! Core arithmetic and logic functions with full flag updates. Each function
//! takes a mutable reference to the FLAGS register and returns the result.

use super::flags::{self, Flag};

// ---------------------------------------------------------------------------
// Addition
// ---------------------------------------------------------------------------

/// 8-bit add with carry-in. Updates CF, OF, AF, SF, ZF, PF.
#[inline]
pub fn add8(flags: &mut u16, a: u8, b: u8, carry_in: bool) -> u8 {
    let ci = carry_in as u16;
    let result16 = a as u16 + b as u16 + ci;
    let result = result16 as u8;

    flags::set(flags, Flag::CF, result16 > 0xFF);
    flags::set(flags, Flag::OF, ((a ^ result) & (b ^ result) & 0x80) != 0);
    flags::set(flags, Flag::AF, ((a ^ b ^ result) & 0x10) != 0);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit add with carry-in. Updates CF, OF, AF, SF, ZF, PF.
#[inline]
pub fn add16(flags: &mut u16, a: u16, b: u16, carry_in: bool) -> u16 {
    let ci = carry_in as u32;
    let result32 = a as u32 + b as u32 + ci;
    let result = result32 as u16;

    flags::set(flags, Flag::CF, result32 > 0xFFFF);
    flags::set(flags, Flag::OF, ((a ^ result) & (b ^ result) & 0x8000) != 0);
    flags::set(flags, Flag::AF, ((a ^ b ^ result) & 0x10) != 0);
    flags::update_szp16(flags, result);
    result
}

// ---------------------------------------------------------------------------
// Subtraction
// ---------------------------------------------------------------------------

/// 8-bit subtract with borrow-in. Updates CF, OF, AF, SF, ZF, PF.
#[inline]
pub fn sub8(flags: &mut u16, a: u8, b: u8, borrow_in: bool) -> u8 {
    let bi = borrow_in as u16;
    let result16 = (a as u16).wrapping_sub(b as u16).wrapping_sub(bi);
    let result = result16 as u8;

    flags::set(flags, Flag::CF, result16 > 0xFF); // borrow occurred
    flags::set(flags, Flag::OF, ((a ^ b) & (a ^ result) & 0x80) != 0);
    flags::set(flags, Flag::AF, ((a ^ b ^ result) & 0x10) != 0);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit subtract with borrow-in. Updates CF, OF, AF, SF, ZF, PF.
#[inline]
pub fn sub16(flags: &mut u16, a: u16, b: u16, borrow_in: bool) -> u16 {
    let bi = borrow_in as u32;
    let result32 = (a as u32).wrapping_sub(b as u32).wrapping_sub(bi);
    let result = result32 as u16;

    flags::set(flags, Flag::CF, result32 > 0xFFFF);
    flags::set(flags, Flag::OF, ((a ^ b) & (a ^ result) & 0x8000) != 0);
    flags::set(flags, Flag::AF, ((a ^ b ^ result) & 0x10) != 0);
    flags::update_szp16(flags, result);
    result
}

// ---------------------------------------------------------------------------
// Increment / Decrement (CF is NOT affected)
// ---------------------------------------------------------------------------

/// 8-bit increment. Updates OF, AF, SF, ZF, PF. CF unchanged.
#[inline]
pub fn inc8(flags: &mut u16, a: u8) -> u8 {
    let result = a.wrapping_add(1);
    flags::set(flags, Flag::OF, a == 0x7F);
    flags::set(flags, Flag::AF, (a & 0x0F) == 0x0F);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit increment. Updates OF, AF, SF, ZF, PF. CF unchanged.
#[inline]
pub fn inc16(flags: &mut u16, a: u16) -> u16 {
    let result = a.wrapping_add(1);
    flags::set(flags, Flag::OF, a == 0x7FFF);
    flags::set(flags, Flag::AF, (a & 0x0F) == 0x0F);
    flags::update_szp16(flags, result);
    result
}

/// 8-bit decrement. Updates OF, AF, SF, ZF, PF. CF unchanged.
#[inline]
pub fn dec8(flags: &mut u16, a: u8) -> u8 {
    let result = a.wrapping_sub(1);
    flags::set(flags, Flag::OF, a == 0x80);
    flags::set(flags, Flag::AF, (a & 0x0F) == 0x00);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit decrement. Updates OF, AF, SF, ZF, PF. CF unchanged.
#[inline]
pub fn dec16(flags: &mut u16, a: u16) -> u16 {
    let result = a.wrapping_sub(1);
    flags::set(flags, Flag::OF, a == 0x8000);
    flags::set(flags, Flag::AF, (a & 0x0F) == 0x00);
    flags::update_szp16(flags, result);
    result
}

// ---------------------------------------------------------------------------
// Negate (two's complement)
// ---------------------------------------------------------------------------

/// 8-bit negate. CF=1 unless operand is 0. Updates OF, AF, SF, ZF, PF.
#[inline]
pub fn neg8(flags: &mut u16, a: u8) -> u8 {
    let result = sub8(flags, 0, a, false);
    flags::set(flags, Flag::CF, a != 0);
    result
}

/// 16-bit negate. CF=1 unless operand is 0. Updates OF, AF, SF, ZF, PF.
#[inline]
pub fn neg16(flags: &mut u16, a: u16) -> u16 {
    let result = sub16(flags, 0, a, false);
    flags::set(flags, Flag::CF, a != 0);
    result
}

// ---------------------------------------------------------------------------
// Logic operations (CF=0, OF=0, AF undefined — we clear AF)
// ---------------------------------------------------------------------------

/// 8-bit AND. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn and8(flags: &mut u16, a: u8, b: u8) -> u8 {
    let result = a & b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit AND. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn and16(flags: &mut u16, a: u16, b: u16) -> u16 {
    let result = a & b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp16(flags, result);
    result
}

/// 8-bit OR. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn or8(flags: &mut u16, a: u8, b: u8) -> u8 {
    let result = a | b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit OR. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn or16(flags: &mut u16, a: u16, b: u16) -> u16 {
    let result = a | b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp16(flags, result);
    result
}

/// 8-bit XOR. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn xor8(flags: &mut u16, a: u8, b: u8) -> u8 {
    let result = a ^ b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp8(flags, result);
    result
}

/// 16-bit XOR. CF=OF=0, AF undefined (cleared). Updates SF, ZF, PF.
#[inline]
pub fn xor16(flags: &mut u16, a: u16, b: u16) -> u16 {
    let result = a ^ b;
    flags::set(flags, Flag::CF, false);
    flags::set(flags, Flag::OF, false);
    flags::set(flags, Flag::AF, false);
    flags::update_szp16(flags, result);
    result
}

// ---------------------------------------------------------------------------
// NOT (no flags affected)
// ---------------------------------------------------------------------------

/// 8-bit bitwise NOT. No flags affected.
#[inline]
pub fn not8(a: u8) -> u8 {
    !a
}

/// 16-bit bitwise NOT. No flags affected.
#[inline]
pub fn not16(a: u16) -> u16 {
    !a
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::i8088::flags::normalize;

    fn f() -> u16 {
        normalize(0)
    }

    // -- ADD 8-bit --

    #[test]
    fn add8_basic() {
        let mut flags = f();
        assert_eq!(add8(&mut flags, 0x10, 0x20, false), 0x30);
        assert!(!flags::get(flags, Flag::CF));
        assert!(!flags::get(flags, Flag::OF));
        assert!(!flags::get(flags, Flag::ZF));
        assert!(!flags::get(flags, Flag::SF));
    }

    #[test]
    fn add8_carry() {
        let mut flags = f();
        assert_eq!(add8(&mut flags, 0xFF, 0x01, false), 0x00);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn add8_overflow() {
        let mut flags = f();
        // 0x7F + 0x01 = 0x80: positive + positive = negative → OF
        assert_eq!(add8(&mut flags, 0x7F, 0x01, false), 0x80);
        assert!(flags::get(flags, Flag::OF));
        assert!(flags::get(flags, Flag::SF));
        assert!(flags::get(flags, Flag::AF)); // 0xF + 0x1 carries
    }

    #[test]
    fn add8_with_carry_in() {
        let mut flags = f();
        assert_eq!(add8(&mut flags, 0x10, 0x20, true), 0x31);
    }

    #[test]
    fn add8_carry_in_overflow() {
        let mut flags = f();
        assert_eq!(add8(&mut flags, 0xFF, 0x00, true), 0x00);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::ZF));
    }

    // -- ADD 16-bit --

    #[test]
    fn add16_carry() {
        let mut flags = f();
        assert_eq!(add16(&mut flags, 0xFFFF, 0x0001, false), 0x0000);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn add16_overflow() {
        let mut flags = f();
        assert_eq!(add16(&mut flags, 0x7FFF, 0x0001, false), 0x8000);
        assert!(flags::get(flags, Flag::OF));
        assert!(flags::get(flags, Flag::SF));
    }

    // -- SUB 8-bit --

    #[test]
    fn sub8_basic() {
        let mut flags = f();
        assert_eq!(sub8(&mut flags, 0x30, 0x10, false), 0x20);
        assert!(!flags::get(flags, Flag::CF));
        assert!(!flags::get(flags, Flag::OF));
    }

    #[test]
    fn sub8_borrow() {
        let mut flags = f();
        assert_eq!(sub8(&mut flags, 0x00, 0x01, false), 0xFF);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::SF));
    }

    #[test]
    fn sub8_overflow() {
        let mut flags = f();
        // 0x80 - 0x01 = 0x7F: negative - positive = positive → OF
        assert_eq!(sub8(&mut flags, 0x80, 0x01, false), 0x7F);
        assert!(flags::get(flags, Flag::OF));
    }

    #[test]
    fn sub8_with_borrow() {
        let mut flags = f();
        assert_eq!(sub8(&mut flags, 0x30, 0x10, true), 0x1F);
    }

    // -- SUB 16-bit --

    #[test]
    fn sub16_borrow() {
        let mut flags = f();
        assert_eq!(sub16(&mut flags, 0x0000, 0x0001, false), 0xFFFF);
        assert!(flags::get(flags, Flag::CF));
    }

    // -- INC/DEC --

    #[test]
    fn inc8_basic() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, true); // CF should be preserved
        assert_eq!(inc8(&mut flags, 0x41), 0x42);
        assert!(flags::get(flags, Flag::CF)); // preserved
        assert!(!flags::get(flags, Flag::ZF));
    }

    #[test]
    fn inc8_overflow() {
        let mut flags = f();
        assert_eq!(inc8(&mut flags, 0x7F), 0x80);
        assert!(flags::get(flags, Flag::OF));
        assert!(flags::get(flags, Flag::SF));
    }

    #[test]
    fn inc8_wrap() {
        let mut flags = f();
        assert_eq!(inc8(&mut flags, 0xFF), 0x00);
        assert!(flags::get(flags, Flag::ZF));
        assert!(flags::get(flags, Flag::AF));
    }

    #[test]
    fn dec8_basic() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, true);
        assert_eq!(dec8(&mut flags, 0x42), 0x41);
        assert!(flags::get(flags, Flag::CF)); // preserved
    }

    #[test]
    fn dec8_overflow() {
        let mut flags = f();
        assert_eq!(dec8(&mut flags, 0x80), 0x7F);
        assert!(flags::get(flags, Flag::OF));
    }

    #[test]
    fn dec8_to_zero() {
        let mut flags = f();
        assert_eq!(dec8(&mut flags, 0x01), 0x00);
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn inc16_overflow() {
        let mut flags = f();
        assert_eq!(inc16(&mut flags, 0x7FFF), 0x8000);
        assert!(flags::get(flags, Flag::OF));
    }

    #[test]
    fn dec16_overflow() {
        let mut flags = f();
        assert_eq!(dec16(&mut flags, 0x8000), 0x7FFF);
        assert!(flags::get(flags, Flag::OF));
    }

    // -- NEG --

    #[test]
    fn neg8_basic() {
        let mut flags = f();
        assert_eq!(neg8(&mut flags, 0x01), 0xFF);
        assert!(flags::get(flags, Flag::CF)); // CF=1 when operand != 0
        assert!(flags::get(flags, Flag::SF));
    }

    #[test]
    fn neg8_zero() {
        let mut flags = f();
        assert_eq!(neg8(&mut flags, 0x00), 0x00);
        assert!(!flags::get(flags, Flag::CF)); // CF=0 when operand is 0
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn neg8_min() {
        let mut flags = f();
        // NEG 0x80 = 0x80 (overflow: -128 can't be negated in 8 bits)
        assert_eq!(neg8(&mut flags, 0x80), 0x80);
        assert!(flags::get(flags, Flag::OF));
        assert!(flags::get(flags, Flag::CF));
    }

    // -- Logic --

    #[test]
    fn and8_basic() {
        let mut flags = f();
        assert_eq!(and8(&mut flags, 0xF0, 0x0F), 0x00);
        assert!(flags::get(flags, Flag::ZF));
        assert!(!flags::get(flags, Flag::CF));
        assert!(!flags::get(flags, Flag::OF));
    }

    #[test]
    fn or8_basic() {
        let mut flags = f();
        assert_eq!(or8(&mut flags, 0xF0, 0x0F), 0xFF);
        assert!(flags::get(flags, Flag::SF));
        assert!(!flags::get(flags, Flag::CF));
    }

    #[test]
    fn xor8_self() {
        let mut flags = f();
        assert_eq!(xor8(&mut flags, 0xAB, 0xAB), 0x00);
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn not8_basic() {
        assert_eq!(not8(0x00), 0xFF);
        assert_eq!(not8(0xFF), 0x00);
        assert_eq!(not8(0xA5), 0x5A);
    }

    #[test]
    fn not16_basic() {
        assert_eq!(not16(0x0000), 0xFFFF);
        assert_eq!(not16(0xFFFF), 0x0000);
    }
}
