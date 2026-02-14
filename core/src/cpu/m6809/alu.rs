use super::{CcFlag, ExecState, M6809};
use crate::core::{Bus, BusMaster};

mod binary;
mod shift;
mod unary;
mod word;

impl M6809 {
    /// Helper to set N, Z, V (cleared) flags for logical operations
    #[inline]
    pub(crate) fn set_flags_logical(&mut self, result: u8) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// Helper to set N, Z, V, C flags for arithmetic operations
    #[inline]
    pub(crate) fn set_flags_arithmetic(&mut self, result: u8, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V, C flags for 16-bit arithmetic
    #[inline]
    pub(crate) fn set_flags_arithmetic16(&mut self, result: u16, overflow: bool, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, overflow);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, V (cleared) flags for 16-bit logical operations
    #[inline]
    pub(crate) fn set_flags_logical16(&mut self, result: u16) {
        self.set_flag(CcFlag::N, result & 0x8000 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, false);
    }

    /// The alu_imm function is a generic helper method designed to reduce code duplication for Immediate Addressing Mode ALU instructions (like ADDA #$10, ANDB #$FF, etc.).
    ///
    /// In the Motorola 6809, immediate mode instructions always follow a specific pattern.
    #[inline]
    pub(crate) fn alu_imm<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 0 {
            // 1. Fetch the operand from memory at PC
            let operand = bus.read(master, self.pc);
            // 2. Advance PC to the next instruction
            self.pc = self.pc.wrapping_add(1);
            // 3. Run the specific ALU logic provided by the caller
            operation(self, operand);
            // 4. Return to Fetch state for the next instruction
            self.state = ExecState::Fetch;
        }
    }

    /// ORCC immediate (0x1A): OR immediate value into CC register.
    /// All CC bits may be set by the OR operand.
    /// 3 total cycles: 1 fetch + 2 exec (read operand + internal apply).
    pub(crate) fn op_orcc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(0x1A, 1);
            }
            1 => {
                // Internal cycle — apply
                self.cc |= self.opcode;
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// ANDCC immediate (0x1C): AND immediate value into CC register.
    /// Used to clear specific CC bits (e.g., ANDCC #$FE clears C flag).
    /// 3 total cycles: 1 fetch + 2 exec (read operand + internal apply).
    pub(crate) fn op_andcc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(0x1C, 1);
            }
            1 => {
                // Internal cycle — apply
                self.cc &= self.opcode;
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// Generic helper for Direct Addressing Mode ALU instructions.
    /// Three execute cycles: cycle 0 fetches the address byte and forms DP:addr,
    /// cycle 1 is an internal cycle, cycle 2 reads the operand and runs the operation.
    #[inline]
    pub(crate) fn alu_direct<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Internal cycle (address computation)
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// Generic helper for Extended Addressing Mode ALU instructions.
    /// Four execute cycles:
    /// Cycle 0: Fetch high byte of address.
    /// Cycle 1: Fetch low byte of address, form effective address.
    /// Cycle 2: Internal cycle.
    /// Cycle 3: Read operand from the effective address and run the operation.
    #[inline]
    pub(crate) fn alu_extended<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr |= low;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                // Internal cycle
                self.state = ExecState::Execute(opcode, 3);
            }
            3 => {
                let operand = bus.read(master, self.temp_addr);
                operation(self, operand);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    // --- Shift and Rotate instructions ---

    /// Helper to set N, Z, V, C flags for left-shift/rotate operations (ASL, ROL).
    /// V = N XOR C (post-operation) per 6809 datasheet.
    #[inline]
    pub(crate) fn set_flags_shift(&mut self, result: u8, carry: bool) {
        let n = result & 0x80 != 0;
        self.set_flag(CcFlag::N, n);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::V, n ^ carry);
        self.set_flag(CcFlag::C, carry);
    }

    /// Helper to set N, Z, C flags for right-shift/rotate operations (LSR, ASR, ROR).
    /// V is not affected by right-shift operations.
    #[inline]
    pub(crate) fn set_flags_shift_right(&mut self, result: u8, carry: bool) {
        self.set_flag(CcFlag::N, result & 0x80 != 0);
        self.set_flag(CcFlag::Z, result == 0);
        self.set_flag(CcFlag::C, carry);
    }

    // --- Indexed Addressing Mode ---

    /// Returns the value of the index register selected by 2-bit code.
    /// 0=X, 1=Y, 2=U, 3=S.
    #[inline]
    fn indexed_reg_value(&self, sel: u8) -> u16 {
        match sel & 0x03 {
            0 => self.x,
            1 => self.y,
            2 => self.u,
            3 => self.s,
            _ => unreachable!(),
        }
    }

    /// Sets the index register selected by 2-bit code.
    #[inline]
    fn set_indexed_reg(&mut self, sel: u8, val: u16) {
        match sel & 0x03 {
            0 => self.x = val,
            1 => self.y = val,
            2 => self.u = val,
            3 => self.s = val,
            _ => unreachable!(),
        }
    }

    /// Sign-extends a 5-bit value to 16-bit.
    #[inline]
    fn sign_extend_5(val: u8) -> u16 {
        if val & 0x10 != 0 {
            (val as u16) | 0xFFE0
        } else {
            val as u16
        }
    }

    /// Multi-cycle indexed address resolution state machine.
    ///
    /// Reads the postbyte and any additional offset bytes from memory,
    /// computing the effective address in `self.temp_addr`.
    ///
    /// Returns `true` when the address is ready; `false` if more cycles are needed
    /// (in which case the next ExecState has already been set).
    ///
    /// Uses `self.opcode` as scratch storage for the postbyte (safe because
    /// ExecState holds the original opcode for dispatch).
    pub(crate) fn indexed_resolve<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) -> bool {
        match cycle {
            0 => {
                let postbyte = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.opcode = postbyte; // scratch storage

                if postbyte & 0x80 == 0 {
                    // 5-bit constant offset: bits 6-5 = register, bits 4-0 = offset
                    let reg = self.indexed_reg_value((postbyte >> 5) & 0x03);
                    let offset = Self::sign_extend_5(postbyte & 0x1F);
                    self.temp_addr = reg.wrapping_add(offset);
                    return true;
                }

                let reg_sel = (postbyte >> 5) & 0x03;
                let indirect = postbyte & 0x10 != 0;
                let mode = postbyte & 0x0F;
                let reg = self.indexed_reg_value(reg_sel);

                match mode {
                    0x00 if !indirect => {
                        // ,R+ (post-increment by 1, non-indirect only)
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(1));
                        true
                    }
                    0x01 => {
                        // ,R++ (post-increment by 2)
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(2));
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x02 if !indirect => {
                        // ,-R (pre-decrement by 1, non-indirect only)
                        let new_reg = reg.wrapping_sub(1);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        true
                    }
                    0x03 => {
                        // ,--R (pre-decrement by 2)
                        let new_reg = reg.wrapping_sub(2);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x04 => {
                        // ,R (no offset)
                        self.temp_addr = reg;
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x05 => {
                        // B,R (accumulator B offset)
                        self.temp_addr = reg.wrapping_add(self.b as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x06 => {
                        // A,R (accumulator A offset)
                        self.temp_addr = reg.wrapping_add(self.a as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x08 | 0x0C => {
                        // n8,R (8-bit offset) or n8,PCR (8-bit PC-relative)
                        // Need 1 more byte
                        self.state = ExecState::Execute(opcode, 1);
                        false
                    }
                    0x09 | 0x0D => {
                        // n16,R (16-bit offset) or n16,PCR (16-bit PC-relative)
                        // Need 2 more bytes
                        self.state = ExecState::Execute(opcode, 1);
                        false
                    }
                    0x0B => {
                        // D,R (accumulator D offset)
                        self.temp_addr = reg.wrapping_add(self.get_d());
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0F if indirect => {
                        // [n16] extended indirect (only valid with indirect bit)
                        self.state = ExecState::Execute(opcode, 1);
                        false
                    }
                    _ => {
                        // Undefined mode
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            1 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                match mode {
                    0x08 => {
                        // n8,R: read 8-bit signed offset
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0C => {
                        // n8,PCR: read 8-bit signed offset, PC-relative
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        // PC-relative uses PC after reading the offset byte
                        self.temp_addr = self.pc.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::Execute(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x09 | 0x0D | 0x0F => {
                        // n16,R / n16,PCR / [n16]: read high byte of 16-bit offset
                        let high = bus.read(master, self.pc) as u16;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = high << 8;
                        self.state = ExecState::Execute(opcode, 2);
                        false
                    }
                    _ => {
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            2 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let offset16 = self.temp_addr | low;

                match mode {
                    0x09 => {
                        // n16,R
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset16);
                    }
                    0x0D => {
                        // n16,PCR (PC after reading offset bytes)
                        self.temp_addr = self.pc.wrapping_add(offset16);
                    }
                    0x0F => {
                        // [n16] extended indirect
                        self.temp_addr = offset16;
                        // Always indirect
                        self.state = ExecState::Execute(opcode, 10);
                        return false;
                    }
                    _ => {}
                }

                if indirect {
                    self.state = ExecState::Execute(opcode, 10);
                    false
                } else {
                    true
                }
            }
            // Indirect resolution: read 16-bit pointer from temp_addr
            10 => {
                let high = bus.read(master, self.temp_addr);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                // Reuse opcode scratch to store high byte (postbyte no longer needed)
                self.opcode = high;
                self.state = ExecState::Execute(opcode, 11);
                false
            }
            11 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let high = (self.opcode as u16) << 8;
                self.temp_addr = high | low;
                true
            }
            _ => false,
        }
    }

    /// Multi-cycle indexed address resolution for Page 2 instructions.
    /// Identical to `indexed_resolve` but uses `ExecutePage2` state transitions.
    pub(crate) fn indexed_resolve_page2<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) -> bool {
        match cycle {
            0 => {
                let postbyte = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.opcode = postbyte;

                if postbyte & 0x80 == 0 {
                    let reg = self.indexed_reg_value((postbyte >> 5) & 0x03);
                    let offset = Self::sign_extend_5(postbyte & 0x1F);
                    self.temp_addr = reg.wrapping_add(offset);
                    return true;
                }

                let reg_sel = (postbyte >> 5) & 0x03;
                let indirect = postbyte & 0x10 != 0;
                let mode = postbyte & 0x0F;
                let reg = self.indexed_reg_value(reg_sel);

                match mode {
                    0x00 if !indirect => {
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(1));
                        true
                    }
                    0x01 => {
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(2));
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x02 if !indirect => {
                        let new_reg = reg.wrapping_sub(1);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        true
                    }
                    0x03 => {
                        let new_reg = reg.wrapping_sub(2);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x04 => {
                        self.temp_addr = reg;
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x05 => {
                        self.temp_addr = reg.wrapping_add(self.b as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x06 => {
                        self.temp_addr = reg.wrapping_add(self.a as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x08 | 0x0C => {
                        self.state = ExecState::ExecutePage2(opcode, 1);
                        false
                    }
                    0x09 | 0x0D => {
                        self.state = ExecState::ExecutePage2(opcode, 1);
                        false
                    }
                    0x0B => {
                        self.temp_addr = reg.wrapping_add(self.get_d());
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0F if indirect => {
                        self.state = ExecState::ExecutePage2(opcode, 1);
                        false
                    }
                    _ => {
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            1 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                match mode {
                    0x08 => {
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0C => {
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = self.pc.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage2(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x09 | 0x0D | 0x0F => {
                        let high = bus.read(master, self.pc) as u16;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = high << 8;
                        self.state = ExecState::ExecutePage2(opcode, 2);
                        false
                    }
                    _ => {
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            2 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let offset16 = self.temp_addr | low;

                match mode {
                    0x09 => {
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset16);
                    }
                    0x0D => {
                        self.temp_addr = self.pc.wrapping_add(offset16);
                    }
                    0x0F => {
                        self.temp_addr = offset16;
                        self.state = ExecState::ExecutePage2(opcode, 10);
                        return false;
                    }
                    _ => {}
                }

                if indirect {
                    self.state = ExecState::ExecutePage2(opcode, 10);
                    false
                } else {
                    true
                }
            }
            10 => {
                let high = bus.read(master, self.temp_addr);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.opcode = high;
                self.state = ExecState::ExecutePage2(opcode, 11);
                false
            }
            11 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let high = (self.opcode as u16) << 8;
                self.temp_addr = high | low;
                true
            }
            _ => false,
        }
    }

    /// Multi-cycle indexed address resolution for Page 3 instructions.
    /// Identical to `indexed_resolve` but uses `ExecutePage3` state transitions.
    pub(crate) fn indexed_resolve_page3<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) -> bool {
        match cycle {
            0 => {
                let postbyte = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.opcode = postbyte;

                if postbyte & 0x80 == 0 {
                    let reg = self.indexed_reg_value((postbyte >> 5) & 0x03);
                    let offset = Self::sign_extend_5(postbyte & 0x1F);
                    self.temp_addr = reg.wrapping_add(offset);
                    return true;
                }

                let reg_sel = (postbyte >> 5) & 0x03;
                let indirect = postbyte & 0x10 != 0;
                let mode = postbyte & 0x0F;
                let reg = self.indexed_reg_value(reg_sel);

                match mode {
                    0x00 if !indirect => {
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(1));
                        true
                    }
                    0x01 => {
                        self.temp_addr = reg;
                        self.set_indexed_reg(reg_sel, reg.wrapping_add(2));
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x02 if !indirect => {
                        let new_reg = reg.wrapping_sub(1);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        true
                    }
                    0x03 => {
                        let new_reg = reg.wrapping_sub(2);
                        self.set_indexed_reg(reg_sel, new_reg);
                        self.temp_addr = new_reg;
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x04 => {
                        self.temp_addr = reg;
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x05 => {
                        self.temp_addr = reg.wrapping_add(self.b as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x06 => {
                        self.temp_addr = reg.wrapping_add(self.a as i8 as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x08 | 0x0C => {
                        self.state = ExecState::ExecutePage3(opcode, 1);
                        false
                    }
                    0x09 | 0x0D => {
                        self.state = ExecState::ExecutePage3(opcode, 1);
                        false
                    }
                    0x0B => {
                        self.temp_addr = reg.wrapping_add(self.get_d());
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0F if indirect => {
                        self.state = ExecState::ExecutePage3(opcode, 1);
                        false
                    }
                    _ => {
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            1 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                match mode {
                    0x08 => {
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x0C => {
                        let offset = bus.read(master, self.pc) as i8;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = self.pc.wrapping_add(offset as i16 as u16);
                        if indirect {
                            self.state = ExecState::ExecutePage3(opcode, 10);
                            false
                        } else {
                            true
                        }
                    }
                    0x09 | 0x0D | 0x0F => {
                        let high = bus.read(master, self.pc) as u16;
                        self.pc = self.pc.wrapping_add(1);
                        self.temp_addr = high << 8;
                        self.state = ExecState::ExecutePage3(opcode, 2);
                        false
                    }
                    _ => {
                        self.state = ExecState::Fetch;
                        false
                    }
                }
            }
            2 => {
                let postbyte = self.opcode;
                let mode = postbyte & 0x0F;
                let indirect = postbyte & 0x10 != 0;
                let reg_sel = (postbyte >> 5) & 0x03;

                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let offset16 = self.temp_addr | low;

                match mode {
                    0x09 => {
                        let reg = self.indexed_reg_value(reg_sel);
                        self.temp_addr = reg.wrapping_add(offset16);
                    }
                    0x0D => {
                        self.temp_addr = self.pc.wrapping_add(offset16);
                    }
                    0x0F => {
                        self.temp_addr = offset16;
                        self.state = ExecState::ExecutePage3(opcode, 10);
                        return false;
                    }
                    _ => {}
                }

                if indirect {
                    self.state = ExecState::ExecutePage3(opcode, 10);
                    false
                } else {
                    true
                }
            }
            10 => {
                let high = bus.read(master, self.temp_addr);
                self.temp_addr = self.temp_addr.wrapping_add(1);
                self.opcode = high;
                self.state = ExecState::ExecutePage3(opcode, 11);
                false
            }
            11 => {
                let low = bus.read(master, self.temp_addr) as u16;
                let high = (self.opcode as u16) << 8;
                self.temp_addr = high | low;
                true
            }
            _ => false,
        }
    }

    /// Generic helper for Indexed Addressing Mode ALU instructions.
    /// Variable execute cycles: address resolution via postbyte, then operand read.
    /// Cycle 50 is the sentinel for "address resolved, read operand."
    pub(crate) fn alu_indexed<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 50 {
            let operand = bus.read(master, self.temp_addr);
            operation(self, operand);
            self.state = ExecState::Fetch;
            return;
        }
        if self.indexed_resolve(opcode, cycle, bus, master) {
            self.state = ExecState::Execute(opcode, 50);
        }
    }

    /// Generic helper for Indexed Addressing Mode read-modify-write instructions.
    /// Used by memory-modify ops in the 0x60-0x6F range (NEG, COM, LSR, etc.).
    /// Cycle 50: read value from EA. Cycle 51: modify and write back.
    pub(crate) fn rmw_indexed<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            50 => {
                self.opcode = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(opcode, 51);
            }
            51 => {
                let result = operation(self, self.opcode);
                bus.write(master, self.temp_addr, result);
                self.state = ExecState::Fetch;
            }
            _ => {
                if self.indexed_resolve(opcode, cycle, bus, master) {
                    self.state = ExecState::Execute(opcode, 50);
                }
            }
        }
    }

    /// Generic helper for Direct Addressing Mode read-modify-write instructions.
    /// Used by memory-modify ops in the 0x00-0x0F range (NEG, COM, LSR, etc.).
    /// Cycle 0: fetch address byte, form DP:addr.
    /// Cycle 1: internal cycle.
    /// Cycle 2: read value from EA.
    /// Cycle 3: internal cycle (modify).
    /// Cycle 4: write result back.
    pub(crate) fn rmw_direct<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            0 => {
                let addr = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = ((self.dp as u16) << 8) | addr;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Internal cycle
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                self.opcode = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(opcode, 3);
            }
            3 => {
                // Internal cycle (modify)
                self.state = ExecState::Execute(opcode, 4);
            }
            4 => {
                let result = operation(self, self.opcode);
                bus.write(master, self.temp_addr, result);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// Generic helper for Extended Addressing Mode read-modify-write instructions.
    /// Used by memory-modify ops in the 0x70-0x7F range (NEG, COM, LSR, etc.).
    /// Cycle 0: fetch address high byte.
    /// Cycle 1: fetch address low byte.
    /// Cycle 2: internal cycle.
    /// Cycle 3: read value from EA.
    /// Cycle 4: internal cycle (modify).
    /// Cycle 5: write result back.
    pub(crate) fn rmw_extended<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8) -> u8,
    {
        match cycle {
            0 => {
                let high = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = high << 8;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                let low = bus.read(master, self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr |= low;
                self.state = ExecState::Execute(opcode, 2);
            }
            2 => {
                // Internal cycle
                self.state = ExecState::Execute(opcode, 3);
            }
            3 => {
                self.opcode = bus.read(master, self.temp_addr);
                self.state = ExecState::Execute(opcode, 4);
            }
            4 => {
                // Internal cycle (modify)
                self.state = ExecState::Execute(opcode, 5);
            }
            5 => {
                let result = operation(self, self.opcode);
                bus.write(master, self.temp_addr, result);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// Generic helper for Page 2 Indexed Addressing Mode ALU instructions.
    /// Same as `alu_indexed` but uses `ExecutePage2` state transitions.
    #[allow(dead_code)]
    pub(crate) fn alu_indexed_page2<B: Bus<Address = u16, Data = u8> + ?Sized, F>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        operation: F,
    ) where
        F: FnOnce(&mut Self, u8),
    {
        if cycle == 50 {
            let operand = bus.read(master, self.temp_addr);
            operation(self, operand);
            self.state = ExecState::Fetch;
            return;
        }
        if self.indexed_resolve_page2(opcode, cycle, bus, master) {
            self.state = ExecState::ExecutePage2(opcode, 50);
        }
    }
}
