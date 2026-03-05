// Cross-validation harness for phosphor-core M6800 CPU
// Links MAME 0.148 m6800.c as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <string>
#include <vector>

// Our shim emu.h (found via -Im6800_0148 include path)
#include "m6800_0148/emu.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat 64KB memory used by the shim's address_space
uint8_t m6800_program[0x10000];

// Memory routing for mame0148_shim.h address_space
static UINT8 m6800_read(int /*space_id*/, offs_t addr) {
    return m6800_program[addr & 0xFFFF];
}
static void m6800_write(int /*space_id*/, offs_t addr, UINT8 val) {
    m6800_program[addr & 0xFFFF] = val;
}
shim_read_fn  shim_mem_read  = m6800_read;
shim_write_fn shim_mem_write = m6800_write;

// Active device pointer for address_space::device()
static legacy_cpu_device g_device;
legacy_cpu_device *shim_active_device = &g_device;

// attotime static constants
const attotime attotime::never = attotime{};
const attotime attotime::zero  = attotime{};

// Include m6800.c directly so its static functions are accessible.
#include "mame0148/src/emu/cpu/m6800/m6800.c"

// --- Harness state ---
static m6800_state g_state;

static int irq_callback_stub(device_t *, int) { return 0; }

static void init_mame_cpu() {
    memset(&g_state, 0, sizeof(g_state));
    g_device.set_token(&g_state);
    cpu_init_m6800(&g_device, irq_callback_stub);
}

static void reset_mame_cpu() {
    cpu_reset_m6800(&g_device);
}

// Execute one instruction. Returns cycles consumed.
static int execute_one() {
    g_state.icount = 1;
    cpu_execute_m6800(&g_device);
    return 1 - g_state.icount;
}

// --- Main ---

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_m6800 <test.json> [test2.json ...]\n");
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
            memset(m6800_program, 0, sizeof(m6800_program));

            // --- Reset CPU ---
            reset_mame_cpu();

            // --- Load initial state ---
            auto &init = tc["initial"];

            // Load RAM (includes instruction bytes)
            for (auto &entry : init["ram"]) {
                uint16_t addr = entry[0].get<uint16_t>();
                uint8_t val = entry[1].get<uint8_t>();
                m6800_program[addr] = val;
            }

            // CPU registers (direct struct access via PAIR union)
            g_state.pc.w.l = init["pc"].get<uint16_t>();
            g_state.pc.w.h = 0;
            g_state.s.w.l  = init["sp"].get<uint16_t>();
            g_state.s.w.h  = 0;
            g_state.d.b.h  = init["a"].get<uint8_t>();   // A = high byte of D
            g_state.d.b.l  = init["b"].get<uint8_t>();   // B = low byte of D
            g_state.x.w.l  = init["x"].get<uint16_t>();
            g_state.x.w.h  = 0;
            g_state.cc     = init["cc"].get<uint8_t>();

            // Clear interrupt/WAI state for clean single-step
            g_state.wai_state = 0;
            g_state.nmi_state = 0;
            g_state.nmi_pending = 0;
            g_state.irq_state[0] = CLEAR_LINE;
            g_state.irq_state[1] = CLEAR_LINE;
            g_state.irq_state[2] = CLEAR_LINE;

            // --- Execute one instruction ---
            int cycles = execute_one();

            // --- Compare final state ---
            auto &fin = tc["final"];

            check("pc", g_state.pc.w.l, fin["pc"].get<uint16_t>());
            check("a",  g_state.d.b.h,  fin["a"].get<uint8_t>());
            check("b",  g_state.d.b.l,  fin["b"].get<uint8_t>());
            check("x",  g_state.x.w.l,  fin["x"].get<uint16_t>());
            check("sp", g_state.s.w.l,  fin["sp"].get<uint16_t>());

            // CC bits 6-7 are undefined on real M6800
            unsigned cc_got = g_state.cc & 0x3F;
            unsigned cc_exp = fin["cc"].get<uint8_t>() & 0x3F;
            check("cc", cc_got, cc_exp);

            // Memory
            for (auto &entry : fin["ram"]) {
                uint16_t addr = entry[0].get<uint16_t>();
                uint8_t expected = entry[1].get<uint8_t>();
                uint8_t got = m6800_program[addr];
                char rn[32];
                snprintf(rn, sizeof(rn), "RAM[0x%04X]", addr);
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
