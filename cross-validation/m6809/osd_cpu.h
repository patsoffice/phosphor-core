// MAME osd_cpu.h shim for standalone mame4all M6809 cross-validation.
// Provides type aliases and the PAIR union that the mame4all code expects.

#ifndef OSD_CPU_H
#define OSD_CPU_H

#include <cstdint>

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

#ifndef FALSE
#define FALSE 0
#endif
#ifndef TRUE
#define TRUE (!FALSE)
#endif

#endif // OSD_CPU_H
