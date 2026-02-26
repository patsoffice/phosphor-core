// MAME memory.h shim for standalone mame4all I8039 cross-validation.
// Provides program memory and port I/O access macros.

#ifndef MEMORY_H
#define MEMORY_H

#include "osd_cpu.h"

// --- Program memory interface ---
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

// --- I/O handler macros (from memory.h) ---
#define READ_HANDLER(name)           UINT8 name(UINT32 offset)
#define WRITE_HANDLER(name)          void name(UINT32 offset, UINT8 data)

#endif // MEMORY_H
