//! Comprehensive tests for the M6809 instruction disassembler.

use phosphor_core::cpu::disasm::Disassemble;
use phosphor_core::cpu::m6809::M6809;

/// Helper: disassemble at address 0x1000 with padding.
fn dis(bytes: &[u8]) -> phosphor_core::cpu::disasm::DisassembledInstruction {
    let mut buf = [0u8; 6];
    let n = bytes.len().min(6);
    buf[..n].copy_from_slice(&bytes[..n]);
    M6809::disassemble(0x1000, &buf)
}

// ── Inherent instructions ────────────────────────────────────────────────────

#[test]
fn test_nop() {
    let r = dis(&[0x12]);
    assert_eq!(r.mnemonic, "NOP");
    assert_eq!(r.byte_len, 1);
    assert!(r.operands.is_empty());
    assert_eq!(r.target_addr, None);
}

#[test]
fn test_inherent_misc() {
    for (op, mne) in [
        (0x13, "SYNC"),
        (0x19, "DAA"),
        (0x1D, "SEX"),
        (0x39, "RTS"),
        (0x3A, "ABX"),
        (0x3B, "RTI"),
        (0x3D, "MUL"),
        (0x3F, "SWI"),
    ] {
        let r = dis(&[op]);
        assert_eq!(r.mnemonic, mne, "opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 1);
    }
}

#[test]
fn test_inherent_a_register() {
    for (op, mne) in [
        (0x40, "NEGA"),
        (0x43, "COMA"),
        (0x44, "LSRA"),
        (0x46, "RORA"),
        (0x47, "ASRA"),
        (0x48, "ASLA"),
        (0x49, "ROLA"),
        (0x4A, "DECA"),
        (0x4C, "INCA"),
        (0x4D, "TSTA"),
        (0x4F, "CLRA"),
    ] {
        let r = dis(&[op]);
        assert_eq!(r.mnemonic, mne, "opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 1);
    }
}

#[test]
fn test_inherent_b_register() {
    for (op, mne) in [
        (0x50, "NEGB"),
        (0x53, "COMB"),
        (0x54, "LSRB"),
        (0x56, "RORB"),
        (0x57, "ASRB"),
        (0x58, "ASLB"),
        (0x59, "ROLB"),
        (0x5A, "DECB"),
        (0x5C, "INCB"),
        (0x5D, "TSTB"),
        (0x5F, "CLRB"),
    ] {
        let r = dis(&[op]);
        assert_eq!(r.mnemonic, mne, "opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 1);
    }
}

// ── Immediate byte ───────────────────────────────────────────────────────────

#[test]
fn test_imm8_alu() {
    let r = dis(&[0x80, 0x42]);
    assert_eq!(r.mnemonic, "SUBA");
    assert_eq!(r.operands, "#$42");
    assert_eq!(r.byte_len, 2);
    assert_eq!(r.target_addr, None);
}

#[test]
fn test_orcc() {
    let r = dis(&[0x1A, 0x50]);
    assert_eq!(r.mnemonic, "ORCC");
    assert_eq!(r.operands, "#$50");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_andcc() {
    let r = dis(&[0x1C, 0xAF]);
    assert_eq!(r.mnemonic, "ANDCC");
    assert_eq!(r.operands, "#$AF");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_cwai() {
    let r = dis(&[0x3C, 0xEF]);
    assert_eq!(r.mnemonic, "CWAI");
    assert_eq!(r.operands, "#$EF");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_lda_imm() {
    let r = dis(&[0x86, 0xFF]);
    assert_eq!(r.mnemonic, "LDA");
    assert_eq!(r.operands, "#$FF");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_ldb_imm() {
    let r = dis(&[0xC6, 0x00]);
    assert_eq!(r.mnemonic, "LDB");
    assert_eq!(r.operands, "#$00");
}

// ── Immediate word ───────────────────────────────────────────────────────────

#[test]
fn test_subd_imm() {
    let r = dis(&[0x83, 0x12, 0x34]);
    assert_eq!(r.mnemonic, "SUBD");
    assert_eq!(r.operands, "#$1234");
    assert_eq!(r.byte_len, 3);
    assert_eq!(r.target_addr, None);
}

#[test]
fn test_ldx_imm() {
    let r = dis(&[0x8E, 0xC0, 0x00]);
    assert_eq!(r.mnemonic, "LDX");
    assert_eq!(r.operands, "#$C000");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_ldu_imm() {
    let r = dis(&[0xCE, 0x40, 0x00]);
    assert_eq!(r.mnemonic, "LDU");
    assert_eq!(r.operands, "#$4000");
}

#[test]
fn test_ldd_imm() {
    let r = dis(&[0xCC, 0xAB, 0xCD]);
    assert_eq!(r.mnemonic, "LDD");
    assert_eq!(r.operands, "#$ABCD");
}

// ── Direct page ──────────────────────────────────────────────────────────────

#[test]
fn test_lda_dir() {
    let r = dis(&[0x96, 0x42]);
    assert_eq!(r.mnemonic, "LDA");
    assert_eq!(r.operands, "$42");
    assert_eq!(r.byte_len, 2);
    assert_eq!(r.target_addr, Some(0x0042));
}

#[test]
fn test_sta_dir() {
    let r = dis(&[0x97, 0x80]);
    assert_eq!(r.mnemonic, "STA");
    assert_eq!(r.operands, "$80");
    assert_eq!(r.target_addr, Some(0x0080));
}

#[test]
fn test_neg_dir() {
    let r = dis(&[0x00, 0x10]);
    assert_eq!(r.mnemonic, "NEG");
    assert_eq!(r.operands, "$10");
    assert_eq!(r.byte_len, 2);
    assert_eq!(r.target_addr, Some(0x0010));
}

#[test]
fn test_jmp_dir() {
    let r = dis(&[0x0E, 0x50]);
    assert_eq!(r.mnemonic, "JMP");
    assert_eq!(r.operands, "$50");
    assert_eq!(r.target_addr, Some(0x0050));
}

#[test]
fn test_jsr_dir() {
    let r = dis(&[0x9D, 0x30]);
    assert_eq!(r.mnemonic, "JSR");
    assert_eq!(r.operands, "$30");
    assert_eq!(r.target_addr, Some(0x0030));
}

// ── Extended ─────────────────────────────────────────────────────────────────

#[test]
fn test_lda_ext() {
    let r = dis(&[0xB6, 0xC0, 0x00]);
    assert_eq!(r.mnemonic, "LDA");
    assert_eq!(r.operands, "$C000");
    assert_eq!(r.byte_len, 3);
    assert_eq!(r.target_addr, Some(0xC000));
}

#[test]
fn test_jmp_ext() {
    let r = dis(&[0x7E, 0xFF, 0xFE]);
    assert_eq!(r.mnemonic, "JMP");
    assert_eq!(r.operands, "$FFFE");
    assert_eq!(r.target_addr, Some(0xFFFE));
}

#[test]
fn test_jsr_ext() {
    let r = dis(&[0xBD, 0x12, 0x34]);
    assert_eq!(r.mnemonic, "JSR");
    assert_eq!(r.operands, "$1234");
    assert_eq!(r.target_addr, Some(0x1234));
}

// ── Indexed: 5-bit constant offset ──────────────────────────────────────────

#[test]
fn test_idx_5bit_zero() {
    // LDA 0,X  (postbyte 0b00000000 = 0x00: reg=X, offset=0)
    let r = dis(&[0xA6, 0x00]);
    assert_eq!(r.mnemonic, "LDA");
    assert_eq!(r.operands, "0,X");
    assert_eq!(r.byte_len, 2);
    assert_eq!(r.target_addr, None);
}

#[test]
fn test_idx_5bit_positive() {
    // LDA 15,Y  (postbyte 0b00101111 = 0x2F: reg=Y, offset=15)
    let r = dis(&[0xA6, 0x2F]);
    assert_eq!(r.operands, "15,Y");
}

#[test]
fn test_idx_5bit_negative() {
    // LDA -16,U  (postbyte 0b01010000 = 0x50: reg=U, offset=-16)
    let r = dis(&[0xA6, 0x50]);
    assert_eq!(r.operands, "-16,U");
}

#[test]
fn test_idx_5bit_minus1() {
    // LDA -1,S  (postbyte 0b01111111 = 0x7F: reg=S, offset=-1)
    let r = dis(&[0xA6, 0x7F]);
    assert_eq!(r.operands, "-1,S");
}

#[test]
fn test_idx_5bit_all_regs() {
    // Test all 4 register selections with offset 5
    for (reg_bits, name) in [(0x00, "X"), (0x20, "Y"), (0x40, "U"), (0x60, "S")] {
        let pb = reg_bits | 0x05; // 5-bit offset = 5
        let r = dis(&[0xA6, pb]);
        assert_eq!(r.operands, format!("5,{}", name), "pb=0x{:02X}", pb);
    }
}

// ── Indexed: no offset, post-inc, pre-dec ───────────────────────────────────

#[test]
fn test_idx_no_offset() {
    // LDA ,X  (postbyte 0x84)
    let r = dis(&[0xA6, 0x84]);
    assert_eq!(r.operands, ",X");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_idx_post_inc_1() {
    // LDA ,X+  (postbyte 0x80)
    let r = dis(&[0xA6, 0x80]);
    assert_eq!(r.operands, ",X+");
}

#[test]
fn test_idx_post_inc_2() {
    // LDA ,Y++  (postbyte 0xA1)
    let r = dis(&[0xA6, 0xA1]);
    assert_eq!(r.operands, ",Y++");
}

#[test]
fn test_idx_pre_dec_1() {
    // LDA ,-U  (postbyte 0xC2)
    let r = dis(&[0xA6, 0xC2]);
    assert_eq!(r.operands, ",-U");
}

#[test]
fn test_idx_pre_dec_2() {
    // LDA ,--S  (postbyte 0xE3)
    let r = dis(&[0xA6, 0xE3]);
    assert_eq!(r.operands, ",--S");
}

// ── Indexed: 8-bit and 16-bit offset ────────────────────────────────────────

#[test]
fn test_idx_8bit_offset() {
    // LDA $FE,X  (postbyte 0x88, offset 0xFE)
    let r = dis(&[0xA6, 0x88, 0xFE]);
    assert_eq!(r.operands, "$FE,X");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_idx_16bit_offset() {
    // LDA $1234,Y  (postbyte 0xA9, offset 0x1234)
    let r = dis(&[0xA6, 0xA9, 0x12, 0x34]);
    assert_eq!(r.operands, "$1234,Y");
    assert_eq!(r.byte_len, 4);
}

// ── Indexed: accumulator offsets ─────────────────────────────────────────────

#[test]
fn test_idx_acc_a() {
    // LDA A,X  (postbyte 0x86)
    let r = dis(&[0xA6, 0x86]);
    assert_eq!(r.operands, "A,X");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_idx_acc_b() {
    // LDA B,Y  (postbyte 0xA5)
    let r = dis(&[0xA6, 0xA5]);
    assert_eq!(r.operands, "B,Y");
}

#[test]
fn test_idx_acc_d() {
    // LDA D,U  (postbyte 0xCB)
    let r = dis(&[0xA6, 0xCB]);
    assert_eq!(r.operands, "D,U");
}

// ── Indexed: PC-relative ─────────────────────────────────────────────────────

#[test]
fn test_idx_pcr_8bit() {
    // LDA $FE,PCR  (postbyte 0x8C, offset 0xFE)
    let r = dis(&[0xA6, 0x8C, 0xFE]);
    assert_eq!(r.operands, "$FE,PCR");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_idx_pcr_16bit() {
    // LDA $1000,PCR  (postbyte 0x8D, offset 0x1000)
    let r = dis(&[0xA6, 0x8D, 0x10, 0x00]);
    assert_eq!(r.operands, "$1000,PCR");
    assert_eq!(r.byte_len, 4);
}

// ── Indexed: extended indirect ───────────────────────────────────────────────

#[test]
fn test_idx_extended_indirect() {
    // LDA [$C000]  (postbyte 0x9F, addr 0xC000)
    let r = dis(&[0xA6, 0x9F, 0xC0, 0x00]);
    assert_eq!(r.operands, "[$C000]");
    assert_eq!(r.byte_len, 4);
}

// ── Indexed: indirect variants ───────────────────────────────────────────────

#[test]
fn test_idx_indirect_no_offset() {
    // LDA [,X]  (postbyte 0x94)
    let r = dis(&[0xA6, 0x94]);
    assert_eq!(r.operands, "[,X]");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_idx_indirect_post_inc_2() {
    // LDA [,Y++]  (postbyte 0xB1)
    let r = dis(&[0xA6, 0xB1]);
    assert_eq!(r.operands, "[,Y++]");
}

#[test]
fn test_idx_indirect_pre_dec_2() {
    // LDA [,--U]  (postbyte 0xD3)
    let r = dis(&[0xA6, 0xD3]);
    assert_eq!(r.operands, "[,--U]");
}

#[test]
fn test_idx_indirect_8bit() {
    // LDA [$10,S]  (postbyte 0xF8, offset 0x10)
    let r = dis(&[0xA6, 0xF8, 0x10]);
    assert_eq!(r.operands, "[$10,S]");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_idx_indirect_16bit() {
    // LDA [$ABCD,X]  (postbyte 0x99, offset 0xABCD)
    let r = dis(&[0xA6, 0x99, 0xAB, 0xCD]);
    assert_eq!(r.operands, "[$ABCD,X]");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_idx_indirect_acc_a() {
    // LDA [A,X]  (postbyte 0x96)
    let r = dis(&[0xA6, 0x96]);
    assert_eq!(r.operands, "[A,X]");
}

#[test]
fn test_idx_indirect_acc_b() {
    // LDA [B,Y]  (postbyte 0xB5)
    let r = dis(&[0xA6, 0xB5]);
    assert_eq!(r.operands, "[B,Y]");
}

#[test]
fn test_idx_indirect_acc_d() {
    // LDA [D,S]  (postbyte 0xFB)
    let r = dis(&[0xA6, 0xFB]);
    assert_eq!(r.operands, "[D,S]");
}

#[test]
fn test_idx_indirect_pcr_8bit() {
    // LDA [$20,PCR]  (postbyte 0x9C, offset 0x20)
    let r = dis(&[0xA6, 0x9C, 0x20]);
    assert_eq!(r.operands, "[$20,PCR]");
}

#[test]
fn test_idx_indirect_pcr_16bit() {
    // LDA [$0100,PCR]  (postbyte 0x9D, offset 0x0100)
    let r = dis(&[0xA6, 0x9D, 0x01, 0x00]);
    assert_eq!(r.operands, "[$0100,PCR]");
}

// ── Indexed: illegal modes ───────────────────────────────────────────────────

#[test]
fn test_idx_illegal_mode() {
    // Postbyte 0x87 = non-indirect mode 0x07 (illegal)
    let r = dis(&[0xA6, 0x87]);
    assert_eq!(r.operands, "???");
}

#[test]
fn test_idx_illegal_indirect_post_inc_1() {
    // Postbyte 0x90 = indirect ,X+ (illegal: no indirect for +1)
    let r = dis(&[0xA6, 0x90]);
    assert_eq!(r.operands, "???");
}

#[test]
fn test_idx_illegal_indirect_pre_dec_1() {
    // Postbyte 0x92 = indirect ,-X (illegal: no indirect for -1)
    let r = dis(&[0xA6, 0x92]);
    assert_eq!(r.operands, "???");
}

// ── Short branches ───────────────────────────────────────────────────────────

#[test]
fn test_branch_forward() {
    // BEQ +5 at addr 0x1000 → target = 0x1000 + 2 + 5 = 0x1007
    let r = M6809::disassemble(0x1000, &[0x27, 0x05, 0, 0, 0, 0]);
    assert_eq!(r.mnemonic, "BEQ");
    assert_eq!(r.operands, "$1007");
    assert_eq!(r.byte_len, 2);
    assert_eq!(r.target_addr, Some(0x1007));
}

#[test]
fn test_branch_backward() {
    // BNE -10 (0xF6) at addr 0x1000 → target = 0x1000 + 2 - 10 = 0x0FF8
    let r = M6809::disassemble(0x1000, &[0x26, 0xF6, 0, 0, 0, 0]);
    assert_eq!(r.mnemonic, "BNE");
    assert_eq!(r.operands, "$0FF8");
    assert_eq!(r.target_addr, Some(0x0FF8));
}

#[test]
fn test_branch_self() {
    // BRA -2 (0xFE) at addr 0x1000 → target = 0x1000 + 2 - 2 = 0x1000
    let r = M6809::disassemble(0x1000, &[0x20, 0xFE, 0, 0, 0, 0]);
    assert_eq!(r.mnemonic, "BRA");
    assert_eq!(r.target_addr, Some(0x1000));
}

#[test]
fn test_branch_wrap_forward() {
    // BRA +5 at addr 0xFFFE → target = 0xFFFE + 2 + 5 = 0x0005
    let r = M6809::disassemble(0xFFFE, &[0x20, 0x05, 0, 0, 0, 0]);
    assert_eq!(r.target_addr, Some(0x0005));
}

#[test]
fn test_all_branch_mnemonics() {
    let branches = [
        (0x20, "BRA"),
        (0x21, "BRN"),
        (0x22, "BHI"),
        (0x23, "BLS"),
        (0x24, "BHS"),
        (0x25, "BLO"),
        (0x26, "BNE"),
        (0x27, "BEQ"),
        (0x28, "BVC"),
        (0x29, "BVS"),
        (0x2A, "BPL"),
        (0x2B, "BMI"),
        (0x2C, "BGE"),
        (0x2D, "BLT"),
        (0x2E, "BGT"),
        (0x2F, "BLE"),
    ];
    for (op, mne) in branches {
        let r = dis(&[op, 0x00]);
        assert_eq!(r.mnemonic, mne, "opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 2);
    }
}

#[test]
fn test_bsr() {
    // BSR at 0x1000 with offset 0x10 → target = 0x1000 + 2 + 0x10 = 0x1012
    let r = M6809::disassemble(0x1000, &[0x8D, 0x10, 0, 0, 0, 0]);
    assert_eq!(r.mnemonic, "BSR");
    assert_eq!(r.operands, "$1012");
    assert_eq!(r.target_addr, Some(0x1012));
}

// ── Long branches (page 1: LBRA, LBSR) ──────────────────────────────────────

#[test]
fn test_lbra() {
    // LBRA at 0x1000 with offset 0x0100 → target = 0x1000 + 3 + 0x0100 = 0x1103
    let r = M6809::disassemble(0x1000, &[0x16, 0x01, 0x00, 0, 0, 0]);
    assert_eq!(r.mnemonic, "LBRA");
    assert_eq!(r.operands, "$1103");
    assert_eq!(r.byte_len, 3);
    assert_eq!(r.target_addr, Some(0x1103));
}

#[test]
fn test_lbsr() {
    // LBSR at 0x1000 with offset 0xFF00 (-256) → target = 0x1000 + 3 - 256 = 0x0F03
    let r = M6809::disassemble(0x1000, &[0x17, 0xFF, 0x00, 0, 0, 0]);
    assert_eq!(r.mnemonic, "LBSR");
    assert_eq!(r.operands, "$0F03");
    assert_eq!(r.target_addr, Some(0x0F03));
}

// ── Long branches (page 2: 0x10 prefix) ─────────────────────────────────────

#[test]
fn test_lbeq() {
    // LBEQ (0x10 0x27) at 0x1000, offset 0x0200 → target = 0x1000 + 4 + 0x200 = 0x1204
    let r = M6809::disassemble(0x1000, &[0x10, 0x27, 0x02, 0x00, 0, 0]);
    assert_eq!(r.mnemonic, "LBEQ");
    assert_eq!(r.operands, "$1204");
    assert_eq!(r.byte_len, 4);
    assert_eq!(r.target_addr, Some(0x1204));
}

#[test]
fn test_lbne_backward() {
    // LBNE (0x10 0x26) at 0x1000, offset 0xFFF0 (-16) → target = 0x1000 + 4 - 16 = 0x0FF4
    let r = M6809::disassemble(0x1000, &[0x10, 0x26, 0xFF, 0xF0, 0, 0]);
    assert_eq!(r.mnemonic, "LBNE");
    assert_eq!(r.target_addr, Some(0x0FF4));
}

#[test]
fn test_all_long_branch_mnemonics() {
    let branches = [
        (0x21, "LBRN"),
        (0x22, "LBHI"),
        (0x23, "LBLS"),
        (0x24, "LBHS"),
        (0x25, "LBLO"),
        (0x26, "LBNE"),
        (0x27, "LBEQ"),
        (0x28, "LBVC"),
        (0x29, "LBVS"),
        (0x2A, "LBPL"),
        (0x2B, "LBMI"),
        (0x2C, "LBGE"),
        (0x2D, "LBLT"),
        (0x2E, "LBGT"),
        (0x2F, "LBLE"),
    ];
    for (op, mne) in branches {
        let r = dis(&[0x10, op, 0x00, 0x00]);
        assert_eq!(r.mnemonic, mne, "page 2 opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 4);
    }
}

// ── TFR / EXG ────────────────────────────────────────────────────────────────

#[test]
fn test_tfr_16bit() {
    // TFR D,X (postbyte 0x01)
    let r = dis(&[0x1F, 0x01]);
    assert_eq!(r.mnemonic, "TFR");
    assert_eq!(r.operands, "D,X");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_tfr_8bit() {
    // TFR A,B (postbyte 0x89)
    let r = dis(&[0x1F, 0x89]);
    assert_eq!(r.operands, "A,B");
}

#[test]
fn test_exg_16bit() {
    // EXG X,Y (postbyte 0x12)
    let r = dis(&[0x1E, 0x12]);
    assert_eq!(r.mnemonic, "EXG");
    assert_eq!(r.operands, "X,Y");
}

#[test]
fn test_tfr_pc() {
    // TFR PC,D (postbyte 0x50)
    let r = dis(&[0x1F, 0x50]);
    assert_eq!(r.operands, "PC,D");
}

#[test]
fn test_tfr_cc_dp() {
    // TFR CC,DP (postbyte 0xAB)
    let r = dis(&[0x1F, 0xAB]);
    assert_eq!(r.operands, "CC,DP");
}

#[test]
fn test_tfr_invalid_reg() {
    // TFR with invalid register ID 6 (postbyte 0x60)
    let r = dis(&[0x1F, 0x60]);
    assert_eq!(r.operands, "?,D");
}

// ── PSHS / PULS / PSHU / PULU ───────────────────────────────────────────────

#[test]
fn test_pshs_all() {
    // PSHS with all registers (0xFF) = PC,U,Y,X,DP,B,A,CC
    let r = dis(&[0x34, 0xFF]);
    assert_eq!(r.mnemonic, "PSHS");
    assert_eq!(r.operands, "PC,U,Y,X,DP,B,A,CC");
    assert_eq!(r.byte_len, 2);
}

#[test]
fn test_pshs_single() {
    // PSHS A (bit 1 = 0x02)
    let r = dis(&[0x34, 0x02]);
    assert_eq!(r.operands, "A");
}

#[test]
fn test_pshs_common() {
    // PSHS X,B,A (bits: 4=X, 2=B, 1=A = 0x16)
    let r = dis(&[0x34, 0x16]);
    assert_eq!(r.operands, "X,B,A");
}

#[test]
fn test_puls_pc() {
    // PULS PC (bit 7 = 0x80)
    let r = dis(&[0x35, 0x80]);
    assert_eq!(r.mnemonic, "PULS");
    assert_eq!(r.operands, "PC");
}

#[test]
fn test_pshu_all() {
    // PSHU with all registers (0xFF) = PC,S,Y,X,DP,B,A,CC
    let r = dis(&[0x36, 0xFF]);
    assert_eq!(r.mnemonic, "PSHU");
    assert_eq!(r.operands, "PC,S,Y,X,DP,B,A,CC");
}

#[test]
fn test_pulu_y_s() {
    // PULU Y,S (bits: 6=S, 5=Y = 0x60)
    let r = dis(&[0x37, 0x60]);
    assert_eq!(r.mnemonic, "PULU");
    assert_eq!(r.operands, "S,Y");
}

#[test]
fn test_pshs_empty() {
    // PSHS with no registers (0x00) — empty operand
    let r = dis(&[0x34, 0x00]);
    assert!(r.operands.is_empty());
}

// ── Page 2 instructions (0x10 prefix) ────────────────────────────────────────

#[test]
fn test_swi2() {
    let r = dis(&[0x10, 0x3F]);
    assert_eq!(r.mnemonic, "SWI2");
    assert_eq!(r.byte_len, 2);
    assert!(r.operands.is_empty());
}

#[test]
fn test_cmpd_imm() {
    let r = dis(&[0x10, 0x83, 0x12, 0x34]);
    assert_eq!(r.mnemonic, "CMPD");
    assert_eq!(r.operands, "#$1234");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_cmpd_dir() {
    let r = dis(&[0x10, 0x93, 0x42]);
    assert_eq!(r.mnemonic, "CMPD");
    assert_eq!(r.operands, "$42");
    assert_eq!(r.byte_len, 3);
    assert_eq!(r.target_addr, Some(0x0042));
}

#[test]
fn test_cmpy_ext() {
    let r = dis(&[0x10, 0xBC, 0xC0, 0x00]);
    assert_eq!(r.mnemonic, "CMPY");
    assert_eq!(r.operands, "$C000");
    assert_eq!(r.byte_len, 4);
    assert_eq!(r.target_addr, Some(0xC000));
}

#[test]
fn test_ldy_imm() {
    let r = dis(&[0x10, 0x8E, 0x40, 0x00]);
    assert_eq!(r.mnemonic, "LDY");
    assert_eq!(r.operands, "#$4000");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_sty_dir() {
    let r = dis(&[0x10, 0x9F, 0x80]);
    assert_eq!(r.mnemonic, "STY");
    assert_eq!(r.operands, "$80");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_ldy_idx() {
    // LDY ,X++ (postbyte 0x81)
    let r = dis(&[0x10, 0xAE, 0x81]);
    assert_eq!(r.mnemonic, "LDY");
    assert_eq!(r.operands, ",X++");
    assert_eq!(r.byte_len, 3);
}

#[test]
fn test_lds_imm() {
    let r = dis(&[0x10, 0xCE, 0x80, 0x00]);
    assert_eq!(r.mnemonic, "LDS");
    assert_eq!(r.operands, "#$8000");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_sts_ext() {
    let r = dis(&[0x10, 0xFF, 0x12, 0x34]);
    assert_eq!(r.mnemonic, "STS");
    assert_eq!(r.operands, "$1234");
    assert_eq!(r.byte_len, 4);
}

// ── Page 3 instructions (0x11 prefix) ────────────────────────────────────────

#[test]
fn test_swi3() {
    let r = dis(&[0x11, 0x3F]);
    assert_eq!(r.mnemonic, "SWI3");
    assert_eq!(r.byte_len, 2);
    assert!(r.operands.is_empty());
}

#[test]
fn test_cmpu_imm() {
    let r = dis(&[0x11, 0x83, 0x12, 0x34]);
    assert_eq!(r.mnemonic, "CMPU");
    assert_eq!(r.operands, "#$1234");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_cmps_ext() {
    let r = dis(&[0x11, 0xBC, 0xC0, 0x00]);
    assert_eq!(r.mnemonic, "CMPS");
    assert_eq!(r.operands, "$C000");
    assert_eq!(r.byte_len, 4);
}

#[test]
fn test_cmpu_idx() {
    // CMPU ,Y (postbyte 0xA4)
    let r = dis(&[0x11, 0xA3, 0xA4]);
    assert_eq!(r.mnemonic, "CMPU");
    assert_eq!(r.operands, ",Y");
    assert_eq!(r.byte_len, 3);
}

// ── Illegal opcodes ──────────────────────────────────────────────────────────

#[test]
fn test_illegal_page1() {
    for op in [0x01, 0x02, 0x05, 0x14, 0x38, 0x3E, 0x41, 0x87, 0xCD] {
        let r = dis(&[op]);
        assert_eq!(r.mnemonic, "???", "opcode 0x{:02X}", op);
        assert_eq!(r.byte_len, 1);
    }
}

#[test]
fn test_illegal_page2() {
    // 0x10 0x00 is illegal on page 2
    let r = dis(&[0x10, 0x00]);
    assert_eq!(r.mnemonic, "???");
    assert_eq!(r.byte_len, 2); // prefix + opcode consumed
}

#[test]
fn test_illegal_page3() {
    // 0x11 0x00 is illegal on page 3
    let r = dis(&[0x11, 0x00]);
    assert_eq!(r.mnemonic, "???");
    assert_eq!(r.byte_len, 2);
}

// ── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn test_empty_bytes() {
    let r = M6809::disassemble(0x1000, &[]);
    assert_eq!(r.mnemonic, "???");
    assert_eq!(r.byte_len, 1);
}

#[test]
fn test_truncated_page_prefix() {
    // Just 0x10 with no following byte
    let r = M6809::disassemble(0x1000, &[0x10]);
    assert_eq!(r.mnemonic, "???");
    assert_eq!(r.byte_len, 1);
}

#[test]
fn test_truncated_imm8() {
    // SUBA immediate but no operand byte
    let r = M6809::disassemble(0x1000, &[0x80]);
    assert_eq!(r.mnemonic, "???");
}

#[test]
fn test_truncated_ext() {
    // LDA extended but only hi byte
    let r = M6809::disassemble(0x1000, &[0xB6, 0xC0]);
    assert_eq!(r.mnemonic, "???");
}

#[test]
fn test_raw_bytes_captured() {
    let r = dis(&[0xB6, 0xC0, 0x00]);
    assert_eq!(r.bytes[0], 0xB6);
    assert_eq!(r.bytes[1], 0xC0);
    assert_eq!(r.bytes[2], 0x00);
    assert_eq!(r.byte_len, 3);
}

// ── Display formatting ──────────────────────────────────────────────────────

#[test]
fn test_display_inherent() {
    let r = dis(&[0x12]);
    assert_eq!(format!("{}", r), "NOP");
}

#[test]
fn test_display_imm8() {
    let r = dis(&[0x86, 0x42]);
    assert_eq!(format!("{}", r), "LDA   #$42");
}

#[test]
fn test_display_dir() {
    let r = dis(&[0x96, 0x80]);
    assert_eq!(format!("{}", r), "LDA   $80");
}

#[test]
fn test_display_ext() {
    let r = dis(&[0xB6, 0xC0, 0x00]);
    assert_eq!(format!("{}", r), "LDA   $C000");
}

#[test]
fn test_display_indexed() {
    let r = dis(&[0xA6, 0x84]);
    assert_eq!(format!("{}", r), "LDA   ,X");
}

#[test]
fn test_display_branch() {
    let r = M6809::disassemble(0x1000, &[0x27, 0x05, 0, 0, 0, 0]);
    assert_eq!(format!("{}", r), "BEQ   $1007");
}

#[test]
fn test_display_tfr() {
    let r = dis(&[0x1F, 0x01]);
    assert_eq!(format!("{}", r), "TFR   D,X");
}

#[test]
fn test_display_pshs() {
    let r = dis(&[0x34, 0x06]);
    assert_eq!(format!("{}", r), "PSHS  B,A");
}

#[test]
fn test_display_page2() {
    let r = dis(&[0x10, 0x83, 0x12, 0x34]);
    assert_eq!(format!("{}", r), "CMPD  #$1234");
}

// ── Symbol resolution ────────────────────────────────────────────────────────

#[test]
fn test_symbols_ext_match() {
    let r = dis(&[0xBD, 0xC0, 0x00]);
    let s = r.format_with_symbols(|addr| {
        if addr == 0xC000 {
            Some("IRQ_HANDLER")
        } else {
            None
        }
    });
    assert_eq!(s, "JSR   IRQ_HANDLER");
}

#[test]
fn test_symbols_dir_match() {
    let r = dis(&[0x96, 0x80]);
    let s = r.format_with_symbols(|addr| {
        if addr == 0x0080 {
            Some("COUNTER")
        } else {
            None
        }
    });
    assert_eq!(s, "LDA   COUNTER");
}

#[test]
fn test_symbols_branch_match() {
    let r = M6809::disassemble(0x1000, &[0x27, 0x05, 0, 0, 0, 0]);
    let s = r.format_with_symbols(|addr| {
        if addr == 0x1007 {
            Some("LOOP_END")
        } else {
            None
        }
    });
    assert_eq!(s, "BEQ   LOOP_END");
}

#[test]
fn test_symbols_no_match() {
    let r = dis(&[0xB6, 0xC0, 0x00]);
    let s = r.format_with_symbols(|_| None);
    assert_eq!(s, "LDA   $C000");
}

#[test]
fn test_symbols_no_target() {
    let r = dis(&[0x12]); // NOP — no target
    let s = r.format_with_symbols(|_| Some("SHOULD_NOT_MATCH"));
    assert_eq!(s, "NOP");
}

// ── LEA instructions ─────────────────────────────────────────────────────────

#[test]
fn test_leax() {
    // LEAX 1,S (postbyte 0x61: reg=S(11), 5-bit offset=1)
    let r = dis(&[0x30, 0x61]);
    assert_eq!(r.mnemonic, "LEAX");
    assert_eq!(r.operands, "1,S");
}

#[test]
fn test_leay_indexed() {
    // LEAY ,X++ (postbyte 0x81)
    let r = dis(&[0x31, 0x81]);
    assert_eq!(r.mnemonic, "LEAY");
    assert_eq!(r.operands, ",X++");
}

// ── Opcode coverage sweep ────────────────────────────────────────────────────

#[test]
fn test_all_valid_page1_opcodes() {
    // Every valid page 1 opcode should NOT produce "???"
    let valid_page1: Vec<u8> = vec![
        // 0x00-0x0F: DIR operations
        0x00, 0x03, 0x04, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0C, 0x0D, 0x0E, 0x0F,
        // 0x10-0x1F: misc
        0x12, 0x13, 0x16, 0x17, 0x19, 0x1A, 0x1C, 0x1D, 0x1E, 0x1F,
        // 0x20-0x2F: branches
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E,
        0x2F, // 0x30-0x3F: LEA/stack/misc
        0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3F,
        // 0x40-0x4F: A inherent
        0x40, 0x43, 0x44, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4C, 0x4D, 0x4F,
        // 0x50-0x5F: B inherent
        0x50, 0x53, 0x54, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5C, 0x5D, 0x5F,
        // 0x60-0x6F: indexed
        0x60, 0x63, 0x64, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6C, 0x6D, 0x6E, 0x6F,
        // 0x70-0x7F: extended
        0x70, 0x73, 0x74, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7C, 0x7D, 0x7E, 0x7F,
        // 0x80-0x8F: A-page imm
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E,
        // 0x90-0x9F: all valid
        0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E,
        0x9F, // 0xA0-0xAF: all valid
        0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE,
        0xAF, // 0xB0-0xBF: all valid
        0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE,
        0xBF, // 0xC0-0xCF: B-page imm
        0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCE,
        // 0xD0-0xDF: all valid
        0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE,
        0xDF, // 0xE0-0xEF: all valid
        0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE,
        0xEF, // 0xF0-0xFF: all valid
        0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
        0xFF,
    ];
    assert_eq!(valid_page1.len(), 221, "expected 221 valid page 1 opcodes");

    for &op in &valid_page1 {
        // Provide enough bytes for any addressing mode (indexed needs postbyte + data)
        let r = M6809::disassemble(0x1000, &[op, 0x84, 0x00, 0x00, 0x00, 0x00]);
        assert_ne!(
            r.mnemonic, "???",
            "page 1 opcode 0x{:02X} should be valid",
            op
        );
    }
}

#[test]
fn test_all_valid_page2_opcodes() {
    let valid_page2: Vec<u8> = vec![
        // Long branches
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E,
        0x2F, // SWI2
        0x3F, // CMPD
        0x83, 0x93, 0xA3, 0xB3, // CMPY
        0x8C, 0x9C, 0xAC, 0xBC, // LDY/STY
        0x8E, 0x9E, 0x9F, 0xAE, 0xAF, 0xBE, 0xBF, // LDS/STS
        0xCE, 0xDE, 0xDF, 0xEE, 0xEF, 0xFE, 0xFF,
    ];
    assert_eq!(valid_page2.len(), 38, "expected 38 valid page 2 opcodes");

    for &op in &valid_page2 {
        let r = M6809::disassemble(0x1000, &[0x10, op, 0x84, 0x00, 0x00, 0x00]);
        assert_ne!(
            r.mnemonic, "???",
            "page 2 opcode 0x10 0x{:02X} should be valid",
            op
        );
    }
}

#[test]
fn test_all_valid_page3_opcodes() {
    let valid_page3: Vec<u8> = vec![
        0x3F, // SWI3
        0x83, 0x93, 0xA3, 0xB3, // CMPU
        0x8C, 0x9C, 0xAC, 0xBC, // CMPS
    ];
    assert_eq!(valid_page3.len(), 9, "expected 9 valid page 3 opcodes");

    for &op in &valid_page3 {
        let r = M6809::disassemble(0x1000, &[0x11, op, 0x84, 0x00, 0x00, 0x00]);
        assert_ne!(
            r.mnemonic, "???",
            "page 3 opcode 0x11 0x{:02X} should be valid",
            op
        );
    }
}

#[test]
fn test_all_opcodes_have_valid_byte_len() {
    // Every opcode (valid or not) should produce a byte_len >= 1
    for first in 0..=255u8 {
        let r = M6809::disassemble(0x1000, &[first, 0x84, 0x00, 0x00, 0x00, 0x00]);
        assert!(
            r.byte_len >= 1,
            "opcode 0x{:02X} has byte_len {}",
            first,
            r.byte_len
        );
    }
}
