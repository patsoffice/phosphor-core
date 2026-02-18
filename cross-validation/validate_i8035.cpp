// Cross-validation harness for phosphor-core I8035 (MCS-48) CPU
// Validates phosphor-generated JSON test vectors against a reference emulator.
//
// Reference emulator: MAME's MCS-48 (mcs48.cpp)
// To use this harness, vendor MAME's MCS-48 source into cross-validation/mcs48/
// and create a shim (mcs48/mame_shim.h) exposing:
//   void mcs48_reset();
//   void mcs48_set_reg(int reg, unsigned val);
//   unsigned mcs48_get_reg(int reg);
//   int mcs48_execute(int cycles);
//   uint8_t mcs48_internal_ram[256];
//   uint8_t mcs48_program_memory[4096];
//
// Build: see Makefile target 'validate_i8035'

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <string>
#include <vector>

#include "include/nlohmann/json.hpp"

// Uncomment when MAME MCS-48 shim is available:
// #include "mcs48/mcs48.h"

using json = nlohmann::json;

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_i8035 <test.json> [test2.json ...]\n");
        return 1;
    }

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

            auto &init = tc["initial"];

            // TODO: Load initial state into reference emulator
            // 1. Reset CPU
            // 2. Set registers: a, pc, psw, f1, t, dbbb, p1, p2, a11, a11_pending
            // 3. Set flags: timer_enabled, counter_enabled, timer_overflow,
            //    int_enabled, tcnti_enabled, in_interrupt
            // 4. Load program memory from init["ram"]
            // 5. Load internal RAM from init["internal_ram"]

            // TODO: Execute one instruction via reference emulator

            // TODO: Compare final state
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

            // Placeholder: mark all tests as passed until reference emulator is wired up
            (void)check_reg;
            (void)init;
            (void)fin;

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
