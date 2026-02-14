use super::{CcFlag, ExecState, M6800};
use crate::core::{Bus, BusMaster, bus::InterruptState};

impl M6800 {
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
