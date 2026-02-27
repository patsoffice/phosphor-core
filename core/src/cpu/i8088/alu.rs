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
// Shift and rotate operations
// ---------------------------------------------------------------------------

/// Dispatch a shift/rotate on an 8-bit value.
/// `op`: 0=ROL, 1=ROR, 2=RCL, 3=RCR, 4=SHL, 5=SHR, 6=SAL(=SHL), 7=SAR.
/// If `count` is 0, no flags are modified and the value is returned unchanged.
pub fn shift_rotate8(flags: &mut u16, val: u8, count: u8, op: u8) -> u8 {
    if count == 0 {
        return val;
    }
    match op & 7 {
        0 => rol8(flags, val, count),
        1 => ror8(flags, val, count),
        2 => rcl8(flags, val, count),
        3 => rcr8(flags, val, count),
        4 | 6 => shl8(flags, val, count),
        5 => shr8(flags, val, count),
        7 => sar8(flags, val, count),
        _ => unreachable!(),
    }
}

/// Dispatch a shift/rotate on a 16-bit value.
pub fn shift_rotate16(flags: &mut u16, val: u16, count: u8, op: u8) -> u16 {
    if count == 0 {
        return val;
    }
    match op & 7 {
        0 => rol16(flags, val, count),
        1 => ror16(flags, val, count),
        2 => rcl16(flags, val, count),
        3 => rcr16(flags, val, count),
        4 | 6 => shl16(flags, val, count),
        5 => shr16(flags, val, count),
        7 => sar16(flags, val, count),
        _ => unreachable!(),
    }
}

// --- Rotates (only CF and OF affected) ---

fn rol8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let eff = (count as u32) % 8;
    let result = val.rotate_left(if eff == 0 { 8 } else { eff });
    let cf = result & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    // OF = MSB XOR CF after final rotation (defined for all counts on 8088)
    flags::set(flags, Flag::OF, ((result >> 7) != 0) ^ cf);
    result
}

fn rol16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let eff = (count as u32) % 16;
    let result = val.rotate_left(if eff == 0 { 16 } else { eff });
    let cf = result & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 15) != 0) ^ cf);
    result
}

fn ror8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let eff = (count as u32) % 8;
    let result = val.rotate_right(if eff == 0 { 8 } else { eff });
    let cf = result & 0x80 != 0;
    flags::set(flags, Flag::CF, cf);
    // OF = XOR of two MSBs of result (defined for all counts on 8088)
    flags::set(flags, Flag::OF, ((result >> 7) ^ (result >> 6)) & 1 != 0);
    result
}

fn ror16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let eff = (count as u32) % 16;
    let result = val.rotate_right(if eff == 0 { 16 } else { eff });
    let cf = result & 0x8000 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 15) ^ (result >> 14)) & 1 != 0);
    result
}

fn rcl8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let eff = (count as u32) % 9;
    if eff == 0 {
        // Full 9-bit cycle(s): value and CF unchanged, but OF is still set
        let cf = flags::get(*flags, Flag::CF);
        flags::set(flags, Flag::OF, ((val >> 7) & 1 != 0) ^ cf);
        return val;
    }
    let cf_in = flags::get(*flags, Flag::CF) as u16;
    let wide = (val as u16) | (cf_in << 8);
    let rotated = ((wide << eff) | (wide >> (9 - eff))) & 0x1FF;
    let result = rotated as u8;
    let cf = (rotated >> 8) & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 7) & 1 != 0) ^ cf);
    result
}

fn rcl16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let eff = (count as u32) % 17;
    if eff == 0 {
        let cf = flags::get(*flags, Flag::CF);
        flags::set(flags, Flag::OF, ((val >> 15) & 1 != 0) ^ cf);
        return val;
    }
    let cf_in = flags::get(*flags, Flag::CF) as u32;
    let wide = (val as u32) | (cf_in << 16);
    let rotated = ((wide << eff) | (wide >> (17 - eff))) & 0x1_FFFF;
    let result = rotated as u16;
    let cf = (rotated >> 16) & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 15) & 1 != 0) ^ cf);
    result
}

fn rcr8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let eff = (count as u32) % 9;
    if eff == 0 {
        flags::set(flags, Flag::OF, ((val >> 7) ^ (val >> 6)) & 1 != 0);
        return val;
    }
    let cf_in = flags::get(*flags, Flag::CF) as u16;
    let wide = (val as u16) | (cf_in << 8);
    let rotated = ((wide >> eff) | (wide << (9 - eff))) & 0x1FF;
    let result = rotated as u8;
    let cf = (rotated >> 8) & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 7) ^ (result >> 6)) & 1 != 0);
    result
}

fn rcr16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let eff = (count as u32) % 17;
    if eff == 0 {
        flags::set(flags, Flag::OF, ((val >> 15) ^ (val >> 14)) & 1 != 0);
        return val;
    }
    let cf_in = flags::get(*flags, Flag::CF) as u32;
    let wide = (val as u32) | (cf_in << 16);
    let rotated = ((wide >> eff) | (wide << (17 - eff))) & 0x1_FFFF;
    let result = rotated as u16;
    let cf = (rotated >> 16) & 1 != 0;
    flags::set(flags, Flag::CF, cf);
    flags::set(flags, Flag::OF, ((result >> 15) ^ (result >> 14)) & 1 != 0);
    result
}

// --- Shifts (CF, OF, SF, ZF, PF affected; AF undefined) ---

fn shl8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let (result, cf) = if count >= 9 {
        (0u8, false)
    } else {
        // count 1..=8; use u16 so count=8 doesn't overflow
        let wide = (val as u16) << count;
        (wide as u8, wide & 0x100 != 0)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp8(flags, result);
    // OF = MSB XOR CF (same as last single-bit SHL step)
    flags::set(flags, Flag::OF, ((result >> 7) & 1 != 0) ^ cf);
    result
}

fn shl16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let count32 = count as u32;
    let (result, cf) = if count32 > 16 {
        (0u16, false)
    } else {
        // count 1..=16; use u32
        let wide = (val as u32) << count32;
        (wide as u16, wide & 0x1_0000 != 0)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp16(flags, result);
    flags::set(flags, Flag::OF, ((result >> 15) & 1 != 0) ^ cf);
    result
}

fn shr8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let (result, cf) = if count > 8 {
        (0u8, false)
    } else if count == 8 {
        (0u8, val & 0x80 != 0)
    } else {
        // count 1..=7
        let cf = (val >> (count - 1)) & 1 != 0;
        (val >> count, cf)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp8(flags, result);
    // OF = MSB of value before last shift step (XOR of two MSBs = MSB since new MSB=0)
    // For count=1: OF = MSB of val. For count>1: OF = MSB of (val >> (count-1)).
    // Since SHR always shifts in 0, OF after last step = MSB of intermediate.
    // More simply: result has MSB=0 (for SHR), so OF = MSB XOR 0 = MSB of result
    // before the last shift. That's (val >> (count-1)) & 0x80.
    // But for count >= 8, intermediate is 0, so OF = 0.
    if count <= 8 {
        let before_last = if count == 1 { val } else { val >> (count - 1) };
        flags::set(flags, Flag::OF, before_last & 0x80 != 0);
    } else {
        flags::set(flags, Flag::OF, false);
    }
    result
}

fn shr16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let (result, cf) = if count as u32 > 16 {
        (0u16, false)
    } else if count == 16 {
        (0u16, val & 0x8000 != 0)
    } else {
        // count 1..=15
        let cf = (val >> (count - 1)) & 1 != 0;
        (val >> count, cf)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp16(flags, result);
    if count <= 16 {
        let before_last = if count == 1 { val } else { val >> (count - 1) };
        flags::set(flags, Flag::OF, before_last & 0x8000 != 0);
    } else {
        flags::set(flags, Flag::OF, false);
    }
    result
}

fn sar8(flags: &mut u16, val: u8, count: u8) -> u8 {
    let (result, cf) = if count >= 8 {
        // All bits become the sign bit
        if val & 0x80 != 0 {
            (0xFF_u8, true)
        } else {
            (0x00_u8, false)
        }
    } else {
        // count 1..=7
        let cf = (val >> (count - 1)) & 1 != 0;
        (((val as i8) >> count) as u8, cf)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp8(flags, result);
    // SAR preserves the sign bit, so OF is always 0 (MSB XOR MSB = 0)
    flags::set(flags, Flag::OF, false);
    result
}

fn sar16(flags: &mut u16, val: u16, count: u8) -> u16 {
    let (result, cf) = if count >= 16 {
        if val & 0x8000 != 0 {
            (0xFFFF_u16, true)
        } else {
            (0x0000_u16, false)
        }
    } else {
        // count 1..=15
        let cf = (val >> (count - 1)) & 1 != 0;
        (((val as i16) >> count) as u16, cf)
    };
    flags::set(flags, Flag::CF, cf);
    flags::update_szp16(flags, result);
    flags::set(flags, Flag::OF, false);
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

    // -- SHL --

    #[test]
    fn shl8_by_1() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x80, 1, 4), 0x00);
        assert!(flags::get(flags, Flag::CF)); // MSB shifted out
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn shl8_by_4() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x0F, 4, 4), 0xF0);
        assert!(!flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::SF));
    }

    #[test]
    fn shl8_by_8() {
        let mut flags = f();
        // SHL 0xFF by 8: CF = bit 0 = 1, result = 0
        assert_eq!(shift_rotate8(&mut flags, 0xFF, 8, 4), 0x00);
        assert!(flags::get(flags, Flag::CF));
    }

    #[test]
    fn shl8_by_9_plus() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0xFF, 9, 4), 0x00);
        assert!(!flags::get(flags, Flag::CF));
    }

    #[test]
    fn shl8_count_zero_no_flags() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, true);
        assert_eq!(shift_rotate8(&mut flags, 0xFF, 0, 4), 0xFF);
        assert!(flags::get(flags, Flag::CF)); // unchanged
    }

    // -- SHR --

    #[test]
    fn shr8_by_1() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x01, 1, 5), 0x00);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::ZF));
    }

    #[test]
    fn shr8_of_flag() {
        let mut flags = f();
        // SHR by 1: OF = MSB of original
        assert_eq!(shift_rotate8(&mut flags, 0x80, 1, 5), 0x40);
        assert!(!flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::OF));
    }

    #[test]
    fn shr16_by_1() {
        let mut flags = f();
        assert_eq!(shift_rotate16(&mut flags, 0x8001, 1, 5), 0x4000);
        assert!(flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::OF)); // MSB was 1
    }

    // -- SAR --

    #[test]
    fn sar8_positive() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x40, 1, 7), 0x20);
        assert!(!flags::get(flags, Flag::CF));
        assert!(!flags::get(flags, Flag::OF)); // SAR OF always 0
    }

    #[test]
    fn sar8_negative() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x80, 1, 7), 0xC0);
        assert!(!flags::get(flags, Flag::CF));
        assert!(flags::get(flags, Flag::SF));
    }

    #[test]
    fn sar8_saturates() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x80, 8, 7), 0xFF);
        assert!(flags::get(flags, Flag::CF));
    }

    // -- ROL --

    #[test]
    fn rol8_by_1() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x80, 1, 0), 0x01);
        assert!(flags::get(flags, Flag::CF)); // bit 0 of result
    }

    #[test]
    fn rol8_by_4() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x12, 4, 0), 0x21);
        assert!(flags::get(flags, Flag::CF));
    }

    // -- ROR --

    #[test]
    fn ror8_by_1() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x01, 1, 1), 0x80);
        assert!(flags::get(flags, Flag::CF)); // MSB of result
    }

    #[test]
    fn ror8_by_4() {
        let mut flags = f();
        assert_eq!(shift_rotate8(&mut flags, 0x12, 4, 1), 0x21);
    }

    // -- RCL --

    #[test]
    fn rcl8_by_1_cf_clear() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, false);
        // RCL 0x80 by 1: old CF (0) goes to bit 0, MSB (1) goes to CF
        assert_eq!(shift_rotate8(&mut flags, 0x80, 1, 2), 0x00);
        assert!(flags::get(flags, Flag::CF));
    }

    #[test]
    fn rcl8_by_1_cf_set() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, true);
        // RCL 0x00 by 1: old CF (1) goes to bit 0, MSB (0) goes to CF
        assert_eq!(shift_rotate8(&mut flags, 0x00, 1, 2), 0x01);
        assert!(!flags::get(flags, Flag::CF));
    }

    // -- RCR --

    #[test]
    fn rcr8_by_1_cf_clear() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, false);
        // RCR 0x01 by 1: old CF (0) goes to MSB, LSB (1) goes to CF
        assert_eq!(shift_rotate8(&mut flags, 0x01, 1, 3), 0x00);
        assert!(flags::get(flags, Flag::CF));
    }

    #[test]
    fn rcr8_by_1_cf_set() {
        let mut flags = f();
        flags::set(&mut flags, Flag::CF, true);
        // RCR 0x00 by 1: old CF (1) goes to MSB, LSB (0) goes to CF
        assert_eq!(shift_rotate8(&mut flags, 0x00, 1, 3), 0x80);
        assert!(!flags::get(flags, Flag::CF));
    }
}
