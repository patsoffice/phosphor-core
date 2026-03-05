// MAME 0.148 emu.h shim for standalone MCS-48 (I8035) cross-validation.
// Provides CPU-specific stubs on top of the shared framework.

#pragma once
#ifndef EMU_H_MCS48_SHIM
#define EMU_H_MCS48_SHIM

#include "../mame0148_shim.h"

// ================================================================
// Flat memory arrays (defined in validate_i8035.cpp)
//
// MCS-48 has 3 address spaces:
//   AS_PROGRAM: up to 4KB ROM/external program memory
//   AS_DATA:    up to 256 bytes internal RAM
//   AS_IO:      port-mapped I/O (0x100-0x121 for special ports)
// ================================================================

extern uint8_t mcs48_program[4096];
extern uint8_t mcs48_data[256];
extern uint8_t mcs48_io[512];

// ================================================================
// Disassembler stubs
// ================================================================

static inline offs_t cpu_disassemble_mcs48(legacy_cpu_device *, char *buf, offs_t,
                                           const UINT8 *, const UINT8 *, int) {
    sprintf(buf, "???");
    return 1;
}
static inline offs_t cpu_disassemble_upi41(legacy_cpu_device *, char *buf, offs_t,
                                           const UINT8 *, const UINT8 *, int) {
    sprintf(buf, "???");
    return 1;
}

#endif // EMU_H_MCS48_SHIM
