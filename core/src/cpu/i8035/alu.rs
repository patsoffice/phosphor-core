use super::{I8035, PswFlag};

impl I8035 {
    // --- Arithmetic ---

    /// ADD A,operand: A = A + operand.
    /// CY, AC affected.
    pub(crate) fn perform_add(&mut self, operand: u8) {
        let a = self.a;
        let result16 = a as u16 + operand as u16;
        self.a = result16 as u8;
        self.set_flag(PswFlag::CY, result16 > 0xFF);
        self.set_flag(PswFlag::AC, (a & 0x0F) + (operand & 0x0F) > 0x0F);
    }

    /// ADDC A,operand: A = A + operand + CY.
    /// CY, AC affected.
    pub(crate) fn perform_addc(&mut self, operand: u8) {
        let a = self.a;
        let carry = if self.flag_set(PswFlag::CY) { 1u16 } else { 0 };
        let result16 = a as u16 + operand as u16 + carry;
        self.a = result16 as u8;
        self.set_flag(PswFlag::CY, result16 > 0xFF);
        self.set_flag(
            PswFlag::AC,
            (a & 0x0F) + (operand & 0x0F) + carry as u8 > 0x0F,
        );
    }

    // --- Logic (no flags affected) ---

    /// ANL A,operand: A = A & operand.
    pub(crate) fn perform_anl(&mut self, operand: u8) {
        self.a &= operand;
    }

    /// ORL A,operand: A = A | operand.
    pub(crate) fn perform_orl(&mut self, operand: u8) {
        self.a |= operand;
    }

    /// XRL A,operand: A = A ^ operand.
    pub(crate) fn perform_xrl(&mut self, operand: u8) {
        self.a ^= operand;
    }

    // --- Increment / Decrement (no flags affected) ---

    /// INC: val + 1, wrapping. No flags affected.
    pub(crate) fn perform_inc(val: u8) -> u8 {
        val.wrapping_add(1)
    }

    /// DEC: val - 1, wrapping. No flags affected.
    pub(crate) fn perform_dec(val: u8) -> u8 {
        val.wrapping_sub(1)
    }

    // --- BCD ---

    /// DA A: Decimal adjust accumulator after BCD addition.
    /// Low nibble is corrected first (+6 if >9 or AC), then high nibble is
    /// checked on the *corrected* value (+6 if >9 or CY), per the Intel
    /// MCS-48 User's Manual. CY can be set but never cleared.
    pub(crate) fn perform_da(&mut self) {
        let mut carry = self.flag_set(PswFlag::CY);

        if (self.a & 0x0F) > 0x09 || self.flag_set(PswFlag::AC) {
            self.a = self.a.wrapping_add(0x06);
        }

        if (self.a & 0xF0) > 0x90 || carry {
            self.a = self.a.wrapping_add(0x60);
            carry = true;
        }

        self.set_flag(PswFlag::CY, carry);
    }

    // --- Unary (no flags affected) ---

    /// CLR A: A = 0.
    pub(crate) fn perform_clr_a(&mut self) {
        self.a = 0;
    }

    /// CPL A: A = ~A (ones complement).
    pub(crate) fn perform_cpl_a(&mut self) {
        self.a = !self.a;
    }

    // --- Rotate ---

    /// RL A: Rotate left. Bit 7 wraps to bit 0. No flags affected.
    pub(crate) fn perform_rl(&mut self) {
        self.a = self.a.rotate_left(1);
    }

    /// RLC A: Rotate left through carry. Bit 7 → CY, old CY → bit 0.
    /// CY affected.
    pub(crate) fn perform_rlc(&mut self) {
        let old_carry = if self.flag_set(PswFlag::CY) { 1 } else { 0 };
        self.set_flag(PswFlag::CY, self.a & 0x80 != 0);
        self.a = (self.a << 1) | old_carry;
    }

    /// RR A: Rotate right. Bit 0 wraps to bit 7. No flags affected.
    pub(crate) fn perform_rr(&mut self) {
        self.a = self.a.rotate_right(1);
    }

    /// RRC A: Rotate right through carry. Bit 0 → CY, old CY → bit 7.
    /// CY affected.
    pub(crate) fn perform_rrc(&mut self) {
        let old_carry = if self.flag_set(PswFlag::CY) { 0x80 } else { 0 };
        self.set_flag(PswFlag::CY, self.a & 0x01 != 0);
        self.a = (self.a >> 1) | old_carry;
    }

    // --- Nibble ---

    /// SWAP A: Exchange upper and lower nibbles. No flags affected.
    pub(crate) fn perform_swap(&mut self) {
        self.a = self.a.rotate_left(4);
    }
}
