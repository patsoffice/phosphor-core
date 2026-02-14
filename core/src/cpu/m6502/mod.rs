mod alu;
mod binary;
mod load_store;

use crate::core::{
    Bus, BusMaster,
    bus::InterruptState,
    component::{BusMasterComponent, Component},
};
use crate::cpu::{
    Cpu,
    state::{CpuStateTrait, M6502State},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum StatusFlag {
    C = 0x01, // Carry
    Z = 0x02, // Zero
    I = 0x04, // Interrupt Disable
    D = 0x08, // Decimal
    B = 0x10, // Break
    U = 0x20, // Unused (always 1)
    V = 0x40, // Overflow
    N = 0x80, // Negative
}

pub struct M6502 {
    // Registers
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub p: u8,

    // Internal state
    pub(crate) state: ExecState,
    pub(crate) opcode: u8,
    pub(crate) temp_addr: u16,
    /// Temporary data storage for multi-cycle operations (RMW operand, address bytes)
    pub(crate) temp_data: u8,
    /// Interrupt type being processed: 0=none, 1=NMI, 2=IRQ, 3=BRK
    pub(crate) interrupt_type: u8,
    /// Previous NMI line state for edge detection
    pub(crate) nmi_previous: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ExecState {
    Fetch,
    Execute(u8, u8), // (opcode, cycle)
    /// Hardware interrupt response sequence (NMI/IRQ push + vector)
    Interrupt(u8),
}

impl Default for M6502 {
    fn default() -> Self {
        Self::new()
    }
}

impl M6502 {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            p: 0x24, // I=1, U=1
            state: ExecState::Fetch,
            opcode: 0,
            temp_addr: 0,
            temp_data: 0,
            interrupt_type: 0,
            nmi_previous: false,
        }
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: StatusFlag, set: bool) {
        if set {
            self.p |= flag as u8;
        } else {
            self.p &= !(flag as u8);
        }
    }

    pub fn execute_cycle<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        bus: &mut B,
        master: BusMaster,
    ) {
        match self.state {
            ExecState::Fetch => {
                self.opcode = bus.read(master, self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.state = ExecState::Execute(self.opcode, 0);
            }
            ExecState::Execute(op, cyc) => {
                self.execute_instruction(op, cyc, bus, master);
            }
            ExecState::Interrupt(cycle) => {
                self.execute_interrupt(cycle, bus, master);
            }
        }
    }

    fn execute_instruction<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        opcode: u8,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match opcode {
            // --- LDA ---
            0xA9 => self.op_lda_imm(cycle, bus, master),
            0xA5 => self.op_lda_zp(cycle, bus, master),
            0xB5 => self.op_lda_zp_x(cycle, bus, master),
            0xAD => self.op_lda_abs(cycle, bus, master),
            0xBD => self.op_lda_abs_x(cycle, bus, master),
            0xB9 => self.op_lda_abs_y(cycle, bus, master),
            0xA1 => self.op_lda_ind_x(cycle, bus, master),
            0xB1 => self.op_lda_ind_y(cycle, bus, master),

            // --- LDX ---
            0xA2 => self.op_ldx_imm(cycle, bus, master),
            0xA6 => self.op_ldx_zp(cycle, bus, master),
            0xB6 => self.op_ldx_zp_y(cycle, bus, master),
            0xAE => self.op_ldx_abs(cycle, bus, master),
            0xBE => self.op_ldx_abs_y(cycle, bus, master),

            // --- LDY ---
            0xA0 => self.op_ldy_imm(cycle, bus, master),
            0xA4 => self.op_ldy_zp(cycle, bus, master),
            0xB4 => self.op_ldy_zp_x(cycle, bus, master),
            0xAC => self.op_ldy_abs(cycle, bus, master),
            0xBC => self.op_ldy_abs_x(cycle, bus, master),

            // --- STA ---
            0x85 => self.op_sta_zp(cycle, bus, master),
            0x95 => self.op_sta_zp_x(cycle, bus, master),
            0x8D => self.op_sta_abs(cycle, bus, master),
            0x9D => self.op_sta_abs_x(cycle, bus, master),
            0x99 => self.op_sta_abs_y(cycle, bus, master),
            0x81 => self.op_sta_ind_x(cycle, bus, master),
            0x91 => self.op_sta_ind_y(cycle, bus, master),

            // --- STX ---
            0x86 => self.op_stx_zp(cycle, bus, master),
            0x96 => self.op_stx_zp_y(cycle, bus, master),
            0x8E => self.op_stx_abs(cycle, bus, master),

            // --- STY ---
            0x84 => self.op_sty_zp(cycle, bus, master),
            0x94 => self.op_sty_zp_x(cycle, bus, master),
            0x8C => self.op_sty_abs(cycle, bus, master),

            // --- ADC ---
            0x69 => self.op_adc_imm(cycle, bus, master),
            0x65 => self.op_adc_zp(cycle, bus, master),
            0x75 => self.op_adc_zp_x(cycle, bus, master),
            0x6D => self.op_adc_abs(cycle, bus, master),
            0x7D => self.op_adc_abs_x(cycle, bus, master),
            0x79 => self.op_adc_abs_y(cycle, bus, master),
            0x61 => self.op_adc_ind_x(cycle, bus, master),
            0x71 => self.op_adc_ind_y(cycle, bus, master),

            // --- SBC ---
            0xE9 => self.op_sbc_imm(cycle, bus, master),
            0xE5 => self.op_sbc_zp(cycle, bus, master),
            0xF5 => self.op_sbc_zp_x(cycle, bus, master),
            0xED => self.op_sbc_abs(cycle, bus, master),
            0xFD => self.op_sbc_abs_x(cycle, bus, master),
            0xF9 => self.op_sbc_abs_y(cycle, bus, master),
            0xE1 => self.op_sbc_ind_x(cycle, bus, master),
            0xF1 => self.op_sbc_ind_y(cycle, bus, master),

            // --- CMP ---
            0xC9 => self.op_cmp_imm(cycle, bus, master),
            0xC5 => self.op_cmp_zp(cycle, bus, master),
            0xD5 => self.op_cmp_zp_x(cycle, bus, master),
            0xCD => self.op_cmp_abs(cycle, bus, master),
            0xDD => self.op_cmp_abs_x(cycle, bus, master),
            0xD9 => self.op_cmp_abs_y(cycle, bus, master),
            0xC1 => self.op_cmp_ind_x(cycle, bus, master),
            0xD1 => self.op_cmp_ind_y(cycle, bus, master),

            // --- AND ---
            0x29 => self.op_and_imm(cycle, bus, master),
            0x25 => self.op_and_zp(cycle, bus, master),
            0x35 => self.op_and_zp_x(cycle, bus, master),
            0x2D => self.op_and_abs(cycle, bus, master),
            0x3D => self.op_and_abs_x(cycle, bus, master),
            0x39 => self.op_and_abs_y(cycle, bus, master),
            0x21 => self.op_and_ind_x(cycle, bus, master),
            0x31 => self.op_and_ind_y(cycle, bus, master),

            // --- ORA ---
            0x09 => self.op_ora_imm(cycle, bus, master),
            0x05 => self.op_ora_zp(cycle, bus, master),
            0x15 => self.op_ora_zp_x(cycle, bus, master),
            0x0D => self.op_ora_abs(cycle, bus, master),
            0x1D => self.op_ora_abs_x(cycle, bus, master),
            0x19 => self.op_ora_abs_y(cycle, bus, master),
            0x01 => self.op_ora_ind_x(cycle, bus, master),
            0x11 => self.op_ora_ind_y(cycle, bus, master),

            // --- EOR ---
            0x49 => self.op_eor_imm(cycle, bus, master),
            0x45 => self.op_eor_zp(cycle, bus, master),
            0x55 => self.op_eor_zp_x(cycle, bus, master),
            0x4D => self.op_eor_abs(cycle, bus, master),
            0x5D => self.op_eor_abs_x(cycle, bus, master),
            0x59 => self.op_eor_abs_y(cycle, bus, master),
            0x41 => self.op_eor_ind_x(cycle, bus, master),
            0x51 => self.op_eor_ind_y(cycle, bus, master),

            // --- BIT ---
            0x24 => self.op_bit_zp(cycle, bus, master),
            0x2C => self.op_bit_abs(cycle, bus, master),

            // --- CPX ---
            0xE0 => self.op_cpx_imm(cycle, bus, master),
            0xE4 => self.op_cpx_zp(cycle, bus, master),
            0xEC => self.op_cpx_abs(cycle, bus, master),

            // --- CPY ---
            0xC0 => self.op_cpy_imm(cycle, bus, master),
            0xC4 => self.op_cpy_zp(cycle, bus, master),
            0xCC => self.op_cpy_abs(cycle, bus, master),

            // --- Flag instructions (all 2-cycle implied) ---
            0x18 => {
                // CLC - Clear Carry
                if cycle == 0 {
                    self.set_flag(StatusFlag::C, false);
                    self.state = ExecState::Fetch;
                }
            }
            0x38 => {
                // SEC - Set Carry
                if cycle == 0 {
                    self.set_flag(StatusFlag::C, true);
                    self.state = ExecState::Fetch;
                }
            }
            0x58 => {
                // CLI - Clear Interrupt Disable
                if cycle == 0 {
                    self.set_flag(StatusFlag::I, false);
                    self.state = ExecState::Fetch;
                }
            }
            0x78 => {
                // SEI - Set Interrupt Disable
                if cycle == 0 {
                    self.set_flag(StatusFlag::I, true);
                    self.state = ExecState::Fetch;
                }
            }
            0xB8 => {
                // CLV - Clear Overflow
                if cycle == 0 {
                    self.set_flag(StatusFlag::V, false);
                    self.state = ExecState::Fetch;
                }
            }
            0xD8 => {
                // CLD - Clear Decimal
                if cycle == 0 {
                    self.set_flag(StatusFlag::D, false);
                    self.state = ExecState::Fetch;
                }
            }
            0xF8 => {
                // SED - Set Decimal
                if cycle == 0 {
                    self.set_flag(StatusFlag::D, true);
                    self.state = ExecState::Fetch;
                }
            }

            // --- Transfer instructions (all 2-cycle implied) ---
            0xAA => {
                // TAX - Transfer A to X. Sets N, Z.
                if cycle == 0 {
                    self.x = self.a;
                    self.set_nz(self.x);
                    self.state = ExecState::Fetch;
                }
            }
            0xA8 => {
                // TAY - Transfer A to Y. Sets N, Z.
                if cycle == 0 {
                    self.y = self.a;
                    self.set_nz(self.y);
                    self.state = ExecState::Fetch;
                }
            }
            0x8A => {
                // TXA - Transfer X to A. Sets N, Z.
                if cycle == 0 {
                    self.a = self.x;
                    self.set_nz(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0x98 => {
                // TYA - Transfer Y to A. Sets N, Z.
                if cycle == 0 {
                    self.a = self.y;
                    self.set_nz(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0xBA => {
                // TSX - Transfer SP to X. Sets N, Z.
                if cycle == 0 {
                    self.x = self.sp;
                    self.set_nz(self.x);
                    self.state = ExecState::Fetch;
                }
            }
            0x9A => {
                // TXS - Transfer X to SP. Does NOT set flags.
                if cycle == 0 {
                    self.sp = self.x;
                    self.state = ExecState::Fetch;
                }
            }

            // --- Register increment/decrement (all 2-cycle implied) ---
            0xE8 => {
                // INX - Increment X. Sets N, Z.
                if cycle == 0 {
                    self.x = self.x.wrapping_add(1);
                    self.set_nz(self.x);
                    self.state = ExecState::Fetch;
                }
            }
            0xC8 => {
                // INY - Increment Y. Sets N, Z.
                if cycle == 0 {
                    self.y = self.y.wrapping_add(1);
                    self.set_nz(self.y);
                    self.state = ExecState::Fetch;
                }
            }
            0xCA => {
                // DEX - Decrement X. Sets N, Z.
                if cycle == 0 {
                    self.x = self.x.wrapping_sub(1);
                    self.set_nz(self.x);
                    self.state = ExecState::Fetch;
                }
            }
            0x88 => {
                // DEY - Decrement Y. Sets N, Z.
                if cycle == 0 {
                    self.y = self.y.wrapping_sub(1);
                    self.set_nz(self.y);
                    self.state = ExecState::Fetch;
                }
            }

            // --- NOP (2-cycle implied) ---
            0xEA => {
                if cycle == 0 {
                    self.state = ExecState::Fetch;
                }
            }

            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    /// Placeholder for interrupt execution (will be implemented in Phase 7)
    fn execute_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        _cycle: u8,
        _bus: &mut B,
        _master: BusMaster,
    ) {
        self.state = ExecState::Fetch;
    }
}

impl Component for M6502 {
    fn tick(&mut self) -> bool {
        false
    }
}

impl BusMasterComponent for M6502 {
    type Bus = dyn Bus<Address = u16, Data = u8>;

    fn tick_with_bus(&mut self, bus: &mut Self::Bus, master: BusMaster) -> bool {
        self.execute_cycle(bus, master);
        matches!(self.state, ExecState::Fetch)
    }
}

impl Cpu for M6502 {
    fn reset(&mut self) {
        self.pc = 0;
        self.sp = 0xFD;
        self.p = 0x24;
    }

    fn signal_interrupt(&mut self, _int: InterruptState) {}

    fn is_sleeping(&self) -> bool {
        false
    }
}

impl CpuStateTrait for M6502 {
    type Snapshot = M6502State;

    fn snapshot(&self) -> M6502State {
        M6502State {
            a: self.a,
            x: self.x,
            y: self.y,
            pc: self.pc,
            sp: self.sp,
            p: self.p,
        }
    }
}
