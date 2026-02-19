// MAME compatibility shim for standalone mame4all I8039 cross-validation.
// Provides all types, macros, and stubs that the mame4all i8039 code expects
// from MAME's infrastructure headers (osd_cpu.h, memory.h, cpuintrf.h).

#ifndef MAME_SHIM_I8039_H
#define MAME_SHIM_I8039_H

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
#ifndef INLINE
#define INLINE static inline
#endif

// --- Memory interface (from memory.h) ---
// Flat 64KB memory array for program memory, defined in validate_i8035.cpp
extern uint8_t i8039_program_memory[0x10000];

#define cpu_readmem16(addr)          ((unsigned)i8039_program_memory[(addr) & 0xFFFF])
#define cpu_writemem16(addr, val)    (i8039_program_memory[(addr) & 0xFFFF] = (UINT8)(val))
#define cpu_readop(addr)             ((unsigned)i8039_program_memory[(addr) & 0xFFFF])
#define cpu_readop_arg(addr)         ((unsigned)i8039_program_memory[(addr) & 0xFFFF])

// --- Port I/O interface ---
// 512-byte array: 0x000-0x0FF = external data memory (MOVX),
//                 0x100-0x1FF = ports (P1, P2, P4-P7, T0, T1, BUS)
extern uint8_t i8039_port_io[0x200];

#define cpu_readport(addr)           ((UINT8)i8039_port_io[(addr) & 0x1FF])
#define cpu_writeport(addr, val)     (i8039_port_io[(addr) & 0x1FF] = (UINT8)(val))

// --- Port handling mode ---
#define OLDPORTHANDLING 0

// --- CPU interface stubs (from cpuintrf.h) ---
#define change_pc(pc)                ((void)0)
#define change_pc16(pc)              ((void)0)
#define CLEAR_LINE                   0
#define ASSERT_LINE                  1
#define HOLD_LINE                    2
#define REG_PREVIOUSPC               (-1)
#define REG_SP_CONTENTS              (-2)

// --- State save stubs (from state.h) ---
#define state_save_register_UINT8(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_UINT16(mod, inst, name, ptr, cnt)  ((void)0)
#define state_save_register_INT32(mod, inst, name, ptr, cnt)   ((void)0)
#define state_save_register_int(mod, inst, name, ptr)          ((void)0)
#define state_save_register_func_postload(fn)                  ((void)0)

// --- Logging stubs ---
#define logerror(...)                ((void)0)

// --- CPU info constants (from cpuintrf.h) ---
#define CPU_INFO_NAME    0
#define CPU_INFO_FAMILY  1
#define CPU_INFO_VERSION 2
#define CPU_INFO_FILE    3
#define CPU_INFO_CREDITS 4
#define CPU_INFO_REG_LAYOUT 100
#define CPU_INFO_WIN_LAYOUT 101

// --- CPU variant selection ---
// Only compile the I8035 variant (thin wrapper around I8039)
#define HAS_I8035   1
#define HAS_I8048   0
#define HAS_N7751   0

#endif // MAME_SHIM_I8039_H
