use super::{CcFlag, ExecState, M6809};
use crate::core::{Bus, BusMaster};

impl M6809 {
    /// Generic helper for short branch instructions (8-bit offset).
    /// Takes 3 cycles: 1 (Fetch) + 1 (Read Offset) + 1 (Calc/Idle).
    fn branch_short<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
        condition: bool,
    ) {
        match cycle {
            0 => {
                // Cycle 1: Fetch offset
                let offset = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.temp_addr = offset as u16;
                self.state = ExecState::Execute(opcode, 1);
            }
            1 => {
                // Cycle 2: Internal operation (add offset if taken)
                if condition {
                    let offset = self.temp_addr as u8 as i8;
                    self.pc = self.pc.wrapping_add(offset as u16);
                }
                self.state = ExecState::Fetch;
            }
            _ => {}
        }
    }

    // 0x20 BRA: Branch Always
    pub(crate) fn op_bra<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch_short(opcode, cycle, bus, master, true);
    }

    // 0x21 BRN: Branch Never (effectively a 3-cycle, 2-byte NOP)
    pub(crate) fn op_brn<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        self.branch_short(opcode, cycle, bus, master, false);
    }

    // 0x22 BHI: Branch if Higher (Unsigned >) -> C=0 and Z=0
    pub(crate) fn op_bhi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & ((CcFlag::C as u8) | (CcFlag::Z as u8))) == 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x23 BLS: Branch if Lower or Same (Unsigned <=) -> C=1 or Z=1
    pub(crate) fn op_bls<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & ((CcFlag::C as u8) | (CcFlag::Z as u8))) != 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x24 BCC: Branch if Carry Clear (Higher or Same) -> C=0
    pub(crate) fn op_bcc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::C as u8)) == 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x25 BCS: Branch if Carry Set (Lower) -> C=1
    pub(crate) fn op_bcs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::C as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x26 BNE: Branch if Not Equal (Z=0)
    pub(crate) fn op_bne<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::Z as u8)) == 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x27 BEQ: Branch if Equal (Z=1)
    pub(crate) fn op_beq<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::Z as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x28 BVC: Branch if Overflow Clear (V=0)
    pub(crate) fn op_bvc<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::V as u8)) == 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x29 BVS: Branch if Overflow Set (V=1)
    pub(crate) fn op_bvs<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x2A BPL: Branch if Plus (N=0)
    pub(crate) fn op_bpl<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::N as u8)) == 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x2B BMI: Branch if Minus (N=1)
    pub(crate) fn op_bmi<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let cond = (self.cc & (CcFlag::N as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, cond);
    }

    // 0x2C BGE: Branch if Greater or Equal (Signed) -> N == V
    pub(crate) fn op_bge<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, n == v);
    }

    // 0x2D BLT: Branch if Less Than (Signed) -> N != V
    pub(crate) fn op_blt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, n != v);
    }

    // 0x2E BGT: Branch if Greater Than (Signed) -> Z=0 and N=V
    pub(crate) fn op_bgt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let z = (self.cc & (CcFlag::Z as u8)) != 0;
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, !z && (n == v));
    }

    // 0x2F BLE: Branch if Less or Equal (Signed) -> Z=1 or N!=V
    pub(crate) fn op_ble<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        let z = (self.cc & (CcFlag::Z as u8)) != 0;
        let n = (self.cc & (CcFlag::N as u8)) != 0;
        let v = (self.cc & (CcFlag::V as u8)) != 0;
        self.branch_short(opcode, cycle, bus, master, z || (n != v));
    }
}
