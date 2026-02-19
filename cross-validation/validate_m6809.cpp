// Cross-validation harness for phosphor-core M6809 CPU
// Links elmerucr/MC6809 as an independent reference emulator
// and validates against SingleStepTests JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <string>
#include <vector>

#include "mc6809/src/mc6809.hpp"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

class FlatMemory6809 : public mc6809 {
public:
    mutable uint8_t memory[0x10000] = {};

    uint8_t read8(uint16_t addr) const override {
        return memory[addr];
    }

    void write8(uint16_t addr, uint8_t val) const override {
        memory[addr] = val;
    }
};

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate <test.json> [test2.json ...]\n");
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

            FlatMemory6809 cpu;

            // Load initial state
            auto &init = tc["initial"];
            cpu.set_pc(init["pc"].get<uint16_t>());
            cpu.set_sp(init["s"].get<uint16_t>());
            cpu.set_us(init["u"].get<uint16_t>());
            cpu.set_ac(init["a"].get<uint8_t>());
            cpu.set_br(init["b"].get<uint8_t>());
            cpu.set_dp(init["dp"].get<uint8_t>());
            cpu.set_xr(init["x"].get<uint16_t>());
            cpu.set_yr(init["y"].get<uint16_t>());
            cpu.set_cc(init["cc"].get<uint8_t>());

            for (auto &ram_entry : init["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t val = ram_entry[1].get<uint8_t>();
                cpu.memory[addr] = val;
            }

            // Execute one instruction
            uint16_t cycles = cpu.execute();

            // Check final state
            auto &fin = tc["final"];

            auto check_reg = [&](const char *reg_name, uint16_t got, uint16_t expected) {
                if (got != expected && passed) {
                    passed = false;
                    char buf[256];
                    snprintf(buf, sizeof(buf),
                             "%s expected=%u got=%u", reg_name, expected, got);
                    first_error = buf;
                }
            };

            check_reg("pc", cpu.get_pc(), fin["pc"].get<uint16_t>());
            check_reg("a",  cpu.get_ac(), fin["a"].get<uint8_t>());
            check_reg("b",  cpu.get_br(), fin["b"].get<uint8_t>());
            check_reg("dp", cpu.get_dp(), fin["dp"].get<uint8_t>());
            check_reg("x",  cpu.get_xr(), fin["x"].get<uint16_t>());
            check_reg("y",  cpu.get_yr(), fin["y"].get<uint16_t>());
            check_reg("u",  cpu.get_us(), fin["u"].get<uint16_t>());
            check_reg("s",  cpu.get_sp(), fin["s"].get<uint16_t>());
            check_reg("cc", cpu.get_cc(), fin["cc"].get<uint8_t>());

            // Check memory
            for (auto &ram_entry : fin["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t expected = ram_entry[1].get<uint8_t>();
                uint8_t got = cpu.memory[addr];
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
            if ((size_t)cycles != expected_cycles && passed) {
                passed = false;
                char buf[256];
                snprintf(buf, sizeof(buf),
                         "cycles expected=%zu got=%u", expected_cycles, cycles);
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
