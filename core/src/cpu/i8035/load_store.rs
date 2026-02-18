use super::{ExecState, I8035};
use crate::core::{Bus, BusMaster};

/// Bus I/O address conventions for MCS-48 ports.
const PORT_BUS: u16 = 0x100;
const PORT_P1: u16 = 0x101;
const PORT_P2: u16 = 0x102;
const PORT_P4: u16 = 0x104;

impl I8035 {
    // ===== 1-cycle register/memory moves =====

    /// MOV A,Rn: A <- R(n). 1 cycle.
    pub(crate) fn op_mov_a_rn(&mut self, n: u8) {
        self.a = self.get_reg(n);
        self.state = ExecState::Fetch;
    }

    /// MOV Rn,A: R(n) <- A. 1 cycle.
    pub(crate) fn op_mov_rn_a(&mut self, n: u8) {
        self.set_reg(n, self.a);
        self.state = ExecState::Fetch;
    }

    /// MOV A,@Ri: A <- RAM[R(i)]. 1 cycle.
    pub(crate) fn op_mov_a_indirect(&mut self, ri: u8) {
        let addr = self.get_reg(ri);
        self.a = self.read_ram(addr);
        self.state = ExecState::Fetch;
    }

    /// MOV @Ri,A: RAM[R(i)] <- A. 1 cycle.
    pub(crate) fn op_mov_indirect_a(&mut self, ri: u8) {
        let addr = self.get_reg(ri);
        self.write_ram(addr, self.a);
        self.state = ExecState::Fetch;
    }

    /// XCH A,Rn: Exchange A with R(n). 1 cycle.
    pub(crate) fn op_xch_a_rn(&mut self, n: u8) {
        let val = self.get_reg(n);
        self.set_reg(n, self.a);
        self.a = val;
        self.state = ExecState::Fetch;
    }

    /// XCH A,@Ri: Exchange A with RAM[R(i)]. 1 cycle.
    pub(crate) fn op_xch_a_indirect(&mut self, ri: u8) {
        let addr = self.get_reg(ri);
        let val = self.read_ram(addr);
        self.write_ram(addr, self.a);
        self.a = val;
        self.state = ExecState::Fetch;
    }

    /// XCHD A,@Ri: Exchange low nibbles of A and RAM[R(i)]. 1 cycle.
    pub(crate) fn op_xchd_a_indirect(&mut self, ri: u8) {
        let addr = self.get_reg(ri);
        let val = self.read_ram(addr);
        let a_lo = self.a & 0x0F;
        let v_lo = val & 0x0F;
        self.a = (self.a & 0xF0) | v_lo;
        self.write_ram(addr, (val & 0xF0) | a_lo);
        self.state = ExecState::Fetch;
    }

    /// MOV A,T: A <- timer register. 1 cycle.
    pub(crate) fn op_mov_a_t(&mut self) {
        self.a = self.t;
        self.state = ExecState::Fetch;
    }

    /// MOV T,A: Timer register <- A. 1 cycle.
    pub(crate) fn op_mov_t_a(&mut self) {
        self.t = self.a;
        self.state = ExecState::Fetch;
    }

    /// MOV A,PSW: A <- program status word. 1 cycle.
    pub(crate) fn op_mov_a_psw(&mut self) {
        self.a = self.psw;
        self.state = ExecState::Fetch;
    }

    /// MOV PSW,A: Program status word <- A. 1 cycle.
    pub(crate) fn op_mov_psw_a(&mut self) {
        self.psw = self.a;
        self.state = ExecState::Fetch;
    }

    // ===== 2-cycle immediate loads =====

    /// MOV A,#data (0x23): A <- immediate. 2 cycles.
    pub(crate) fn op_mov_a_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.a = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// MOV Rn,#data (0xB8-0xBF): R(n) <- immediate. 2 cycles.
    pub(crate) fn op_mov_rn_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        n: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.set_reg(n, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// MOV @Ri,#data (0xB0-0xB1): RAM[R(i)] <- immediate. 2 cycles.
    pub(crate) fn op_mov_indirect_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        ri: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                let addr = self.get_reg(ri);
                self.write_ram(addr, data);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ===== 2-cycle external memory access =====

    /// MOVX A,@Ri (0x80-0x81): A <- external RAM[R(i)]. 2 cycles.
    pub(crate) fn op_movx_a_indirect<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        ri: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = self.get_reg(ri) as u16;
                self.a = bus.io_read(master, addr);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// MOVX @Ri,A (0x90-0x91): External RAM[R(i)] <- A. 2 cycles.
    pub(crate) fn op_movx_indirect_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        ri: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = self.get_reg(ri) as u16;
                bus.io_write(master, addr, self.a);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ===== 2-cycle program memory reads =====

    /// MOVP A,@A (0xA3): A <- program_memory[(PC & 0xF00) | A]. 2 cycles.
    pub(crate) fn op_movp_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = (self.pc & 0xF00) | self.a as u16;
                self.a = bus.read(master, addr);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// MOVP3 A,@A (0xE3): A <- program_memory[0x300 | A]. 2 cycles.
    pub(crate) fn op_movp3_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = 0x300 | self.a as u16;
                self.a = bus.read(master, addr);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ===== 2-cycle port I/O =====

    /// INS A,BUS (0x08): A <- BUS port. 2 cycles.
    pub(crate) fn op_ins_a_bus<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.a = bus.io_read(master, PORT_BUS);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// IN A,P1 (0x09): A <- port 1 pins. 2 cycles.
    pub(crate) fn op_in_a_p1<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.a = bus.io_read(master, PORT_P1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// IN A,P2 (0x0A): A <- port 2 pins. 2 cycles.
    pub(crate) fn op_in_a_p2<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.a = bus.io_read(master, PORT_P2);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// OUTL BUS,A (0x02): BUS port <- A. 2 cycles.
    pub(crate) fn op_outl_bus_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.dbbb = self.a;
                bus.io_write(master, PORT_BUS, self.a);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// OUTL P1,A (0x39): Port 1 latch <- A. 2 cycles.
    pub(crate) fn op_outl_p1_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.p1 = self.a;
                bus.io_write(master, PORT_P1, self.a);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// OUTL P2,A (0x3A): Port 2 latch <- A. 2 cycles.
    pub(crate) fn op_outl_p2_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.p2 = self.a;
                bus.io_write(master, PORT_P2, self.a);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ===== 2-cycle port read-modify-write =====

    /// ANL BUS,#data (0x98): BUS latch <- BUS latch & immediate. 2 cycles.
    pub(crate) fn op_anl_bus_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.dbbb &= data;
                bus.io_write(master, PORT_BUS, self.dbbb);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ORL BUS,#data (0x88): BUS latch <- BUS latch | immediate. 2 cycles.
    pub(crate) fn op_orl_bus_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.dbbb |= data;
                bus.io_write(master, PORT_BUS, self.dbbb);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ANL P1,#data (0x99): P1 latch <- P1 latch & immediate. 2 cycles.
    pub(crate) fn op_anl_p1_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.p1 &= data;
                bus.io_write(master, PORT_P1, self.p1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ORL P1,#data (0x89): P1 latch <- P1 latch | immediate. 2 cycles.
    pub(crate) fn op_orl_p1_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.p1 |= data;
                bus.io_write(master, PORT_P1, self.p1);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ANL P2,#data (0x9A): P2 latch <- P2 latch & immediate. 2 cycles.
    pub(crate) fn op_anl_p2_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.p2 &= data;
                bus.io_write(master, PORT_P2, self.p2);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ORL P2,#data (0x8A): P2 latch <- P2 latch | immediate. 2 cycles.
    pub(crate) fn op_orl_p2_imm<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let data = bus.read(master, self.pc);
                self.pc = (self.pc + 1) & 0x0FFF;
                self.p2 |= data;
                bus.io_write(master, PORT_P2, self.p2);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    // ===== 2-cycle 4-bit expander port I/O (P4-P7 via P2) =====

    /// MOVD A,Pp (0x0C-0x0F): A <- expander port (low nibble). 2 cycles.
    pub(crate) fn op_movd_a_pp<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        port: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                self.a = bus.io_read(master, PORT_P4 + port as u16) & 0x0F;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// MOVD Pp,A (0x3C-0x3F): Expander port <- A (low nibble). 2 cycles.
    pub(crate) fn op_movd_pp_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        port: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                bus.io_write(master, PORT_P4 + port as u16, self.a & 0x0F);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ANLD Pp,A (0x9C-0x9F): Expander port <- port & A (low nibble). 2 cycles.
    pub(crate) fn op_anld_pp_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        port: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = PORT_P4 + port as u16;
                let val = bus.io_read(master, addr) & (self.a | 0xF0);
                bus.io_write(master, addr, val & 0x0F);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }

    /// ORLD Pp,A (0x8C-0x8F): Expander port <- port | A (low nibble). 2 cycles.
    pub(crate) fn op_orld_pp_a<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        port: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => self.state = ExecState::Execute(self.opcode),
            1 => {
                let addr = PORT_P4 + port as u16;
                let val = bus.io_read(master, addr) | (self.a & 0x0F);
                bus.io_write(master, addr, val & 0x0F);
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
    }
}
