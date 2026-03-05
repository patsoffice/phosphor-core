// MAME 0.148 emu.h shim for standalone MB88XX cross-validation.
// Provides minimal C++ class stubs and macro definitions allowing
// mame0148/src/emu/cpu/mb88xx/mb88xx.c to compile unmodified.

#pragma once
#ifndef EMU_H_SHIM
#define EMU_H_SHIM

// Prevent MAME headers from complaining about missing __EMU_H__
#define __EMU_H__

#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

// ================================================================
// Basic types (osdcomm.h)
// ================================================================

typedef uint8_t   UINT8;
typedef uint16_t  UINT16;
typedef uint32_t  UINT32;
typedef uint64_t  UINT64;
typedef int8_t    INT8;
typedef int16_t   INT16;
typedef int32_t   INT32;
typedef int64_t   INT64;
typedef UINT32    offs_t;
typedef void      genf(void);

#define INLINE static inline

// ================================================================
// Constants
// ================================================================

enum { CLEAR_LINE = 0, ASSERT_LINE = 1 };
enum { ENDIANNESS_BIG = 1 };

// State register IDs (devstate.h)
enum {
    STATE_GENPC     = -1,
    STATE_GENPCBASE = -2,
    STATE_GENSP     = -3,
    STATE_GENFLAGS  = -4
};

// Address space IDs (memory.h)
const int ADDRESS_SPACES = 4;
enum address_spacenum {
    AS_0 = 0, AS_1, AS_2, AS_3,
    AS_PROGRAM = AS_0,
    AS_DATA    = AS_1,
    AS_IO      = AS_2
};

const int MAX_INPUT_LINES = 35;
const int MAX_REGS = 256;

// CPUINFO constants (devcpu.h)
enum {
    CPUINFO_INT_FIRST = 0x00000,
        CPUINFO_INT_ENDIANNESS = CPUINFO_INT_FIRST,
        CPUINFO_INT_DATABUS_WIDTH,
        CPUINFO_INT_DATABUS_WIDTH_0 = CPUINFO_INT_DATABUS_WIDTH + 0,
        CPUINFO_INT_DATABUS_WIDTH_1 = CPUINFO_INT_DATABUS_WIDTH + 1,
        CPUINFO_INT_DATABUS_WIDTH_2 = CPUINFO_INT_DATABUS_WIDTH + 2,
        CPUINFO_INT_DATABUS_WIDTH_3 = CPUINFO_INT_DATABUS_WIDTH + 3,
        CPUINFO_INT_DATABUS_WIDTH_LAST = CPUINFO_INT_DATABUS_WIDTH + ADDRESS_SPACES - 1,
        CPUINFO_INT_ADDRBUS_WIDTH,
        CPUINFO_INT_ADDRBUS_WIDTH_0 = CPUINFO_INT_ADDRBUS_WIDTH + 0,
        CPUINFO_INT_ADDRBUS_WIDTH_1 = CPUINFO_INT_ADDRBUS_WIDTH + 1,
        CPUINFO_INT_ADDRBUS_WIDTH_2 = CPUINFO_INT_ADDRBUS_WIDTH + 2,
        CPUINFO_INT_ADDRBUS_WIDTH_3 = CPUINFO_INT_ADDRBUS_WIDTH + 3,
        CPUINFO_INT_ADDRBUS_WIDTH_LAST = CPUINFO_INT_ADDRBUS_WIDTH + ADDRESS_SPACES - 1,
        CPUINFO_INT_ADDRBUS_SHIFT,
        CPUINFO_INT_ADDRBUS_SHIFT_0 = CPUINFO_INT_ADDRBUS_SHIFT + 0,
        CPUINFO_INT_ADDRBUS_SHIFT_1 = CPUINFO_INT_ADDRBUS_SHIFT + 1,
        CPUINFO_INT_ADDRBUS_SHIFT_2 = CPUINFO_INT_ADDRBUS_SHIFT + 2,
        CPUINFO_INT_ADDRBUS_SHIFT_3 = CPUINFO_INT_ADDRBUS_SHIFT + 3,
        CPUINFO_INT_ADDRBUS_SHIFT_LAST = CPUINFO_INT_ADDRBUS_SHIFT + ADDRESS_SPACES - 1,

        CPUINFO_INT_CONTEXT_SIZE = 0x04000,
        CPUINFO_INT_INPUT_LINES,
        CPUINFO_INT_DEFAULT_IRQ_VECTOR,
        CPUINFO_INT_CLOCK_MULTIPLIER,
        CPUINFO_INT_CLOCK_DIVIDER,
        CPUINFO_INT_MIN_INSTRUCTION_BYTES,
        CPUINFO_INT_MAX_INSTRUCTION_BYTES,
        CPUINFO_INT_MIN_CYCLES,
        CPUINFO_INT_MAX_CYCLES,

        CPUINFO_INT_LOGADDR_WIDTH,
        CPUINFO_INT_LOGADDR_WIDTH_PROGRAM = CPUINFO_INT_LOGADDR_WIDTH + AS_PROGRAM,
        CPUINFO_INT_LOGADDR_WIDTH_DATA = CPUINFO_INT_LOGADDR_WIDTH + AS_DATA,
        CPUINFO_INT_LOGADDR_WIDTH_IO = CPUINFO_INT_LOGADDR_WIDTH + AS_IO,
        CPUINFO_INT_LOGADDR_WIDTH_LAST = CPUINFO_INT_LOGADDR_WIDTH + ADDRESS_SPACES - 1,
        CPUINFO_INT_PAGE_SHIFT,
        CPUINFO_INT_PAGE_SHIFT_PROGRAM = CPUINFO_INT_PAGE_SHIFT + AS_PROGRAM,
        CPUINFO_INT_PAGE_SHIFT_DATA = CPUINFO_INT_PAGE_SHIFT + AS_DATA,
        CPUINFO_INT_PAGE_SHIFT_IO = CPUINFO_INT_PAGE_SHIFT + AS_IO,
        CPUINFO_INT_PAGE_SHIFT_LAST = CPUINFO_INT_PAGE_SHIFT + ADDRESS_SPACES - 1,

        CPUINFO_INT_INPUT_STATE,
        CPUINFO_INT_INPUT_STATE_LAST = CPUINFO_INT_INPUT_STATE + MAX_INPUT_LINES - 1,
        CPUINFO_INT_REGISTER = CPUINFO_INT_INPUT_STATE_LAST + 10,
        CPUINFO_INT_SP = CPUINFO_INT_REGISTER + STATE_GENSP,
        CPUINFO_INT_PC = CPUINFO_INT_REGISTER + STATE_GENPC,
        CPUINFO_INT_PREVIOUSPC = CPUINFO_INT_REGISTER + STATE_GENPCBASE,

        CPUINFO_IS_OCTAL = CPUINFO_INT_REGISTER + MAX_REGS - 2,
        CPUINFO_INT_REGISTER_LAST = CPUINFO_INT_REGISTER + MAX_REGS - 1,

    CPUINFO_INT_CPU_SPECIFIC = 0x08000,

    CPUINFO_PTR_FIRST = 0x10000,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP = CPUINFO_PTR_FIRST,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP_0 = CPUINFO_PTR_INTERNAL_MEMORY_MAP + 0,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP_1 = CPUINFO_PTR_INTERNAL_MEMORY_MAP + 1,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP_2 = CPUINFO_PTR_INTERNAL_MEMORY_MAP + 2,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP_3 = CPUINFO_PTR_INTERNAL_MEMORY_MAP + 3,
        CPUINFO_PTR_INTERNAL_MEMORY_MAP_LAST = CPUINFO_PTR_INTERNAL_MEMORY_MAP + ADDRESS_SPACES - 1,
        CPUINFO_PTR_DEFAULT_MEMORY_MAP,
        CPUINFO_PTR_DEFAULT_MEMORY_MAP_LAST = CPUINFO_PTR_DEFAULT_MEMORY_MAP + ADDRESS_SPACES - 1,

        CPUINFO_PTR_INSTRUCTION_COUNTER = 0x14000,

    CPUINFO_PTR_CPU_SPECIFIC = 0x18000,

    CPUINFO_FCT_FIRST = 0x20000,
        CPUINFO_FCT_SET_INFO = 0x24000,
        CPUINFO_FCT_INIT,
        CPUINFO_FCT_RESET,
        CPUINFO_FCT_EXIT,
        CPUINFO_FCT_EXECUTE,
        CPUINFO_FCT_BURN,
        CPUINFO_FCT_DISASSEMBLE,
        CPUINFO_FCT_TRANSLATE,
        CPUINFO_FCT_READ,
        CPUINFO_FCT_WRITE,
        CPUINFO_FCT_READOP,
        CPUINFO_FCT_DEBUG_INIT,
        CPUINFO_FCT_IMPORT_STATE,
        CPUINFO_FCT_EXPORT_STATE,
        CPUINFO_FCT_IMPORT_STRING,
        CPUINFO_FCT_EXPORT_STRING,

    CPUINFO_FCT_CPU_SPECIFIC = 0x28000,

    CPUINFO_STR_FIRST = 0x30000,
        CPUINFO_STR_NAME = CPUINFO_STR_FIRST,
        CPUINFO_STR_SHORTNAME,
        CPUINFO_STR_FAMILY,
        CPUINFO_STR_VERSION,
        CPUINFO_STR_SOURCE_FILE,
        CPUINFO_STR_CREDITS,

        CPUINFO_STR_REGISTER = 0x34000 + 10,
        CPUINFO_STR_FLAGS = CPUINFO_STR_REGISTER + STATE_GENFLAGS,
        CPUINFO_STR_REGISTER_LAST = CPUINFO_STR_REGISTER + MAX_REGS - 1,

    CPUINFO_STR_CPU_SPECIFIC = 0x38000
};

// ================================================================
// Forward declarations
// ================================================================

class device_t;
class legacy_cpu_device;
class cpu_device;
struct address_map;

// ================================================================
// Flat memory arrays (defined in validate_mb88xx.cpp)
// ================================================================

extern uint8_t mb88_program[2048];
extern uint8_t mb88_data[128];
extern uint8_t mb88_io[8];

// ================================================================
// attotime stub
// ================================================================

struct attotime {
    static const attotime never;
    static const attotime zero;
    static attotime from_hz(double) { return attotime{}; }
};

// ================================================================
// emu_timer stub (serial timer is no-op for single-step validation)
// ================================================================

struct running_machine;

struct emu_timer {
    void adjust(attotime, INT32 = 0, attotime = attotime{}) {}
};

// ================================================================
// device_scheduler / running_machine stubs
// ================================================================

struct device_scheduler {
    emu_timer m_timer;
    template<typename... Args>
    emu_timer *timer_alloc(Args&&...) { return &m_timer; }
};

struct running_machine {
    int debug_flags = 0;
    device_scheduler m_scheduler;
    device_scheduler &scheduler() { return m_scheduler; }
};

// ================================================================
// Memory access classes
// ================================================================

struct direct_read_data {
    UINT8 read_decrypted_byte(offs_t addr) {
        return mb88_program[addr & 0x7FF];
    }
};

struct address_space {
    int space_id;
    direct_read_data m_direct;

    address_space() : space_id(0) {}

    UINT8 read_byte(offs_t addr) {
        switch (space_id) {
            case AS_DATA: return mb88_data[addr & 0x7F];
            case AS_IO:   return mb88_io[addr & 0x07];
            default:      return mb88_program[addr & 0x7FF];
        }
    }

    void write_byte(offs_t addr, UINT8 val) {
        switch (space_id) {
            case AS_DATA: mb88_data[addr & 0x7F] = val; break;
            case AS_IO:   mb88_io[addr & 0x07] = val; break;
            default:      mb88_program[addr & 0x7FF] = val; break;
        }
    }

    direct_read_data &direct() { return m_direct; }
};

// ================================================================
// device_type
// ================================================================

typedef device_t *(*device_type)(void *, const char *, device_t *, UINT32);

// ================================================================
// device_irq_acknowledge_callback
// ================================================================

typedef int (*device_irq_acknowledge_callback)(device_t *device, int irqnum);

// ================================================================
// device_t
// ================================================================

class device_t {
protected:
    void *m_token;
    running_machine m_machine;
    address_space m_spaces[3];
    const void *m_static_config;
    UINT32 m_clock;

public:
    device_t() : m_token(nullptr), m_static_config(nullptr),
                 m_clock(1536000) {
        m_spaces[0].space_id = AS_PROGRAM;
        m_spaces[1].space_id = AS_DATA;
        m_spaces[2].space_id = AS_IO;
    }

    device_type type() const { return nullptr; }
    const void *static_config() const { return m_static_config; }
    UINT32 clock() const { return m_clock; }
    running_machine &machine() { return m_machine; }

    address_space &space(int sp) {
        if (sp >= 0 && sp < 3) return m_spaces[sp];
        return m_spaces[0];
    }

    // save_item is a no-op
    template<typename T> void save_item(T &, const char *) {}
};

// ================================================================
// cpu_device / legacy_cpu_device
// ================================================================

class cpu_device : public device_t {};

class legacy_cpu_device : public cpu_device {
public:
    void *token() const { return m_token; }
    void set_token(void *t) { m_token = t; }
};

// downcast
template<typename T>
T downcast(device_t *d) { return static_cast<T>(d); }

// ================================================================
// cpuinfo union
// ================================================================

// Forward types needed by cpuinfo
typedef void (*address_map_constructor)(address_map &, device_t &);
typedef void (*cpu_set_info_func)(legacy_cpu_device *, UINT32, union cpuinfo *);
typedef void (*cpu_init_func)(legacy_cpu_device *, device_irq_acknowledge_callback);
typedef void (*cpu_reset_func)(legacy_cpu_device *);
typedef void (*cpu_exit_func)(legacy_cpu_device *);
typedef void (*cpu_execute_func)(legacy_cpu_device *);
typedef void (*cpu_burn_func)(legacy_cpu_device *, int);
typedef offs_t (*cpu_disassemble_func)(legacy_cpu_device *, char *, offs_t, const UINT8 *, const UINT8 *, int);

union cpuinfo {
    INT64                   i;
    void *                  p;
    genf *                  f;
    char *                  s;
    cpu_set_info_func       setinfo;
    cpu_init_func           init;
    cpu_reset_func          reset;
    cpu_exit_func           exit;
    cpu_execute_func        execute;
    cpu_burn_func           burn;
    cpu_disassemble_func    disassemble;
    int *                   icount;
    address_map_constructor internal_map8;
};

// ================================================================
// CPU interface macros (devcpu.h)
// ================================================================

#define CPU_GET_INFO_NAME(name)    cpu_get_info_##name
#define CPU_GET_INFO(name)         void CPU_GET_INFO_NAME(name)(legacy_cpu_device *device, UINT32 state, cpuinfo *info)
#define CPU_GET_INFO_CALL(name)    CPU_GET_INFO_NAME(name)(device, state, info)

#define CPU_SET_INFO_NAME(name)    cpu_set_info_##name
#define CPU_SET_INFO(name)         void CPU_SET_INFO_NAME(name)(legacy_cpu_device *device, UINT32 state, cpuinfo *info)

#define CPU_INIT_NAME(name)        cpu_init_##name
#define CPU_INIT(name)             void CPU_INIT_NAME(name)(legacy_cpu_device *device, device_irq_acknowledge_callback irqcallback)

#define CPU_RESET_NAME(name)       cpu_reset_##name
#define CPU_RESET(name)            void CPU_RESET_NAME(name)(legacy_cpu_device *device)

#define CPU_EXECUTE_NAME(name)     cpu_execute_##name
#define CPU_EXECUTE(name)          void CPU_EXECUTE_NAME(name)(legacy_cpu_device *device)

#define CPU_DISASSEMBLE_NAME(name) cpu_disassemble_##name
#define CPU_DISASSEMBLE(name)      offs_t CPU_DISASSEMBLE_NAME(name)(legacy_cpu_device *device, char *buffer, offs_t pc, const UINT8 *oprom, const UINT8 *opram, int options)

#define TIMER_CALLBACK(name)       void name(running_machine &machine, void *ptr, int param)

// NAME/FUNC macros for save_item and timer_alloc
#define NAME(x) x, #x
#define FUNC(x) &x, #x

// ================================================================
// Device declaration/definition macros
// ================================================================

#define DECLARE_LEGACY_CPU_DEVICE(name, basename)                  \
    CPU_GET_INFO(basename);                                        \
    extern const device_type name;

#define DEFINE_LEGACY_CPU_DEVICE(name, basename)                   \
    const device_type name = nullptr;

// ================================================================
// Address map macros (expand to empty functions)
// ================================================================

struct address_map {};

#define ADDRESS_MAP_NAME(n)  construct_address_map_##n
#define ADDRESS_MAP_START(n, sp, bits, cls) \
    void ADDRESS_MAP_NAME(n)(address_map &, device_t &) {
#define ADDRESS_MAP_END }
#define AM_RANGE(s, e)
#define AM_ROM
#define AM_RAM

// ================================================================
// Misc stubs
// ================================================================

#define debugger_instruction_hook(dev, pc) ((void)0)
#define fatalerror(...) do { fprintf(stderr, __VA_ARGS__); abort(); } while(0)

// Stub for the disassembler (defined in mb88dasm.c which we don't compile)
offs_t cpu_disassemble_mb88(legacy_cpu_device *, char *buffer, offs_t pc,
                            const UINT8 *, const UINT8 *, int) {
    sprintf(buffer, "???");
    return 1;
}

// Device type constants are defined by DEFINE_LEGACY_CPU_DEVICE in mb88xx.c.
// They point to legacy_device_creator<> which returns nullptr — this makes
// get_safe_token()'s assertions pass since all type pointers are equal.

#endif // EMU_H_SHIM
