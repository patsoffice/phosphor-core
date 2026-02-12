use super::{CcFlag, ExecState, M6809};
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

    // --- SWI / RTI ---

    /// Returns the register value to push for a given SWI push cycle (0-11).
    /// Push order matches PSHS hardware: PC, U, Y, X, DP, B, A, CC.
    /// For 16-bit regs, low byte is pushed first, high byte second
    /// (so high byte ends up at lower address = big-endian in memory).
    fn swi_push_byte(&self, push_cycle: u8) -> u8 {
        match push_cycle {
            0 => self.pc as u8,
            1 => (self.pc >> 8) as u8,
            2 => self.u as u8,
            3 => (self.u >> 8) as u8,
            4 => self.y as u8,
            5 => (self.y >> 8) as u8,
            6 => self.x as u8,
            7 => (self.x >> 8) as u8,
            8 => self.dp,
            9 => self.b,
            10 => self.a,
            11 => self.cc,
            _ => 0,
        }
    }

    /// SWI (0x3F): Software Interrupt.
    /// Sets E flag, pushes all registers onto S stack, sets I+F flags,
    /// then vectors through 0xFFFA/0xFFFB.
    /// No flags affected directly (E, I, F are set as part of interrupt sequence).
    pub(crate) fn op_swi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Set E flag to indicate entire state saved
                self.cc |= CcFlag::E as u8;
                // Push PC low byte
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(0));
                self.state = ExecState::Execute(0x3F, 1);
            }
            c @ 1..=11 => {
                // Push remaining registers
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(c));
                self.state = ExecState::Execute(0x3F, c + 1);
            }
            12 => {
                // Mask both IRQ and FIRQ
                self.cc |= CcFlag::I as u8 | CcFlag::F as u8;
                // Read vector high byte
                let hi = bus.read(master, 0xFFFA);
                self.temp_addr = (hi as u16) << 8;
                self.state = ExecState::Execute(0x3F, 13);
            }
            13 => {
                // Read vector low byte, jump to handler
                let lo = bus.read(master, 0xFFFB);
                self.pc = self.temp_addr | (lo as u16);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SWI2 (0x103F): Software Interrupt 2.
    /// Sets E flag, pushes all registers onto S stack. Does NOT mask interrupts.
    /// Vectors through 0xFFF4/0xFFF5.
    pub(crate) fn op_swi2<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.cc |= CcFlag::E as u8;
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(0));
                self.state = ExecState::ExecutePage2(0x3F, 1);
            }
            c @ 1..=11 => {
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(c));
                self.state = ExecState::ExecutePage2(0x3F, c + 1);
            }
            12 => {
                // SWI2 does NOT mask interrupts
                let hi = bus.read(master, 0xFFF4);
                self.temp_addr = (hi as u16) << 8;
                self.state = ExecState::ExecutePage2(0x3F, 13);
            }
            13 => {
                let lo = bus.read(master, 0xFFF5);
                self.pc = self.temp_addr | (lo as u16);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// SWI3 (0x113F): Software Interrupt 3.
    /// Sets E flag, pushes all registers onto S stack. Does NOT mask interrupts.
    /// Vectors through 0xFFF2/0xFFF3.
    pub(crate) fn op_swi3<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.cc |= CcFlag::E as u8;
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(0));
                self.state = ExecState::ExecutePage3(0x3F, 1);
            }
            c @ 1..=11 => {
                self.s = self.s.wrapping_sub(1);
                bus.write(master, self.s, self.swi_push_byte(c));
                self.state = ExecState::ExecutePage3(0x3F, c + 1);
            }
            12 => {
                // SWI3 does NOT mask interrupts
                let hi = bus.read(master, 0xFFF2);
                self.temp_addr = (hi as u16) << 8;
                self.state = ExecState::ExecutePage3(0x3F, 13);
            }
            13 => {
                let lo = bus.read(master, 0xFFF3);
                self.pc = self.temp_addr | (lo as u16);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    /// RTI (0x3B): Return from Interrupt.
    /// Pulls CC from S stack. If E flag is set in pulled CC, pulls all registers
    /// (A, B, DP, X, Y, U, PC). If E is clear, pulls only PC (fast FIRQ return).
    /// CC is restored from the stack (all flags affected).
    pub(crate) fn op_rti<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Pull CC
                self.cc = bus.read(master, self.s);
                self.s = self.s.wrapping_add(1);
                if self.cc & (CcFlag::E as u8) != 0 {
                    // E set: pull all registers
                    self.state = ExecState::Execute(0x3B, 1);
                } else {
                    // E clear: pull PC only (fast FIRQ return)
                    self.state = ExecState::Execute(0x3B, 10);
                }
            }
            1 => {
                self.a = bus.read(master, self.s);
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 2);
            }
            2 => {
                self.b = bus.read(master, self.s);
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 3);
            }
            3 => {
                self.dp = bus.read(master, self.s);
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 4);
            }
            4 => {
                // X high byte
                self.x = (bus.read(master, self.s) as u16) << 8;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 5);
            }
            5 => {
                // X low byte
                self.x |= bus.read(master, self.s) as u16;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 6);
            }
            6 => {
                // Y high byte
                self.y = (bus.read(master, self.s) as u16) << 8;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 7);
            }
            7 => {
                // Y low byte
                self.y |= bus.read(master, self.s) as u16;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 8);
            }
            8 => {
                // U high byte
                self.u = (bus.read(master, self.s) as u16) << 8;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 9);
            }
            9 => {
                // U low byte
                self.u |= bus.read(master, self.s) as u16;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 10);
            }
            10 => {
                // PC high byte (shared by E=0 and E=1 paths)
                self.pc = (bus.read(master, self.s) as u16) << 8;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Execute(0x3B, 11);
            }
            11 => {
                // PC low byte
                self.pc |= bus.read(master, self.s) as u16;
                self.s = self.s.wrapping_add(1);
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }
}
