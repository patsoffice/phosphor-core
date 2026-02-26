// MAME memory.h shim for standalone mame4all M6800 cross-validation.
// Provides flat 64KB memory access macros.

#ifndef MEMORY_H
#define MEMORY_H

#include "osd_cpu.h"

// --- Memory interface ---
// Flat 64KB memory array, defined in validate_m6800.cpp
extern uint8_t m6800_flat_memory[0x10000];

#define cpu_readmem16(addr)          ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_writemem16(addr, val)    (m6800_flat_memory[(addr) & 0xFFFF] = (UINT8)(val))
#define cpu_readop(addr)             ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_readop_arg(addr)         ((unsigned)m6800_flat_memory[(addr) & 0xFFFF])
#define cpu_readport16(port)         0
#define cpu_writeport16(port, val)   ((void)0)

// --- I/O handler macros (from memory.h) ---
#define READ_HANDLER(name)           UINT8 name(UINT32 offset)
#define WRITE_HANDLER(name)          void name(UINT32 offset, UINT8 data)

#endif // MEMORY_H
