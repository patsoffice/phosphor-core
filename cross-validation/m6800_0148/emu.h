// MAME 0.148 emu.h shim for standalone M6800 cross-validation.
// Provides CPU-specific stubs on top of the shared framework.

#pragma once
#ifndef EMU_H_M6800_SHIM
#define EMU_H_M6800_SHIM

#include "../mame0148_shim.h"

// ================================================================
// Flat memory (defined in validate_m6800.cpp)
// ================================================================

extern uint8_t m6800_program[0x10000];

// ================================================================
// Disassembler stubs (6800dasm.c is not compiled)
// ================================================================

static inline offs_t cpu_disassemble_m6800(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_m6801(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_m6802(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_m6803(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_m6808(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_hd6301(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_hd63701(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }
static inline offs_t cpu_disassemble_nsc8105(legacy_cpu_device *, char *buf, offs_t, const UINT8 *, const UINT8 *, int) { sprintf(buf, "???"); return 1; }

#endif // EMU_H_M6800_SHIM
