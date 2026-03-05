// Shared MAME 0.148 CPU device framework shim.
// Provides minimal C++ class stubs and macro definitions for all
// MAME 0.148 CPU cores: legacy (MB88XX, M6800, MCS48) and modern (M6809).
//
// Each CPU's emu.h includes this header and adds CPU-specific memory
// arrays and address_space routing.
//
// Define SHIM_MODERN_CPU_DEVICE before including to get the modern
// C++ device pattern (machine_config, address_space_config, extended
// cpu_device with constructor/state_add/standard_irq_callback).

#pragma once
#ifndef MAME0148_SHIM_H
#define MAME0148_SHIM_H

// Prevent MAME headers from complaining about missing __EMU_H__
#define __EMU_H__

#include <cassert>
#include <cmath>
#include <cstdarg>
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

#ifndef FALSE
#define FALSE 0
#endif
#ifndef TRUE
#define TRUE (!FALSE)
#endif

// ================================================================
// PAIR union (osd_cpu.h) — endian-aware register pairs
// ================================================================

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

// ================================================================
// Constants
// ================================================================

enum { CLEAR_LINE = 0, ASSERT_LINE = 1, PULSE_LINE = 2 };
enum { INPUT_LINE_NMI = 32, INPUT_LINE_HALT = 33 };
enum { ENDIANNESS_BIG = 1, ENDIANNESS_LITTLE = 0 };

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
// attotime stub
// ================================================================

struct attotime {
    static const attotime never;
    static const attotime zero;
    static attotime from_hz(double) { return attotime{}; }
    static attotime from_ticks(UINT64, UINT32) { return attotime{}; }
};

// ================================================================
// emu_timer stub
// ================================================================

struct running_machine;

struct emu_timer {
    void adjust(attotime, INT32 = 0, attotime = attotime{}) {}
    void enable(bool) {}
};

// ================================================================
// device_scheduler / running_machine stubs
// ================================================================

struct device_scheduler {
    emu_timer m_timer;
    template<typename... Args>
    emu_timer *timer_alloc(Args&&...) { return &m_timer; }
    template<typename... Args>
    void synchronize(Args&&...) {}
};

struct running_machine {
    int debug_flags = 0;
    device_scheduler m_scheduler;
    device_scheduler &scheduler() { return m_scheduler; }
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
// devcb stubs (for M6800 SC2 output line, etc.)
// ================================================================

struct devcb_write_line {};
#define DEVCB_NULL devcb_write_line{}

struct devcb_resolved_write_line {
    void resolve(devcb_write_line, device_t &) {}
    void operator()(int) {}
};

// ================================================================
// address_space / direct_read_data
// ================================================================

typedef UINT8 (*shim_read_fn)(int space_id, offs_t addr);
typedef void  (*shim_write_fn)(int space_id, offs_t addr, UINT8 val);

// Set by each CPU's validate_*.cpp before use
extern shim_read_fn  shim_mem_read;
extern shim_write_fn shim_mem_write;

struct direct_read_data {
    UINT8 read_decrypted_byte(offs_t addr) {
        return shim_mem_read(AS_PROGRAM, addr);
    }
    UINT8 read_raw_byte(offs_t addr) {
        return shim_mem_read(AS_PROGRAM, addr);
    }
};

#ifndef SHIM_MODERN_CPU_DEVICE
// Forward reference — device_t defined later, we need a back-pointer
extern legacy_cpu_device *shim_active_device;
#endif

struct address_space {
    int space_id;
    direct_read_data m_direct;
    void *m_base;   // optional: for get_write_ptr (MCS48 register bank)

    address_space() : space_id(0), m_base(nullptr) {}

    UINT8 read_byte(offs_t addr) {
        return shim_mem_read(space_id, addr);
    }

    void write_byte(offs_t addr, UINT8 val) {
        shim_mem_write(space_id, addr, val);
    }

    direct_read_data &direct() { return m_direct; }

    // Used by MCS48 update_regptr
    void *get_write_ptr(offs_t addr) { return (uint8_t *)m_base + addr; }

    // Used by M6800 m6801_io_r/w to get back to the CPU state
    device_t &device();

    // Used by M6800 m6801_io_r/w — always false in validation
    bool debugger_access() const { return false; }
};

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
    char m_tag_buf[16];

public:
    device_t() : m_token(nullptr), m_static_config(nullptr),
                 m_clock(1536000) {
        m_spaces[0].space_id = AS_PROGRAM;
        m_spaces[1].space_id = AS_DATA;
        m_spaces[2].space_id = AS_IO;
        m_tag_buf[0] = '\0';
    }

    device_type type() const { return nullptr; }
    const char *tag() const { return m_tag_buf; }
    const void *static_config() const { return m_static_config; }
    UINT32 clock() const { return m_clock; }
    running_machine &machine() { return m_machine; }

    address_space &space(int sp) {
        if (sp >= 0 && sp < 3) return m_spaces[sp];
        return m_spaces[0];
    }

    // save_item is a no-op
    template<typename T> void save_item(T &, const char *) {}

    // interface() — used by MCS48 to get device_state_interface
    template<typename T> void interface(T *&ptr);
};

// ================================================================
// astring stub
// ================================================================

struct astring {
    char buf[64];
    astring() { buf[0] = '\0'; }
    astring &format(const char *fmt, ...) {
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(buf, sizeof(buf), fmt, ap);
        va_end(ap);
        return *this;
    }
    void printf(const char *fmt, ...) {
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(buf, sizeof(buf), fmt, ap);
        va_end(ap);
    }
    operator const char *() const { return buf; }
};

// ================================================================
// device_state_entry / device_state_interface stubs
// ================================================================

struct device_state_entry {
    int m_index;
    int index() const { return m_index; }
    device_state_entry &mask(UINT32) { return *this; }
    device_state_entry &noshow() { return *this; }
    device_state_entry &formatstr(const char *) { return *this; }
    device_state_entry &callimport() { return *this; }
    device_state_entry &callexport() { return *this; }
};

class device_state_interface {
    device_state_entry m_dummy;
public:
    template<typename T>
    device_state_entry &state_add(int index, const char *, T &) {
        m_dummy.m_index = index;
        return m_dummy;
    }
};

// Deferred implementation of device_t::interface()
template<typename T>
void device_t::interface(T *&ptr) {
    static T instance;
    ptr = &instance;
}

// ================================================================
// cpu_device / legacy_cpu_device
// ================================================================

#ifdef SHIM_MODERN_CPU_DEVICE
// ----------------------------------------------------------------
// Modern C++ device pattern (M6809)
// ----------------------------------------------------------------

struct machine_config {};

struct address_space_config {
    const char *m_name;
    int m_endianness;
    int m_databus_width;
    int m_addrbus_width;
    address_space_config()
        : m_name(""), m_endianness(ENDIANNESS_BIG),
          m_databus_width(8), m_addrbus_width(16) {}
    address_space_config(const char *name, int endian, int dbw, int abw)
        : m_name(name), m_endianness(endian),
          m_databus_width(dbw), m_addrbus_width(abw) {}
};

class cpu_device : public device_t {
public:
    int *m_icountptr = nullptr;
    device_state_interface m_state_iface;

    cpu_device() = default;
    cpu_device(const machine_config &, const device_type, const char *,
               const char *tag, device_t *, UINT32) {
        if (tag) {
            strncpy(m_tag_buf, tag, sizeof(m_tag_buf) - 1);
            m_tag_buf[sizeof(m_tag_buf) - 1] = '\0';
        }
    }

    int standard_irq_callback(int) { return 0; }

    template<typename T>
    device_state_entry &state_add(int index, const char *name, T &val) {
        return m_state_iface.state_add(index, name, val);
    }
};

template<typename T>
device_t *device_creator(void *, const char *, device_t *, UINT32) {
    return nullptr;
}

inline void static_set_static_config(device_t &, const void *) {}

// Reference downcast (pointer version below)
template<typename T>
T downcast(device_t &d) { return static_cast<T>(d); }

// address_space::device() not needed in modern pattern (uses m_program)
inline device_t &address_space::device() {
    static device_t dummy;
    return dummy;
}

#else
// ----------------------------------------------------------------
// Legacy C device pattern (M6800, MCS48, MB88XX)
// ----------------------------------------------------------------

class cpu_device : public device_t {};

class legacy_cpu_device : public cpu_device {
public:
    void *token() const { return m_token; }
    void set_token(void *t) { m_token = t; }
};

// Deferred implementation of address_space::device()
inline device_t &address_space::device() { return *shim_active_device; }

#endif // SHIM_MODERN_CPU_DEVICE

// downcast (pointer version, used by both patterns)
template<typename T>
T downcast(device_t *d) { return static_cast<T>(d); }

// ================================================================
// cpuinfo union (legacy pattern only, but typedefs are harmless)
// ================================================================

#ifndef SHIM_MODERN_CPU_DEVICE

typedef void (*address_map_constructor)(address_map &, device_t &);
typedef void (*cpu_set_info_func)(legacy_cpu_device *, UINT32, union cpuinfo *);
typedef void (*cpu_init_func)(legacy_cpu_device *, device_irq_acknowledge_callback);
typedef void (*cpu_reset_func)(legacy_cpu_device *);
typedef void (*cpu_exit_func)(legacy_cpu_device *);
typedef void (*cpu_execute_func)(legacy_cpu_device *);
typedef void (*cpu_burn_func)(legacy_cpu_device *, int);
typedef offs_t (*cpu_disassemble_func)(legacy_cpu_device *, char *, offs_t, const UINT8 *, const UINT8 *, int);
typedef void (*cpu_import_state_func)(legacy_cpu_device *, const device_state_entry &);
typedef void (*cpu_export_state_func)(legacy_cpu_device *, const device_state_entry &);
typedef void (*cpu_export_string_func)(legacy_cpu_device *, const device_state_entry &, astring &);

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
    cpu_import_state_func   import_state;
    cpu_export_state_func   export_state;
    cpu_export_string_func  export_string;
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

#define CPU_EXIT_NAME(name)        cpu_exit_##name
#define CPU_EXIT(name)             void CPU_EXIT_NAME(name)(legacy_cpu_device *device)

#define CPU_RESET_NAME(name)       cpu_reset_##name
#define CPU_RESET(name)            void CPU_RESET_NAME(name)(legacy_cpu_device *device)

#define CPU_EXECUTE_NAME(name)     cpu_execute_##name
#define CPU_EXECUTE(name)          void CPU_EXECUTE_NAME(name)(legacy_cpu_device *device)

#define CPU_DISASSEMBLE_NAME(name) cpu_disassemble_##name
#define CPU_DISASSEMBLE(name)      offs_t CPU_DISASSEMBLE_NAME(name)(legacy_cpu_device *device, char *buffer, offs_t pc, const UINT8 *oprom, const UINT8 *opram, int options)

#define CPU_IMPORT_STATE_NAME(name) cpu_import_state_##name
#define CPU_IMPORT_STATE(name)      void CPU_IMPORT_STATE_NAME(name)(legacy_cpu_device *device, const device_state_entry &entry)

#define CPU_EXPORT_STATE_NAME(name) cpu_export_state_##name
#define CPU_EXPORT_STATE(name)      void CPU_EXPORT_STATE_NAME(name)(legacy_cpu_device *device, const device_state_entry &entry)

#define CPU_EXPORT_STRING_NAME(name) cpu_export_string_##name
#define CPU_EXPORT_STRING(name)      void CPU_EXPORT_STRING_NAME(name)(legacy_cpu_device *device, const device_state_entry &entry, astring &string)

// ================================================================
// Device declaration/definition macros
// ================================================================

#define DECLARE_LEGACY_CPU_DEVICE(name, basename)                  \
    CPU_GET_INFO(basename);                                        \
    extern const device_type name;

#define DEFINE_LEGACY_CPU_DEVICE(name, basename)                   \
    const device_type name = nullptr;

#endif // !SHIM_MODERN_CPU_DEVICE

// ================================================================
// CPU_DISASSEMBLE macro (used by both patterns)
// ================================================================

#ifndef CPU_DISASSEMBLE_NAME
#define CPU_DISASSEMBLE_NAME(name) cpu_disassemble_##name
#define CPU_DISASSEMBLE(name)      offs_t CPU_DISASSEMBLE_NAME(name)(cpu_device *device, char *buffer, offs_t pc, const UINT8 *oprom, const UINT8 *opram, int options)
#endif

// ================================================================
// Common macros (used by both patterns)
// ================================================================

#define TIMER_CALLBACK(name)       void name(running_machine &machine, void *ptr, int param)

// NAME/FUNC macros for save_item and timer_alloc
#define NAME(x) x, #x
#define FUNC(x) &x, #x

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
#define AM_NOP
#define AM_READWRITE_LEGACY(r, w)

// ================================================================
// Read/write handler macros (for M6800 m6801_io_r/w)
// ================================================================

#define DECLARE_READ8_HANDLER(name)  UINT8 name(address_space &, offs_t)
#define DECLARE_WRITE8_HANDLER(name) void name(address_space &, offs_t, UINT8)
#define READ8_HANDLER(name)          UINT8 name(address_space &space, offs_t offset)
#define WRITE8_HANDLER(name)         void name(address_space &space, offs_t offset, UINT8 data)

// ================================================================
// Misc stubs
// ================================================================

#define debugger_instruction_hook(dev, pc) ((void)0)
#define fatalerror(...) do { fprintf(stderr, __VA_ARGS__); abort(); } while(0)
#define logerror(...) ((void)0)

#endif // MAME0148_SHIM_H
