// MAME 0.148 emu.h shim for standalone MB88XX cross-validation.
// Provides CPU-specific memory routing on top of the shared framework.

#pragma once
#ifndef EMU_H_MB88XX_SHIM
#define EMU_H_MB88XX_SHIM

#include "../mame0148_shim.h"

// ================================================================
// Flat memory arrays (defined in validate_mb88xx.cpp)
// ================================================================

extern uint8_t mb88_program[2048];
extern uint8_t mb88_data[128];
extern uint8_t mb88_io[8];

// ================================================================
// Disassembler stub (mb88dasm.c is not compiled)
// ================================================================

static inline offs_t cpu_disassemble_mb88(legacy_cpu_device *, char *buffer,
                                          offs_t, const UINT8 *, const UINT8 *, int) {
    sprintf(buffer, "???");
    return 1;
}

#endif // EMU_H_MB88XX_SHIM
