// MAME 0.148 emu.h shim for standalone M6809 cross-validation.
// The M6809 uses MAME's modern C++ device class pattern (not legacy),
// so this shim enables SHIM_MODERN_CPU_DEVICE in the shared header.

#pragma once
#ifndef EMU_H_M6809_SHIM
#define EMU_H_M6809_SHIM

// Tell mame0148_shim.h to use the modern cpu_device definition
#define SHIM_MODERN_CPU_DEVICE
#include "../mame0148_shim.h"

// ================================================================
// Flat memory array (defined in validate_m6809.cpp)
// ================================================================

extern uint8_t m6809_program[0x10000];

// ================================================================
// Disassembler stub
// ================================================================

static inline offs_t cpu_disassemble_m6809(cpu_device *, char *buf, offs_t,
                                           const UINT8 *, const UINT8 *, int) {
    sprintf(buf, "???");
    return 1;
}

#endif // EMU_H_M6809_SHIM
