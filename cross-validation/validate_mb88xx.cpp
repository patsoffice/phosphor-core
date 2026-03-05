// Cross-validation harness for phosphor-core MB88XX (Fujitsu) CPU
// Links MAME 0.148 mb88xx.c as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <string>
#include <vector>

// Our shim emu.h (found via -Imb88xx include path)
#include "mb88xx/emu.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat memory arrays used by the emu.h shim's address_space stubs
uint8_t mb88_program[2048];
uint8_t mb88_data[128];
uint8_t mb88_io[8];

// Memory routing for mame0148_shim.h address_space
static UINT8 mb88_read(int space_id, offs_t addr) {
    switch (space_id) {
        case AS_DATA: return mb88_data[addr & 0x7F];
        case AS_IO:   return mb88_io[addr & 0x07];
        default:      return mb88_program[addr & 0x7FF];
    }
}
static void mb88_write(int space_id, offs_t addr, UINT8 val) {
    switch (space_id) {
        case AS_DATA: mb88_data[addr & 0x7F] = val; break;
        case AS_IO:   mb88_io[addr & 0x07] = val; break;
        default:      mb88_program[addr & 0x7FF] = val; break;
    }
}
shim_read_fn  shim_mem_read  = mb88_read;
shim_write_fn shim_mem_write = mb88_write;

// attotime static constants
const attotime attotime::never = attotime{};
const attotime attotime::zero  = attotime{};

// Include mb88xx.c directly so its static functions are accessible.
// The shim emu.h satisfies all MAME framework dependencies.
#include "mame0148/src/emu/cpu/mb88xx/mb88xx.c"

// --- Harness state ---

static legacy_cpu_device g_device;
legacy_cpu_device *shim_active_device = &g_device;
static mb88_state g_state;

static int irq_callback_stub(device_t *, int) { return 0; }

static void init_mame_cpu() {
    memset(&g_state, 0, sizeof(g_state));
    g_device.set_token(&g_state);
    cpu_init_mb88(&g_device, irq_callback_stub);
}

static void reset_mame_cpu() {
    cpu_reset_mb88(&g_device);
}

// Execute one instruction. Returns cycles consumed (1 or 2).
static int execute_one() {
    g_state.icount = 1;
    cpu_execute_mb88(&g_device);
    return 1 - g_state.icount;
}

// --- Main ---

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_mb88xx <test.json> [test2.json ...]\n");
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
            memset(mb88_program, 0, sizeof(mb88_program));
            memset(mb88_data, 0, sizeof(mb88_data));
            memset(mb88_io, 0, sizeof(mb88_io));

            // --- Reset CPU ---
            reset_mame_cpu();

            // --- Load initial state ---
            auto &init = tc["initial"];

            // Program ROM
            for (auto &entry : init["rom"])
                mb88_program[entry[0].get<uint16_t>() & 0x7FF] =
                    entry[1].get<uint8_t>();

            // Data RAM
            for (auto &entry : init["ram"])
                mb88_data[entry[0].get<uint8_t>() & 0x7F] =
                    entry[1].get<uint8_t>();

            // I/O ports
            for (auto &entry : init["io"])
                mb88_io[entry[0].get<uint8_t>() & 0x07] =
                    entry[1].get<uint8_t>();

            // CPU registers (direct struct access)
            g_state.PC  = init["pc"].get<uint8_t>() & 0x3F;
            g_state.PA  = init["pa"].get<uint8_t>() & 0x1F;
            g_state.A   = init["a"].get<uint8_t>()  & 0x0F;
            g_state.X   = init["x"].get<uint8_t>()  & 0x0F;
            g_state.Y   = init["y"].get<uint8_t>()  & 0x0F;
            g_state.SI  = init["si"].get<uint8_t>() & 0x03;
            g_state.st  = init["st"].get<uint8_t>() & 1;
            g_state.zf  = init["zf"].get<uint8_t>() & 1;
            g_state.cf  = init["cf"].get<uint8_t>() & 1;
            g_state.vf  = init["vf"].get<uint8_t>() & 1;
            g_state.sf  = init["sf"].get<uint8_t>() & 1;
            g_state.nf  = init["nf"].get<uint8_t>() & 1;
            g_state.pio = init["pio"].get<uint8_t>();
            g_state.TH  = init["th"].get<uint8_t>() & 0x0F;
            g_state.TL  = init["tl"].get<uint8_t>() & 0x0F;
            g_state.TP  = init["tp"].get<uint8_t>();
            g_state.SB  = init["sb"].get<uint8_t>() & 0x0F;
            g_state.pending_interrupt = 0;
            g_state.SBcount = 0;
            g_state.ctr = 0;

            // Stack
            for (int i = 0; i < 4; i++)
                g_state.SP[i] = init["stack"][i].get<uint16_t>();

            // --- Execute one instruction ---
            int cycles = execute_one();

            // --- Compare final state ---
            auto &fin = tc["final"];

            check("pc",  g_state.PC,  fin["pc"].get<uint8_t>());
            check("pa",  g_state.PA,  fin["pa"].get<uint8_t>());
            check("a",   g_state.A,   fin["a"].get<uint8_t>());
            check("x",   g_state.X,   fin["x"].get<uint8_t>());
            check("y",   g_state.Y,   fin["y"].get<uint8_t>());
            check("si",  g_state.SI,  fin["si"].get<uint8_t>());
            check("st",  g_state.st,  fin["st"].get<uint8_t>());
            check("zf",  g_state.zf,  fin["zf"].get<uint8_t>());
            check("cf",  g_state.cf,  fin["cf"].get<uint8_t>());
            check("vf",  g_state.vf,  fin["vf"].get<uint8_t>());
            check("sf",  g_state.sf,  fin["sf"].get<uint8_t>());
            check("pio", g_state.pio, fin["pio"].get<uint8_t>());
            check("th",  g_state.TH,  fin["th"].get<uint8_t>());
            check("tl",  g_state.TL,  fin["tl"].get<uint8_t>());
            check("sb",  g_state.SB,  fin["sb"].get<uint8_t>());

            // Stack
            for (int i = 0; i < 4; i++) {
                char sn[16];
                snprintf(sn, sizeof(sn), "sp[%d]", i);
                check(sn, g_state.SP[i], fin["stack"][i].get<uint16_t>());
            }

            // Data RAM
            for (auto &entry : fin["ram"]) {
                uint8_t addr = entry[0].get<uint8_t>();
                uint8_t expected = entry[1].get<uint8_t>();
                uint8_t got = mb88_data[addr & 0x7F];
                char rn[32];
                snprintf(rn, sizeof(rn), "RAM[0x%02X]", addr);
                check(rn, got, expected);
            }

            // Cycle count
            size_t expected_cycles = tc["cycles"].get<size_t>();
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
        // Tally failures by opcode (first 2 hex chars of test name)
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
