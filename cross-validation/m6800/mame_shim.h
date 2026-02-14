// MAME compatibility shim for standalone mame4all M6800 cross-validation.
// Provides all types, macros, and stubs that the mame4all m6800 code expects
// from MAME's infrastructure headers (osd_cpu.h, memory.h, cpuintrf.h, etc.)

#ifndef MAME_SHIM_H
#define MAME_SHIM_H

#include <cstdint>
#include <cstring>

// --- Type aliases (from osd_cpu.h) ---
typedef uint8_t  UINT8;
typedef uint16_t UINT16;
typedef uint32_t UINT32;
typedef int8_t   INT8;
typedef int16_t  INT16;
typedef int32_t  INT32;

// --- PAIR union (from osd_cpu.h) ---
// Endian-aware register union for 8/16/32-bit access.
// macOS ARM64 and x86-64 are both little-endian.
#ifdef __BIG_ENDIAN__
typedef union {
    struct { UINT8 h3, h2, h, l; } b;
    struct { INT8  h3, h2, h, l; } sb;
    struct { UINT16 h, l; } w;
    struct { INT16  h, l; } sw;
    UINT32 d;
    INT32  sd;
} PAIR;
#else // LSB_FIRST (little-endian)
typedef union {
    struct { UINT8 l, h, h2, h3; } b;
    struct { INT8  l, h, h2, h3; } sb;
    struct { UINT16 l, h; } w;
    struct { INT16  l, h; } sw;
    UINT32 d;
    INT32  sd;
} PAIR;
#endif

// --- Compiler macros ---
#define INLINE static inline

// --- Memory interface (from memory.h / cpuintrf.h) ---
// Flat 64KB memory array, defined in validate_m6800.cpp
extern uint8_t m6800_flat_memory[0x10000];

#define cpu_readmem16(addr)          ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_writemem16(addr, val)    (m6800_flat_memory[(addr) & 0xFFFF] = (UINT8)(val))
#define cpu_readop(addr)             ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_readop_arg(addr)         ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_readport16(port)         0
#define cpu_writeport16(port, val)   ((void)0)

// --- CPU interface stubs (from cpuintrf.h) ---
#define change_pc16(pc)              ((void)0)
#define CLEAR_LINE                   0
#define ASSERT_LINE                  1
#define HOLD_LINE                    2
#define REG_PREVIOUSPC               (-1)
#define REG_SP_CONTENTS              (-2)

// --- I/O handler macros (from memory.h) ---
#define READ_HANDLER(name)           UINT8 name(UINT32 offset)
#define WRITE_HANDLER(name)          void name(UINT32 offset, UINT8 data)

// --- State save stubs (from state.h) ---
#define state_save_register_UINT8(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_UINT16(mod, inst, name, ptr, cnt)  ((void)0)
#define state_save_register_INT32(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_int(mod, inst, name, ptr)          ((void)0)
#define state_save_register_func_postload(fn)                  ((void)0)

// Old-style state save/load stubs (used in m6800_state_save/load)
#define state_save_UINT8(file, mod, cpu, name, ptr, cnt)       ((void)0)
#define state_save_UINT16(file, mod, cpu, name, ptr, cnt)      ((void)0)
#define state_load_UINT8(file, mod, cpu, name, ptr, cnt)       ((void)0)
#define state_load_UINT16(file, mod, cpu, name, ptr, cnt)      ((void)0)
#define cpu_getactivecpu()           0

// --- Logging stubs (from osdepend.h) ---
#define logerror(...)                ((void)0)

// --- CPU info constants (from cpuintrf.h) ---
#define CPU_INFO_NAME    0
#define CPU_INFO_FAMILY  1
#define CPU_INFO_VERSION 2
#define CPU_INFO_FILE    3
#define CPU_INFO_CREDITS 4
#define CPU_INFO_REG_LAYOUT 100
#define CPU_INFO_WIN_LAYOUT 101

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

#endif // MAME_SHIM_H
