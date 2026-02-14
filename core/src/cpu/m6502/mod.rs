mod alu;
mod binary;
mod branch;
mod load_store;
mod shift;
mod stack;
mod unary;

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
                let ints = bus.check_interrupts(master);
                if self.handle_interrupts(ints) {
                    return;
                }
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

            // --- ASL ---
            0x0A => {
                // ASL Accumulator - 2 cycles
                if cycle == 0 {
                    self.a = self.perform_asl(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0x06 => self.op_asl_zp(cycle, bus, master),
            0x16 => self.op_asl_zp_x(cycle, bus, master),
            0x0E => self.op_asl_abs(cycle, bus, master),
            0x1E => self.op_asl_abs_x(cycle, bus, master),

            // --- LSR ---
            0x4A => {
                // LSR Accumulator - 2 cycles
                if cycle == 0 {
                    self.a = self.perform_lsr(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0x46 => self.op_lsr_zp(cycle, bus, master),
            0x56 => self.op_lsr_zp_x(cycle, bus, master),
            0x4E => self.op_lsr_abs(cycle, bus, master),
            0x5E => self.op_lsr_abs_x(cycle, bus, master),

            // --- ROL ---
            0x2A => {
                // ROL Accumulator - 2 cycles
                if cycle == 0 {
                    self.a = self.perform_rol(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0x26 => self.op_rol_zp(cycle, bus, master),
            0x36 => self.op_rol_zp_x(cycle, bus, master),
            0x2E => self.op_rol_abs(cycle, bus, master),
            0x3E => self.op_rol_abs_x(cycle, bus, master),

            // --- ROR ---
            0x6A => {
                // ROR Accumulator - 2 cycles
                if cycle == 0 {
                    self.a = self.perform_ror(self.a);
                    self.state = ExecState::Fetch;
                }
            }
            0x66 => self.op_ror_zp(cycle, bus, master),
            0x76 => self.op_ror_zp_x(cycle, bus, master),
            0x6E => self.op_ror_abs(cycle, bus, master),
            0x7E => self.op_ror_abs_x(cycle, bus, master),

            // --- INC ---
            0xE6 => self.op_inc_zp(cycle, bus, master),
            0xF6 => self.op_inc_zp_x(cycle, bus, master),
            0xEE => self.op_inc_abs(cycle, bus, master),
            0xFE => self.op_inc_abs_x(cycle, bus, master),

            // --- DEC ---
            0xC6 => self.op_dec_zp(cycle, bus, master),
            0xD6 => self.op_dec_zp_x(cycle, bus, master),
            0xCE => self.op_dec_abs(cycle, bus, master),
            0xDE => self.op_dec_abs_x(cycle, bus, master),

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

            // --- Branches ---
            0x10 => self.op_bpl(cycle, bus, master),
            0x30 => self.op_bmi(cycle, bus, master),
            0x50 => self.op_bvc(cycle, bus, master),
            0x70 => self.op_bvs(cycle, bus, master),
            0x90 => self.op_bcc(cycle, bus, master),
            0xB0 => self.op_bcs(cycle, bus, master),
            0xD0 => self.op_bne(cycle, bus, master),
            0xF0 => self.op_beq(cycle, bus, master),

            // --- Jumps ---
            0x4C => self.op_jmp_abs(cycle, bus, master),
            0x6C => self.op_jmp_ind(cycle, bus, master),
            0x20 => self.op_jsr(cycle, bus, master),
            0x60 => self.op_rts(cycle, bus, master),
            0x40 => self.op_rti(cycle, bus, master),

            // --- Stack ---
            0x48 => self.op_pha(cycle, bus, master),
            0x68 => self.op_pla(cycle, bus, master),
            0x08 => self.op_php(cycle, bus, master),
            0x28 => self.op_plp(cycle, bus, master),

            // --- BRK ---
            0x00 => self.op_brk(cycle, bus, master),

            // Unknown opcode - just fetch next
            _ => {
                self.state = ExecState::Fetch;
            }
        }
    }

    /// Check for pending interrupts during Fetch state. Returns true if an
    /// interrupt was taken (state transitions to Interrupt sequence).
    fn handle_interrupts(&mut self, ints: InterruptState) -> bool {
        // NMI is edge-triggered: detect rising edge
        let nmi_edge = ints.nmi && !self.nmi_previous;
        self.nmi_previous = ints.nmi;

        if nmi_edge {
            self.interrupt_type = 1; // NMI
            self.state = ExecState::Interrupt(0);
            return true;
        }

        // IRQ is level-triggered, masked by I flag
        if ints.irq && (self.p & StatusFlag::I as u8) == 0 {
            self.interrupt_type = 2; // IRQ
            self.state = ExecState::Interrupt(0);
            return true;
        }

        false
    }

    /// Execute hardware interrupt sequence (NMI/IRQ).
    /// 7 cycles total: 1 (detection in Fetch) + 6 (this handler, cycles 0-5).
    /// Pushes PC and P (with B=0), then reads vector and sets I flag.
    fn execute_interrupt<B: Bus<Address = u16, Data = u8> + ?Sized>(
        &mut self,
        cycle: u8,
        bus: &mut B,
        master: BusMaster,
    ) {
        match cycle {
            0 => {
                // Internal cycle (replaces phantom opcode read)
                self.state = ExecState::Interrupt(1);
            }
            1 => {
                // Push PCH
                bus.write(master, 0x0100 | self.sp as u16, (self.pc >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(2);
            }
            2 => {
                // Push PCL
                bus.write(master, 0x0100 | self.sp as u16, self.pc as u8);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(3);
            }
            3 => {
                // Push P with B=0, U=1 (hardware interrupt, not BRK)
                let p_push = (self.p | StatusFlag::U as u8) & !(StatusFlag::B as u8);
                bus.write(master, 0x0100 | self.sp as u16, p_push);
                self.sp = self.sp.wrapping_sub(1);
                self.state = ExecState::Interrupt(4);
            }
            4 => {
                // Set I flag, read vector low byte
                self.set_flag(StatusFlag::I, true);
                let vector_addr = match self.interrupt_type {
                    1 => 0xFFFA, // NMI
                    _ => 0xFFFE, // IRQ
                };
                self.pc = bus.read(master, vector_addr) as u16;
                self.state = ExecState::Interrupt(5);
            }
            5 => {
                // Read vector high byte
                let vector_addr = match self.interrupt_type {
                    1 => 0xFFFB, // NMI
                    _ => 0xFFFF, // IRQ
                };
                self.pc |= (bus.read(master, vector_addr) as u16) << 8;
                self.interrupt_type = 0;
                self.state = ExecState::Fetch;
            }
            _ => self.state = ExecState::Fetch,
        }
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
