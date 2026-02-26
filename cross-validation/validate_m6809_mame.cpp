// Cross-validation harness for phosphor-core M6809 CPU
// Links mame4all M6809 as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <string>
#include <vector>

#include "mame4all/examples/mame4all/src/cpu/m6809/m6809.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat 64KB memory used by the mame4all M6809 shim
uint8_t m6809_flat_memory[0x10000];

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_m6809_mame <test.json> [test2.json ...]\n");
        return 1;
    }

    // Initialize CPU once
    m6809_reset(nullptr);

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
            memset(m6809_flat_memory, 0, sizeof(m6809_flat_memory));

            // Load initial state
            auto &init = tc["initial"];

            // Load RAM first (includes instruction bytes)
            for (auto &ram_entry : init["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t val = ram_entry[1].get<uint8_t>();
                m6809_flat_memory[addr] = val;
            }

            // Reset CPU: clears interrupt state, loads PC from reset vector
            // at 0xFFFE. We override PC and all registers below.
            m6809_reset(nullptr);

            // Set registers via mame4all API
            m6809_set_reg(M6809_PC, init["pc"].get<uint16_t>());
            m6809_set_reg(M6809_S,  init["s"].get<uint16_t>());
            m6809_set_reg(M6809_U,  init["u"].get<uint16_t>());
            m6809_set_reg(M6809_A,  init["a"].get<uint8_t>());
            m6809_set_reg(M6809_B,  init["b"].get<uint8_t>());
            m6809_set_reg(M6809_DP, init["dp"].get<uint8_t>());
            m6809_set_reg(M6809_X,  init["x"].get<uint16_t>());
            m6809_set_reg(M6809_Y,  init["y"].get<uint16_t>());
            m6809_set_reg(M6809_CC, init["cc"].get<uint8_t>());

            // Execute exactly one instruction: budget of 1 cycle ensures the
            // loop exits after one instruction, returning actual cycles consumed.
            int cycles_consumed = m6809_execute(1);

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

            check_reg("pc", m6809_get_reg(M6809_PC), fin["pc"].get<uint16_t>());
            check_reg("a",  m6809_get_reg(M6809_A),  fin["a"].get<uint8_t>());
            check_reg("b",  m6809_get_reg(M6809_B),  fin["b"].get<uint8_t>());
            check_reg("dp", m6809_get_reg(M6809_DP), fin["dp"].get<uint8_t>());
            check_reg("x",  m6809_get_reg(M6809_X),  fin["x"].get<uint16_t>());
            check_reg("y",  m6809_get_reg(M6809_Y),  fin["y"].get<uint16_t>());
            check_reg("u",  m6809_get_reg(M6809_U),  fin["u"].get<uint16_t>());
            check_reg("s",  m6809_get_reg(M6809_S),  fin["s"].get<uint16_t>());
            check_reg("cc", m6809_get_reg(M6809_CC), fin["cc"].get<uint8_t>());

            // Check memory
            for (auto &ram_entry : fin["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t expected = ram_entry[1].get<uint8_t>();
                uint8_t got = m6809_flat_memory[addr];
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
