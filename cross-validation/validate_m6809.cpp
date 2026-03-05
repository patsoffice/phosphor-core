// Cross-validation harness for phosphor-core M6809 CPU
// Links MAME 0.148 m6809.c as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <string>
#include <vector>

// Our shim emu.h (found via -Im6809_0148 include path)
#include "m6809_0148/emu.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat memory array used by the emu.h shim's address_space stubs
uint8_t m6809_program[0x10000];

// Memory routing for mame0148_shim.h address_space
static UINT8 m6809_read(int, offs_t addr) {
    return m6809_program[addr & 0xFFFF];
}
static void m6809_write(int, offs_t addr, UINT8 val) {
    m6809_program[addr & 0xFFFF] = val;
}
shim_read_fn  shim_mem_read  = m6809_read;
shim_write_fn shim_mem_write = m6809_write;

// attotime static constants
const attotime attotime::never = attotime{};
const attotime attotime::zero  = attotime{};

// Include m6809.c directly so its class methods are compiled in this TU.
// The shim emu.h satisfies all MAME framework dependencies.
#include "mame0148/src/emu/cpu/m6809/m6809.c"

// Stub for the private disassemble method (never called in validation)
offs_t m6809_base_device::disassemble(char *buf, offs_t, const UINT8 *,
                                       const UINT8 *, int) {
    sprintf(buf, "???");
    return 1;
}

// --- Test subclass to expose protected members ---

class m6809_test_device : public m6809_base_device {
public:
    m6809_test_device(const machine_config &mc)
        : m6809_base_device(mc, "cpu", nullptr, 1000000, M6809, 1) {}

    void do_start() { device_start(); }
    void do_run()   { execute_run(); }

    // Register access
    void set_pc(uint16_t v) { m_pc.d = v; }
    void set_a(uint8_t v)   { m_d.b.h = v; }
    void set_b(uint8_t v)   { m_d.b.l = v; }
    void set_dp(uint8_t v)  { m_dp.d = 0; m_dp.b.h = v; }
    void set_x(uint16_t v)  { m_x.d = v; }
    void set_y(uint16_t v)  { m_y.d = v; }
    void set_u(uint16_t v)  { m_u.d = v; }
    void set_s(uint16_t v)  { m_s.d = v; }
    void set_cc(uint8_t v)  { m_cc = v; }

    uint16_t get_pc() const { return m_pc.w.l; }
    uint8_t  get_a()  const { return m_d.b.h; }
    uint8_t  get_b()  const { return m_d.b.l; }
    uint8_t  get_dp() const { return m_dp.b.h; }
    uint16_t get_x()  const { return m_x.w.l; }
    uint16_t get_y()  const { return m_y.w.l; }
    uint16_t get_u()  const { return m_u.w.l; }
    uint16_t get_s()  const { return m_s.w.l; }
    uint8_t  get_cc() const { return m_cc; }

    int &icount()       { return m_icount; }
    int &extra_cycles() { return m_extra_cycles; }

    void clear_irq_state() {
        m_int_state = 0;
        m_nmi_state = CLEAR_LINE;
        m_irq_state[0] = CLEAR_LINE;
        m_irq_state[1] = CLEAR_LINE;
        m_extra_cycles = 0;
    }
};

// --- Harness ---

static machine_config g_mconfig;
static m6809_test_device g_cpu(g_mconfig);

static void init_cpu() {
    g_cpu.do_start();
}

// Execute one instruction. Returns cycles consumed.
static int execute_one() {
    g_cpu.clear_irq_state();
    g_cpu.icount() = 1;
    g_cpu.do_run();
    return 1 - g_cpu.icount();
}

// --- Main ---

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_m6809 <test.json> [test2.json ...]\n");
        return 1;
    }

    init_cpu();

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
            memset(m6809_program, 0, sizeof(m6809_program));

            // --- Load initial state ---
            auto &init = tc["initial"];

            // RAM
            for (auto &entry : init["ram"])
                m6809_program[entry[0].get<uint16_t>()] =
                    entry[1].get<uint8_t>();

            // CPU registers
            g_cpu.set_pc(init["pc"].get<uint16_t>());
            g_cpu.set_a(init["a"].get<uint8_t>());
            g_cpu.set_b(init["b"].get<uint8_t>());
            g_cpu.set_dp(init["dp"].get<uint8_t>());
            g_cpu.set_x(init["x"].get<uint16_t>());
            g_cpu.set_y(init["y"].get<uint16_t>());
            g_cpu.set_u(init["u"].get<uint16_t>());
            g_cpu.set_s(init["s"].get<uint16_t>());
            g_cpu.set_cc(init["cc"].get<uint8_t>());

            // --- Execute one instruction ---
            int cycles = execute_one();

            // --- Compare final state ---
            auto &fin = tc["final"];

            check("pc", g_cpu.get_pc(), fin["pc"].get<uint16_t>());
            check("a",  g_cpu.get_a(),  fin["a"].get<uint8_t>());
            check("b",  g_cpu.get_b(),  fin["b"].get<uint8_t>());
            check("dp", g_cpu.get_dp(), fin["dp"].get<uint8_t>());
            check("x",  g_cpu.get_x(),  fin["x"].get<uint16_t>());
            check("y",  g_cpu.get_y(),  fin["y"].get<uint16_t>());
            check("u",  g_cpu.get_u(),  fin["u"].get<uint16_t>());
            check("s",  g_cpu.get_s(),  fin["s"].get<uint16_t>());
            check("cc", g_cpu.get_cc(), fin["cc"].get<uint8_t>());

            // Memory
            for (auto &entry : fin["ram"]) {
                uint16_t addr = entry[0].get<uint16_t>();
                uint8_t expected = entry[1].get<uint8_t>();
                uint8_t got = m6809_program[addr];
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
