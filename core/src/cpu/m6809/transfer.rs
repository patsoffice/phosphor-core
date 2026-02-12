use super::{ExecState, M6809};
use crate::core::{Bus, BusMaster};

impl M6809 {
    // Register IDs for TFR/EXG
    // 0=D, 1=X, 2=Y, 3=U, 4=S, 5=PC, 8=A, 9=B, 10=CC, 11=DP

    fn get_reg_val(&self, id: u8) -> u16 {
        match id {
            0 => self.get_d(),
            1 => self.x,
            2 => self.y,
            3 => self.u,
            4 => self.s,
            5 => self.pc,
            8 => self.a as u16,
            9 => self.b as u16,
            10 => self.cc as u16,
            11 => self.dp as u16,
            _ => 0, // Undefined
        }
    }

    fn set_reg_val(&mut self, id: u8, val: u16) {
        match id {
            0 => self.set_d(val),
            1 => self.x = val,
            2 => self.y = val,
            3 => self.u = val,
            4 => self.s = val,
            5 => self.pc = val,
            8 => self.a = val as u8,
            9 => self.b = val as u8,
            10 => self.cc = val as u8,
            11 => self.dp = val as u8,
            _ => {} // Undefined
        }
    }

    fn is_16bit(id: u8) -> bool {
        id < 8
    }

    /// TFR immediate (0x1F): Transfer register R1 to R2.
    /// Operand: High nibble = Source, Low nibble = Dest.
    /// Condition: Sizes must match (8->8 or 16->16).
    pub(crate) fn op_tfr<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        if cycle == 0 {
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);

            let src = operand >> 4;
            let dst = operand & 0x0F;

            if Self::is_16bit(src) == Self::is_16bit(dst) {
                let val = self.get_reg_val(src);
                self.set_reg_val(dst, val);
            }
            // If sizes mismatch, undefined behavior (we do nothing)

            self.state = ExecState::Fetch;
        }
    }

    /// EXG immediate (0x1E): Exchange registers R1 and R2.
    /// Operand: High nibble = R1, Low nibble = R2.
    /// Condition: Sizes must match.
    pub(crate) fn op_exg<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        if cycle == 0 {
            let operand = bus.read(master, self.pc);
            self.pc = self.pc.wrapping_add(1);

            let r1 = operand >> 4;
            let r2 = operand & 0x0F;

            if Self::is_16bit(r1) == Self::is_16bit(r2) {
                let val1 = self.get_reg_val(r1);
                let val2 = self.get_reg_val(r2);
                self.set_reg_val(r1, val2);
                self.set_reg_val(r2, val1);
            }

            self.state = ExecState::Fetch;
        }
    }
}
