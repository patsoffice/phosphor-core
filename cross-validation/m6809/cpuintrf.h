// MAME cpuintrf.h shim for standalone mame4all M6809 cross-validation.
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

// --- CPU variant selection ---
#define HAS_HD6309  0

// --- Misc stubs ---
#define cpu_getactivecpu()           0

#endif // CPUINTRF_H
