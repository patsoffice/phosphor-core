use super::{CcFlag, ExecState, M6800};
use crate::core::{Bus, BusMaster};

impl M6800 {
    // --- Push / Pull (4 cycles each: 1 fetch + 3 execute) ---

    /// PSHA (0x36): Push A onto stack.
    /// No flags affected.
    pub(crate) fn op_psha<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 | 1 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            2 => {
                bus.write(master, self.sp, self.a);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PSHB (0x37): Push B onto stack.
    /// No flags affected.
    pub(crate) fn op_pshb<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 | 1 => {
                self.state = ExecState::Execute(self.opcode, cycle + 1);
            }
            2 => {
                bus.write(master, self.sp, self.b);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PULA (0x32): Pull A from stack.
    /// No flags affected.
    pub(crate) fn op_pula<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.a = bus.read(master, self.sp);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// PULB (0x33): Pull B from stack.
    /// No flags affected.
    pub(crate) fn op_pulb<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            0 => {
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.b = bus.read(master, self.sp);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            2 => {
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- SWI (0x3F): Software Interrupt ---
    // 12 cycles: 1 fetch + 2 internal + 9 interrupt (push 7 + vector 2)

    /// SWI (0x3F): Software interrupt.
    /// Pushes all registers, sets I flag, jumps to SWI vector (0xFFFA).
    pub(crate) fn op_swi(&mut self, cycle: u8) {
        match cycle {
            0 => {
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.interrupt_type = 3; // SWI
                self.state = ExecState::Interrupt(0);
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- RTI (0x3B): Return from Interrupt ---
    // 10 cycles: 1 fetch + 9 execute (pull 7 bytes + 2 internal)

    /// RTI (0x3B): Return from interrupt.
    /// Pulls CC, B, A, XH, XL, PCH, PCL from stack. All flags restored.
    pub(crate) fn op_rti<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            // Pull CC
            0 => {
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            1 => {
                self.cc = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            // Pull B
            2 => {
                self.b = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            // Pull A
            3 => {
                self.a = bus.read(master, self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            // Pull X high
            4 => {
                self.temp_addr = (bus.read(master, self.sp) as u16) << 8;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            // Pull X low
            5 => {
                self.temp_addr |= bus.read(master, self.sp) as u16;
                self.x = self.temp_addr;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 6);
            }
            // Pull PC high
            6 => {
                self.temp_addr = (bus.read(master, self.sp) as u16) << 8;
                self.sp = self.sp.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 7);
            }
            // Pull PC low
            7 => {
                self.temp_addr |= bus.read(master, self.sp) as u16;
                self.pc = self.temp_addr;
                self.state = ExecState::Execute(self.opcode, 8);
            }
            // Internal cycle, done
            8 => {
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- WAI (0x3E): Wait for Interrupt ---
    // 9 cycles: 1 fetch + 8 execute (push 7 bytes + enter wait state)

    /// WAI (0x3E): Wait for interrupt.
    /// Pushes all registers then enters wait state until NMI or IRQ.
    pub(crate) fn op_wai<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self, cycle: u8, bus: &mut B, master: BusMaster,
    ) {
        match cycle {
            // Push PC low
            0 => {
                bus.write(master, self.sp, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 1);
            }
            // Push PC high
            1 => {
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 2);
            }
            // Push X low
            2 => {
                bus.write(master, self.sp, self.x as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 3);
            }
            // Push X high
            3 => {
                bus.write(master, self.sp, (self.x >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 4);
            }
            // Push A
            4 => {
                bus.write(master, self.sp, self.a);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 5);
            }
            // Push B
            5 => {
                bus.write(master, self.sp, self.b);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 6);
            }
            // Push CC
            6 => {
                bus.write(master, self.sp, self.cc);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Execute(self.opcode, 7);
            }
            // Enter wait state
            7 => {
                self.state = ExecState::WaitForInterrupt;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // --- Interrupt handling infrastructure ---

    /// Execute hardware interrupt sequence (NMI/IRQ) or SWI.
    /// Pushes 7 bytes: PCL, PCH, XL, XH, A, B, CC then reads vector.
    pub(crate) fn execute_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            // Push PC low
            0 => {
                bus.write(master, self.sp, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(1);
            }
            // Push PC high
            1 => {
                bus.write(master, self.sp, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(2);
            }
            // Push X low
            2 => {
                bus.write(master, self.sp, self.x as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(3);
            }
            // Push X high
            3 => {
                bus.write(master, self.sp, (self.x >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(4);
            }
            // Push A
            4 => {
                bus.write(master, self.sp, self.a);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(5);
            }
            // Push B
            5 => {
                bus.write(master, self.sp, self.b);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(6);
            }
            // Push CC
            6 => {
                bus.write(master, self.sp, self.cc);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(7);
            }
            // Set I flag, read vector high byte
            7 => {
                self.set_flag(CcFlag::I, true);
                let vector_addr = match self.interrupt_type {
                    1 => 0xFFFC, // NMI
                    2 => 0xFFF8, // IRQ
                    3 => 0xFFFA, // SWI
                    _ => 0xFFFE, // RESET
                };
                self.temp_addr = (bus.read(master, vector_addr) as u16) << 8;
                self.state = ExecState::Interrupt(8);
            }
            // Read vector low byte
            8 => {
                let vector_addr = match self.interrupt_type {
                    1 => 0xFFFD, // NMI
                    2 => 0xFFF9, // IRQ
                    3 => 0xFFFB, // SWI
                    _ => 0xFFFF, // RESET
                };
                self.temp_addr |= bus.read(master, vector_addr) as u16;
                self.pc = self.temp_addr;
                self.interrupt_type = 0;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// WAI wait state: wait for interrupt, registers already pushed.
    pub(crate) fn wait_for_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        let ints = bus.check_interrupts(master);

        // NMI edge detection
        let nmi_edge = ints.nmi && !self.nmi_previous;
        self.nmi_previous = ints.nmi;

        if nmi_edge {
            self.interrupt_type = 1;
            // Skip pushing (already done by WAI), go straight to vector fetch
            self.set_flag(CcFlag::I, true);
            self.state = ExecState::Interrupt(7);
            return;
        }

        if ints.irq && (self.cc & CcFlag::I as u8) == 0 {
            self.interrupt_type = 2;
            self.set_flag(CcFlag::I, true);
            self.state = ExecState::Interrupt(7);
        }
        // Otherwise stay in WaitForInterrupt
    }
}
