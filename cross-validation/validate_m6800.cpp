// Cross-validation harness for phosphor-core M6800 CPU
// Links mame4all M6800 as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <string>
#include <vector>

#include "m6800/m6800.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat 64KB memory used by the mame4all M6800 shim
uint8_t m6800_flat_memory[0x10000];

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_m6800 <test.json> [test2.json ...]\n");
        return 1;
    }

    // Initialize CPU once
    m6800_reset(nullptr);

    int total_tests = 0;
    int total_passed = 0;
    int total_failed = 0;
    std::vector<Failure> failures;

    for (int file_idx = 1; file_idx < argc; file_idx++) {
        const char *path = argv[file_idx];
        printf("Loading %s...\n", path);

        std::ifstream f(path);
        if (!f.is_open()) {
            fprintf(stderr, "Error: cannot open %s\n", path);
            return 1;
        }

        json tests = json::parse(f);
        printf("  %zu test cases\n", tests.size());

        int file_passed = 0;
        int file_failed = 0;

        for (auto &tc : tests) {
            total_tests++;
            std::string name = tc["name"].get<std::string>();
            bool passed = true;
            std::string first_error;

            // Reset CPU fully and clear memory to avoid stale state
            memset(m6800_flat_memory, 0, sizeof(m6800_flat_memory));

            // Load initial state
            auto &init = tc["initial"];

            // Load RAM first (includes instruction bytes)
            for (auto &ram_entry : init["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t val = ram_entry[1].get<uint8_t>();
                m6800_flat_memory[addr] = val;
            }

            // Reset CPU: clears wai_state, irq_state, extra_cycles, sets
            // insn/cycles tables. PC will be loaded from 0xFFFE (which is
            // whatever is in memory), but we override it below.
            m6800_reset(nullptr);

            // Set registers via mame4all API (overrides reset vector PC)
            m6800_set_reg(M6800_PC, init["pc"].get<uint16_t>());
            m6800_set_reg(M6800_S, init["sp"].get<uint16_t>());
            m6800_set_reg(M6800_A, init["a"].get<uint8_t>());
            m6800_set_reg(M6800_B, init["b"].get<uint8_t>());
            m6800_set_reg(M6800_X, init["x"].get<uint16_t>());
            m6800_set_reg(M6800_CC, init["cc"].get<uint8_t>());

            // Execute exactly one instruction: budget of 1 cycle ensures the
            // do-while loop exits after one instruction (min 2 cycles),
            // returning actual cycles consumed as (1 - m6800_ICount).
            int cycles_consumed = m6800_execute(1);

            // Check final state
            auto &fin = tc["final"];

            auto check_reg = [&](const char *reg_name, unsigned got, unsigned expected) {
                if (got != expected && passed) {
                    passed = false;
                    char buf[256];
                    snprintf(buf, sizeof(buf),
                             "%s expected=%u got=%u", reg_name, expected, got);
                    first_error = buf;
                }
            };

            check_reg("pc", m6800_get_reg(M6800_PC), fin["pc"].get<uint16_t>());
            check_reg("a", m6800_get_reg(M6800_A), fin["a"].get<uint8_t>());
            check_reg("b", m6800_get_reg(M6800_B), fin["b"].get<uint8_t>());
            check_reg("x", m6800_get_reg(M6800_X), fin["x"].get<uint16_t>());
            check_reg("sp", m6800_get_reg(M6800_S), fin["sp"].get<uint16_t>());

            // Compare CC with mask 0x3F (bits 6-7 are undefined on real 6800)
            unsigned cc_got = m6800_get_reg(M6800_CC) & 0x3F;
            unsigned cc_exp = fin["cc"].get<uint8_t>() & 0x3F;
            check_reg("cc", cc_got, cc_exp);

            // Check memory
            for (auto &ram_entry : fin["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t expected = ram_entry[1].get<uint8_t>();
                uint8_t got = m6800_flat_memory[addr];
                if (got != expected && passed) {
                    passed = false;
                    char buf[256];
                    snprintf(buf, sizeof(buf),
                             "RAM[0x%04X] expected=%u got=%u", addr, expected, got);
                    first_error = buf;
                }
            }

            // Check cycle count
            size_t expected_cycles = tc["cycles"].size();
            if ((size_t)cycles_consumed != expected_cycles && passed) {
                passed = false;
                char buf[256];
                snprintf(buf, sizeof(buf),
                         "cycles expected=%zu got=%d", expected_cycles, cycles_consumed);
                first_error = buf;
            }

            if (passed) {
                file_passed++;
                total_passed++;
            } else {
                file_failed++;
                total_failed++;
                failures.push_back({name, first_error});
            }
        }

        printf("  Results: %d passed, %d failed\n", file_passed, file_failed);
        if (file_failed > 0 && !failures.empty()) {
            printf("  First error: %s\n", failures.back().detail.c_str());
        }
    }

    // Summary
    printf("\n=== Summary ===\n");
    printf("Total: %d tests, %d passed, %d failed\n",
           total_tests, total_passed, total_failed);

    if (!failures.empty()) {
        printf("\nAll %zu failures:\n", failures.size());
        for (size_t i = 0; i < failures.size(); i++) {
            printf("  FAIL %s: %s\n",
                   failures[i].test_name.c_str(),
                   failures[i].detail.c_str());
        }
    }

    return total_failed > 0 ? 1 : 0;
}
