use super::{ExecState, M6809};
use crate::core::{Bus, BusMaster};

impl M6809 {
    // Stack operation state management using temp_addr:
    // Low byte (0-7): The post-byte mask (remaining registers to process)
    // High byte (8-15):
    //   Bit 8: "Second byte" flag for 16-bit registers (0=First/Low, 1=Second/High)
    //   Bits 12-15: Current bit index being processed (0-7)

    /// Generic PUSH operation (PSHS/PSHU)
    /// Pushes registers from High ID (PC=7) to Low ID (CC=0).
    /// Stack grows downward.
    /// 16-bit regs: Push Low byte, then High byte (so High is at lower addr).
    fn op_push<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        use_u: bool,
    ) {
        if cycle == 0 {
            // Cycle 0: Read post-byte
            let mask = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            // Initialize state: Mask in low byte, no current bit selected (high byte 0)
            self.temp_addr = mask as u16;
            self.state = ExecState::Execute(opcode, 1);
            return;
        }

        // Cycle 1+: Process registers
        // Check if we are currently processing a 16-bit register (second byte)
        let mut mask = (self.temp_addr & 0xFF) as u8;
        let state = (self.temp_addr >> 8) as u8;
        let second_byte = (state & 0x01) != 0;
        let mut current_bit = 0;

        if mask == 0 && !second_byte {
            // Done
            self.state = ExecState::Fetch;
            return;
        }

        // If not in the middle of a 16-bit reg, find next register
        if !second_byte {
            // PSH order: PC(7), U/S(6), Y(5), X(4), DP(3), B(2), A(1), CC(0)
            // Find highest set bit
            for i in (0..=7).rev() {
                if (mask & (1 << i)) != 0 {
                    current_bit = i;
                    break;
                }
            }
        } else {
            // Recover current bit from state if needed, though we can re-derive it
            // Actually we need to know which bit we are on if we are in second byte
            // But since we don't clear the mask bit until finished, the loop above finds it again.
            for i in (0..=7).rev() {
                if (mask & (1 << i)) != 0 {
                    current_bit = i;
                    break;
                }
            }
        }

        // Perform Push
        let sp = if use_u { self.u } else { self.s };
        let write_addr = sp.wrapping_sub(1);

        let val_to_push = match current_bit {
            7 => {
                if second_byte {
                    (self.pc >> 8) as u8
                } else {
                    self.pc as u8
                }
            } // PC
            6 => {
                // U or S
                let val = if use_u { self.s } else { self.u };
                if second_byte {
                    (val >> 8) as u8
                } else {
                    val as u8
                }
            }
            5 => {
                if second_byte {
                    (self.y >> 8) as u8
                } else {
                    self.y as u8
                }
            } // Y
            4 => {
                if second_byte {
                    (self.x >> 8) as u8
                } else {
                    self.x as u8
                }
            } // X
            3 => self.dp, // DP
            2 => self.b,  // B
            1 => self.a,  // A
            0 => self.cc, // CC
            _ => 0,
        };

        bus.write(master, write_addr, val_to_push);

        // Update SP
        if use_u {
            self.u = write_addr;
        } else {
            self.s = write_addr;
        }

        // Update state for next cycle
        let is_16bit = current_bit >= 4; // 4,5,6,7 are 16-bit

        if is_16bit {
            if !second_byte {
                // Just pushed Low byte, next is High byte
                // Keep mask bit set, set second_byte flag
                // We don't strictly need to store current_bit in temp_addr because we re-scan mask
                self.temp_addr |= 0x0100;
            } else {
                // Finished 16-bit reg
                mask &= !(1 << current_bit);
                self.temp_addr = mask as u16; // Clear state
            }
        } else {
            // Finished 8-bit reg
            mask &= !(1 << current_bit);
            self.temp_addr = mask as u16; // Clear state
        }

        // Continue execution next cycle
        self.state = ExecState::Execute(opcode, cycle + 1);
    }

    /// Generic PULL operation (PULS/PULU)
    /// Pulls registers from Low ID (CC=0) to High ID (PC=7).
    /// Stack grows upward (incrementing).
    /// 16-bit regs: Pull High byte, then Low byte.
    fn op_pull<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        use_u: bool,
    ) {
        if cycle == 0 {
            let mask = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);
            self.temp_addr = mask as u16;
            self.state = ExecState::Execute(opcode, 1);
            return;
        }

        let mut mask = (self.temp_addr & 0xFF) as u8;
        let state = (self.temp_addr >> 8) as u8;
        let second_byte = (state & 0x01) != 0; // For pull, 2nd byte is Low
        let mut current_bit = 0;

        if mask == 0 && !second_byte {
            self.state = ExecState::Fetch;
            return;
        }

        // PUL order: CC(0), A(1), B(2), DP(3), X(4), Y(5), U/S(6), PC(7)
        // Find lowest set bit
        for i in 0..=7 {
            if (mask & (1 << i)) != 0 {
                current_bit = i;
                break;
            }
        }

        let sp = if use_u { self.u } else { self.s };
        let val = bus.read(master, sp);

        // Update SP
        let new_sp = sp.wrapping_add(1);
        if use_u {
            self.u = new_sp;
        } else {
            self.s = new_sp;
        }

        // Store value
        let is_16bit = current_bit >= 4;

        if is_16bit {
            // 16-bit: Pull High then Low
            // If !second_byte (first step): val is High byte
            // If second_byte (second step): val is Low byte

            // Helper to update 16-bit reg
            let update_reg = |cpu: &mut M6809, val: u8, is_high: bool| {
                let target_reg = match current_bit {
                    7 => &mut cpu.pc,
                    6 => {
                        if use_u {
                            &mut cpu.s
                        } else {
                            &mut cpu.u
                        }
                    }
                    5 => &mut cpu.y,
                    4 => &mut cpu.x,
                    _ => unreachable!(),
                };
                if is_high {
                    *target_reg = (*target_reg & 0x00FF) | ((val as u16) << 8);
                } else {
                    *target_reg = (*target_reg & 0xFF00) | (val as u16);
                }
            };

            update_reg(self, val, !second_byte);

            if !second_byte {
                // Done High byte, next is Low
                self.temp_addr |= 0x0100;
            } else {
                // Done Low byte
                mask &= !(1 << current_bit);
                self.temp_addr = mask as u16;
            }
        } else {
            // 8-bit reg
            match current_bit {
                3 => self.dp = val,
                2 => self.b = val,
                1 => self.a = val,
                0 => self.cc = val,
                _ => unreachable!(),
            }
            mask &= !(1 << current_bit);
            self.temp_addr = mask as u16;
        }

        self.state = ExecState::Execute(opcode, cycle + 1);
    }

    pub(crate) fn op_pshs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.op_push(0x34, cycle, bus, master, false);
    }

    pub(crate) fn op_puls<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.op_pull(0x35, cycle, bus, master, false);
    }

    pub(crate) fn op_pshu<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.op_push(0x36, cycle, bus, master, true);
    }

    pub(crate) fn op_pulu<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.op_pull(0x37, cycle, bus, master, true);
    }
}
