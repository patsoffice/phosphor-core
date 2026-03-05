// Cross-validation harness for phosphor-core I8035 (MCS-48) CPU
// Links MAME 0.148 mcs48.c as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <string>
#include <vector>

// Our shim emu.h (found via -Imcs48_0148 include path)
#include "mcs48_0148/emu.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat memory arrays used by the emu.h shim's address_space stubs
uint8_t mcs48_program[4096];
uint8_t mcs48_data[256];
uint8_t mcs48_io[512];

// Memory routing for mame0148_shim.h address_space
static UINT8 mcs48_read(int space_id, offs_t addr) {
    switch (space_id) {
        case AS_DATA: return mcs48_data[addr & 0xFF];
        case AS_IO:   return mcs48_io[addr & 0x1FF];
        default:      return mcs48_program[addr & 0xFFF];
    }
}
static void mcs48_write(int space_id, offs_t addr, UINT8 val) {
    switch (space_id) {
        case AS_DATA: mcs48_data[addr & 0xFF] = val; break;
        case AS_IO:   mcs48_io[addr & 0x1FF] = val; break;
        default:      mcs48_program[addr & 0xFFF] = val; break;
    }
}
shim_read_fn  shim_mem_read  = mcs48_read;
shim_write_fn shim_mem_write = mcs48_write;

// attotime static constants
const attotime attotime::never = attotime{};
const attotime attotime::zero  = attotime{};

// Include mcs48.c directly so its static functions are accessible.
// The shim emu.h satisfies all MAME framework dependencies.
#include "mame0148/src/emu/cpu/mcs48/mcs48.c"

// --- Harness state ---

static legacy_cpu_device g_device;
legacy_cpu_device *shim_active_device = &g_device;
static mcs48_state g_state;

static int irq_callback_stub(device_t *, int) { return 0; }

static void init_mame_cpu() {
    memset(&g_state, 0, sizeof(g_state));
    g_device.set_token(&g_state);
    // Set m_base for AS_DATA so get_write_ptr() works (used by update_regptr)
    g_device.space(AS_DATA).m_base = mcs48_data;
    // I8035: external ROM, 64 bytes internal RAM, MCS48 feature set
    cpu_init_mcs48_norom(&g_device, irq_callback_stub);
}

static void reset_mame_cpu() {
    cpu_reset_mcs48(&g_device);
}

// Execute one instruction. Returns cycles consumed.
static int execute_one() {
    g_state.icount = 1;
    cpu_execute_mcs48(&g_device);
    return 1 - g_state.icount;
}

// --- A11 workaround ---
// MAME 0.148 (like mame4all) sets a11 immediately on SEL MB0/MB1,
// but real hardware defers it to the next JMP/CALL. For JMP/CALL
// opcodes, we pre-latch a11 from a11_pending so MAME uses the
// correct bank. For SEL MB0/MB1, we skip the a11 comparison.

static bool is_jmp_call(uint8_t opcode) {
    return (opcode & 0x1F) == 0x04 ||  // JMP_n
           (opcode & 0x1F) == 0x14;    // CALL_n
}

static bool is_sel_mb(uint8_t opcode) {
    return opcode == 0xE5 || opcode == 0xF5;
}

// Expander port opcodes use the 8243 protocol which modifies P2's
// lower nibble. Without an actual 8243 connected, the resulting
// P2 and A values differ between emulators. Skip the affected
// register for these opcodes.
static bool is_expander_read(uint8_t opcode) {
    // MOVD A,P4-P7: 0x0C, 0x0D, 0x0E, 0x0F
    return (opcode & 0xFC) == 0x0C;
}
static bool is_expander_write(uint8_t opcode) {
    // MOVD P4-P7,A: 0x3C-0x3F
    // ORLD P4-P7,A: 0x8C-0x8F
    // ANLD P4-P7,A: 0x9C-0x9F
    return (opcode & 0xFC) == 0x3C ||
           (opcode & 0xFC) == 0x8C ||
           (opcode & 0xFC) == 0x9C;
}

// --- Main ---

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_i8035 <test.json> [test2.json ...]\n");
        return 1;
    }

    init_mame_cpu();

    int total_tests = 0, total_passed = 0, total_failed = 0;
    std::vector<Failure> failures;

    for (int fi = 1; fi < argc; fi++) {
        const char *path = argv[fi];

        std::ifstream f(path);
        if (!f.is_open()) {
            fprintf(stderr, "Error: cannot open %s\n", path);
            return 1;
        }

        json tests = json::parse(f);
        int file_passed = 0, file_failed = 0;

        for (auto &tc : tests) {
            total_tests++;
            std::string name = tc["name"].get<std::string>();
            bool passed = true;
            std::string first_error;

            auto check = [&](const char *rname, unsigned got, unsigned expected) {
                if (got != expected && passed) {
                    passed = false;
                    char buf[256];
                    snprintf(buf, sizeof(buf), "%s expected=%u got=%u",
                             rname, expected, got);
                    first_error = buf;
                }
            };

            // --- Clear memory ---
            memset(mcs48_program, 0, sizeof(mcs48_program));
            memset(mcs48_data, 0, sizeof(mcs48_data));
            memset(mcs48_io, 0xFF, sizeof(mcs48_io));

            // --- Reset CPU ---
            reset_mame_cpu();

            // --- Load initial state ---
            auto &init = tc["initial"];

            // Program ROM
            for (auto &entry : init["ram"])
                mcs48_program[entry[0].get<uint16_t>() & 0xFFF] =
                    entry[1].get<uint8_t>();

            // Internal RAM (AS_DATA)
            for (auto &entry : init["internal_ram"])
                mcs48_data[entry[0].get<uint8_t>() & 0xFF] =
                    entry[1].get<uint8_t>();

            // Port I/O initial values
            mcs48_io[MCS48_PORT_P1]  = init["p1"].get<uint8_t>();
            mcs48_io[MCS48_PORT_P2]  = init["p2"].get<uint8_t>();
            mcs48_io[MCS48_PORT_BUS] = init["dbbb"].get<uint8_t>();

            // CPU registers (direct struct access)
            g_state.pc  = init["pc"].get<uint16_t>() & 0xFFF;
            g_state.a   = init["a"].get<uint8_t>();
            g_state.psw = init["psw"].get<uint8_t>();
            g_state.p1  = init["p1"].get<uint8_t>();
            g_state.p2  = init["p2"].get<uint8_t>();

            // F1 flag is stored in sts bit 3 (STS_F1 = 0x08)
            g_state.sts = init["f1"].get<bool>() ? STS_F1 : 0;

            // Timer
            g_state.timer     = init["t"].get<uint8_t>();
            g_state.prescaler = 0;

            // A11 bank select (0x000 or 0x800)
            g_state.a11 = init["a11"].get<bool>() ? 0x800 : 0x000;

            // Timer/counter control
            UINT8 tc_enabled = 0;
            if (init["timer_enabled"].get<bool>()) tc_enabled |= TIMER_ENABLED;
            if (init["counter_enabled"].get<bool>()) tc_enabled |= COUNTER_ENABLED;
            g_state.timecount_enabled = tc_enabled;

            // timer_flag = JTF-visible overflow flag
            g_state.timer_flag = init["timer_overflow"].get<bool>() ? TRUE : FALSE;

            // Interrupt state
            g_state.xirq_enabled = init["int_enabled"].get<bool>() ? TRUE : FALSE;
            g_state.tirq_enabled = init["tcnti_enabled"].get<bool>() ? TRUE : FALSE;
            g_state.irq_in_progress = init["in_interrupt"].get<bool>() ? TRUE : FALSE;

            // Prevent interrupts from firing during single-step
            g_state.irq_state = 0;
            g_state.timer_overflow = FALSE;
            g_state.t1_history = 0;

            // A11 pre-latch workaround for JMP/CALL
            uint8_t opcode = mcs48_program[g_state.pc & 0xFFF];
            if (is_jmp_call(opcode)) {
                g_state.a11 = init["a11_pending"].get<bool>() ? 0x800 : 0x000;
            }

            // --- Execute one instruction ---
            int cycles = execute_one();

            // --- Compare final state ---
            auto &fin = tc["final"];

            check("pc",  g_state.pc & 0xFFF, fin["pc"].get<uint16_t>());

            // A — skip for expander read (MOVD A,Px) since no 8243 connected
            if (!is_expander_read(opcode))
                check("a", g_state.a, fin["a"].get<uint8_t>());

            // PSW bit 3 is always 1 on real hardware; mask it
            check("psw", (unsigned)(g_state.psw & 0xF7),
                  (unsigned)(fin["psw"].get<uint8_t>() & 0xF7));

            // F1 flag
            check("f1", (g_state.sts & STS_F1) ? 1u : 0u,
                  fin["f1"].get<bool>() ? 1u : 0u);

            // Timer
            check("t", g_state.timer, fin["t"].get<uint8_t>());

            // Ports — skip P2 for expander write ops (8243 protocol modifies P2)
            check("p1",   (unsigned)mcs48_io[MCS48_PORT_P1],
                  fin["p1"].get<uint8_t>());
            if (!is_expander_write(opcode) && !is_expander_read(opcode))
                check("p2", (unsigned)mcs48_io[MCS48_PORT_P2],
                      fin["p2"].get<uint8_t>());
            check("dbbb", (unsigned)mcs48_io[MCS48_PORT_BUS],
                  fin["dbbb"].get<uint8_t>());

            // A11 — skip for SEL MB0/MB1 (immediate vs deferred)
            if (!is_sel_mb(opcode)) {
                check("a11", g_state.a11 ? 1u : 0u,
                      fin["a11"].get<bool>() ? 1u : 0u);
            }

            // Timer/counter control flags
            check("timer_enabled",
                  (g_state.timecount_enabled & TIMER_ENABLED) ? 1u : 0u,
                  fin["timer_enabled"].get<bool>() ? 1u : 0u);
            check("counter_enabled",
                  (g_state.timecount_enabled & COUNTER_ENABLED) ? 1u : 0u,
                  fin["counter_enabled"].get<bool>() ? 1u : 0u);

            // timer_flag = JTF-visible overflow flag
            check("timer_overflow", (unsigned)g_state.timer_flag,
                  fin["timer_overflow"].get<bool>() ? 1u : 0u);

            // Interrupt flags
            check("int_enabled", (unsigned)g_state.xirq_enabled,
                  fin["int_enabled"].get<bool>() ? 1u : 0u);
            check("tcnti_enabled", (unsigned)g_state.tirq_enabled,
                  fin["tcnti_enabled"].get<bool>() ? 1u : 0u);
            check("in_interrupt", (unsigned)g_state.irq_in_progress,
                  fin["in_interrupt"].get<bool>() ? 1u : 0u);

            // Internal RAM
            for (auto &entry : fin["internal_ram"]) {
                uint8_t addr = entry[0].get<uint8_t>();
                uint8_t expected = entry[1].get<uint8_t>();
                uint8_t got = mcs48_data[addr & 0xFF];
                char rn[32];
                snprintf(rn, sizeof(rn), "iRAM[0x%02X]", addr);
                check(rn, got, expected);
            }

            // Cycle count
            size_t expected_cycles = tc["cycles"].size();
            check("cycles", (unsigned)cycles, (unsigned)expected_cycles);

            if (passed) { file_passed++; total_passed++; }
            else {
                file_failed++; total_failed++;
                failures.push_back({name, first_error});
            }
        }

        printf("%s: %d passed, %d failed (of %zu)\n",
               path, file_passed, file_failed, tests.size());
    }

    printf("\n=== Summary ===\n");
    printf("Total: %d tests, %d passed, %d failed\n",
           total_tests, total_passed, total_failed);

    if (!failures.empty()) {
        std::map<std::string, int> tallies;
        std::map<std::string, std::string> first_errors;
        for (auto &f : failures) {
            std::string op = f.test_name.substr(0, 2);
            tallies[op]++;
            if (first_errors.find(op) == first_errors.end())
                first_errors[op] = f.detail;
        }
        printf("\nFailures by opcode (%zu unique):\n", tallies.size());
        for (auto &[op, count] : tallies)
            printf("  0x%s: %d failures  [%s]\n",
                   op.c_str(), count, first_errors[op].c_str());
    }

    return total_failed > 0 ? 1 : 0;
}
