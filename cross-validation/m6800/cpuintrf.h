// MAME cpuintrf.h shim for standalone mame4all M6800 cross-validation.
// Provides CPU interface stubs and constants.

#ifndef CPUINTRF_H
#define CPUINTRF_H

#include "osd_cpu.h"

// --- CPU interface stubs ---
#define change_pc16(pc)              ((void)0)
#define CLEAR_LINE                   0
#define ASSERT_LINE                  1
#define HOLD_LINE                    2
#define REG_PREVIOUSPC               (-1)
#define REG_SP_CONTENTS              (-2)

// --- CPU info constants ---
#define CPU_INFO_NAME    0
#define CPU_INFO_FAMILY  1
#define CPU_INFO_VERSION 2
#define CPU_INFO_FILE    3
#define CPU_INFO_CREDITS 4
#define CPU_INFO_REG_LAYOUT 100
#define CPU_INFO_WIN_LAYOUT 101

// --- Logging stubs ---
#define logerror(...)                ((void)0)

// --- Misc stubs ---
#define cpu_getactivecpu()           0

// --- M6808 WAI alias (used in 6800ops.cpp wai instruction) ---
#define M6808_WAI M6800_WAI

// --- CPU variant selection ---
// Only compile the base M6800 variant
#define HAS_M6800   1
#define HAS_M6801   0
#define HAS_M6802   0
#define HAS_M6803   0
#define HAS_M6808   0
#define HAS_HD63701 0
#define HAS_NSC8105 0

#endif // CPUINTRF_H
