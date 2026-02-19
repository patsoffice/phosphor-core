// Cross-validation harness for phosphor-core I8035 (MCS-48) CPU
// Links mame4all I8039 as an independent reference emulator
// and validates against phosphor-generated JSON test vectors.

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <set>
#include <string>
#include <vector>

#include "i8039/i8039.h"
#include "include/nlohmann/json.hpp"

using json = nlohmann::json;

// Flat 64KB program memory used by the mame4all I8039 shim
uint8_t i8039_program_memory[0x10000];

// Port I/O array: 0x000-0x0FF = external data memory (MOVX),
//                 0x100-0x1FF = ports (P1=0x101, P2=0x102, BUS=0x120, etc.)
uint8_t i8039_port_io[0x200];

// Replicate the I8039_Regs struct from i8039.cpp for get_context/set_context.
// Must be layout-compatible with the vendored source.
typedef struct {
    PAIR    PREPC;
    PAIR    PC;
    UINT8   A, SP, PSW;
    UINT8   RAM[128];
    UINT8   bus, f1;
    int     pending_irq, irq_executing, masterClock, regPtr;
    UINT8   t_flag, timer, timerON, countON, xirq_en, tirq_en;
    UINT16  A11, A11ff;
    int     irq_state;
    int     (*irq_callback)(int irqline);
} I8039_Regs_Copy;

// Port addresses used by mame4all
static const int PORT_P1  = 0x101;
static const int PORT_P2  = 0x102;
static const int PORT_BUS = 0x120;

// Opcodes excluded from cross-validation due to unfixable mame4all bugs.
static bool is_excluded_opcode(uint8_t op) {
    switch (op) {
        // ANLD Pp,A — mame4all reads M_RDMEM_OPCODE() instead of R.A (bug)
        case 0x9C: case 0x9D: case 0x9E: case 0x9F:
            return true;
        default:
            return false;
    }
}

// Opcodes where the a11 comparison should be skipped.
// SEL MB0/MB1: mame4all sets A11 immediately, phosphor defers to a11_pending.
static bool skip_a11_compare(uint8_t op) {
    return op == 0xE5 || op == 0xF5;
}

// Opcodes where the timer value comparison should be skipped.
// STRT T: mame4all uses a ÷32 prescaler, phosphor ticks T every cycle.
static bool skip_timer_compare(uint8_t op) {
    return op == 0x55;
}

// Parse a hex opcode from the test file stem (e.g., "a3" -> 0xA3)
static uint8_t parse_opcode_from_name(const std::string &name) {
    // The name field contains the hex bytes of the instruction.
    // The first two hex chars are the opcode.
    if (name.size() >= 2) {
        unsigned val = 0;
        if (sscanf(name.c_str(), "%02x", &val) == 1) {
            return (uint8_t)val;
        }
    }
    return 0;
}

struct Failure {
    std::string test_name;
    std::string detail;
};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: validate_i8035 <test.json> [test2.json ...]\n");
        return 1;
    }

    // Verify struct layout compatibility by checking size
    I8039_Regs_Copy layout_check;
    unsigned ctx_size = i8039_get_context(&layout_check);
    if (ctx_size != sizeof(I8039_Regs_Copy)) {
        fprintf(stderr, "Error: I8039_Regs size mismatch: expected %zu, got %u\n",
                sizeof(I8039_Regs_Copy), ctx_size);
        return 1;
    }

    int total_tests = 0;
    int total_passed = 0;
    int total_failed = 0;
    int total_skipped = 0;
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

        // Check if this entire file should be skipped based on the first test
        if (!tests.empty()) {
            std::string first_name = tests[0]["name"].get<std::string>();
            uint8_t opcode = parse_opcode_from_name(first_name);
            if (is_excluded_opcode(opcode)) {
                printf("  Skipped (excluded opcode 0x%02X)\n", opcode);
                total_skipped += (int)tests.size();
                continue;
            }
        }

        int file_passed = 0;
        int file_failed = 0;

        for (auto &tc : tests) {
            total_tests++;
            std::string name = tc["name"].get<std::string>();
            bool passed = true;
            std::string first_error;

            // --- Clear memory and ports ---
            memset(i8039_program_memory, 0, sizeof(i8039_program_memory));
            memset(i8039_port_io, 0xFF, sizeof(i8039_port_io));

            // --- Load initial state ---
            auto &init = tc["initial"];

            // Load program memory from RAM entries
            for (auto &ram_entry : init["ram"]) {
                uint16_t addr = ram_entry[0].get<uint16_t>();
                uint8_t val = ram_entry[1].get<uint8_t>();
                i8039_program_memory[addr] = val;
            }

            // Initialize port I/O with latch values
            i8039_port_io[PORT_P1]  = init["p1"].get<uint8_t>();
            i8039_port_io[PORT_P2]  = init["p2"].get<uint8_t>();
            i8039_port_io[PORT_BUS] = init["dbbb"].get<uint8_t>();

            // Reset CPU (sets timerON=1 as Mario Bros. hack, we override below)
            i8039_reset(nullptr);

            // Get context struct to set internal state
            I8039_Regs_Copy regs;
            i8039_get_context(&regs);

            // Set registers
            regs.PC.w.l = init["pc"].get<uint16_t>();
            regs.PC.w.h = 0;
            regs.A = init["a"].get<uint8_t>();
            regs.PSW = init["psw"].get<uint8_t>();
            regs.f1 = init["f1"].get<bool>() ? 1 : 0;
            regs.timer = init["t"].get<uint8_t>();
            regs.bus = init["dbbb"].get<uint8_t>();

            // A11 / A11ff: stored as bool in test vectors, 0x800 or 0 in mame4all
            regs.A11 = init["a11"].get<bool>() ? 0x800 : 0;
            regs.A11ff = init["a11_pending"].get<bool>() ? 0x800 : 0;

            // Timer/counter state
            regs.timerON = init["timer_enabled"].get<bool>() ? 1 : 0;
            regs.countON = init["counter_enabled"].get<bool>() ? 1 : 0;
            regs.t_flag = init["timer_overflow"].get<bool>() ? 1 : 0;

            // Interrupt state
            regs.xirq_en = init["int_enabled"].get<bool>() ? 1 : 0;
            regs.tirq_en = init["tcnti_enabled"].get<bool>() ? 1 : 0;
            regs.irq_executing = init["in_interrupt"].get<bool>()
                ? I8039_EXT_INT : I8039_IGNORE_INT;

            // Clear pending IRQ and timer prescaler
            regs.pending_irq = I8039_IGNORE_INT;
            regs.masterClock = 0;
            regs.irq_state = CLEAR_LINE;
            regs.irq_callback = nullptr;
            regs.PREPC.d = 0;

            // Set regPtr based on BS flag (bit 4 of PSW)
            regs.regPtr = (regs.PSW & 0x10) ? 24 : 0;

            // Load internal RAM (64 bytes for I8035)
            for (auto &iram_entry : init["internal_ram"]) {
                uint8_t offset = iram_entry[0].get<uint8_t>();
                uint8_t val = iram_entry[1].get<uint8_t>();
                if (offset < 128) {
                    regs.RAM[offset] = val;
                }
            }

            // Apply context (also recalculates SP from PSW and regPtr from BS)
            i8039_set_context(&regs);

            // Phosphor latches a11_pending → a11 at JMP/CALL time, but
            // mame4all uses R.A11 directly. Pre-latch A11 for JMP/CALL
            // opcodes so mame4all sees the same effective bank.
            uint8_t opcode = i8039_program_memory[regs.PC.w.l & 0xFFFF];
            bool is_jmp  = (opcode & 0x1F) == 0x04; // JMP_n: x04,x24,...,xE4
            bool is_call = (opcode & 0x1F) == 0x14; // CALL_n: x14,x34,...,xF4
            if (is_jmp || is_call) {
                // Set A11 = A11ff so mame4all uses the pending bank
                I8039_Regs_Copy tmp;
                i8039_get_context(&tmp);
                tmp.A11 = tmp.A11ff;
                i8039_set_context(&tmp);
            }

            // --- Execute one instruction ---
            int cycles_consumed = i8039_execute(1);

            // --- Read final state ---
            I8039_Regs_Copy final_regs;
            i8039_get_context(&final_regs);

            // --- Compare final state ---
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

            check_reg("a", final_regs.A, fin["a"].get<uint8_t>());
            // I8035 has 12-bit PC; mame4all uses 16-bit counter without masking.
            // Page-crossing tolerance: when a 2-byte conditional jump starts at
            // page offset 0xFE, mame4all uses the post-fetch PC (next page) for
            // the jump target while phosphor uses the pre-fetch PC (current page).
            // This causes an expected ±256 difference in the final PC.
            {
                unsigned mame_pc = final_regs.PC.w.l & 0x0FFF;
                unsigned phos_pc = fin["pc"].get<uint16_t>();
                uint16_t init_pc = init["pc"].get<uint16_t>();
                bool page_cross = (init_pc & 0xFF) == 0xFE;
                unsigned diff12 = (mame_pc - phos_pc) & 0xFFF;
                bool is_page_diff = page_cross &&
                    (diff12 == 0x100 || diff12 == 0xF00);
                if (!is_page_diff) {
                    check_reg("pc", mame_pc, phos_pc);
                }
            }
            check_reg("psw", final_regs.PSW, fin["psw"].get<uint8_t>());
            check_reg("f1", final_regs.f1, fin["f1"].get<bool>() ? 1u : 0u);
            if (!skip_timer_compare(opcode)) {
                check_reg("t", final_regs.timer, fin["t"].get<uint8_t>());
            }

            // P1/P2/DBBB: compare against port I/O array (mame4all has no
            // internal port latches — all port writes go to cpu_writeport)
            check_reg("dbbb", (unsigned)i8039_port_io[PORT_BUS],
                      fin["dbbb"].get<uint8_t>());
            check_reg("p1", (unsigned)i8039_port_io[PORT_P1],
                      fin["p1"].get<uint8_t>());
            check_reg("p2", (unsigned)i8039_port_io[PORT_P2],
                      fin["p2"].get<uint8_t>());

            // A11 / A11ff
            if (!skip_a11_compare(opcode)) {
                check_reg("a11", final_regs.A11 ? 1u : 0u,
                          fin["a11"].get<bool>() ? 1u : 0u);
            }
            check_reg("a11_pending", final_regs.A11ff ? 1u : 0u,
                      fin["a11_pending"].get<bool>() ? 1u : 0u);

            // Timer/counter control flags
            check_reg("timer_enabled", (unsigned)final_regs.timerON,
                      fin["timer_enabled"].get<bool>() ? 1u : 0u);
            check_reg("counter_enabled", (unsigned)final_regs.countON,
                      fin["counter_enabled"].get<bool>() ? 1u : 0u);
            if (!skip_timer_compare(opcode)) {
                check_reg("timer_overflow", (unsigned)final_regs.t_flag,
                          fin["timer_overflow"].get<bool>() ? 1u : 0u);
            }

            // Interrupt flags
            check_reg("int_enabled", (unsigned)final_regs.xirq_en,
                      fin["int_enabled"].get<bool>() ? 1u : 0u);
            check_reg("tcnti_enabled", (unsigned)final_regs.tirq_en,
                      fin["tcnti_enabled"].get<bool>() ? 1u : 0u);
            check_reg("in_interrupt",
                      final_regs.irq_executing != I8039_IGNORE_INT ? 1u : 0u,
                      fin["in_interrupt"].get<bool>() ? 1u : 0u);

            // Internal RAM (64 bytes for I8035)
            for (auto &iram_entry : fin["internal_ram"]) {
                uint8_t offset = iram_entry[0].get<uint8_t>();
                uint8_t expected = iram_entry[1].get<uint8_t>();
                if (offset < 64) {
                    uint8_t got = final_regs.RAM[offset];
                    if (got != expected && passed) {
                        passed = false;
                        char buf[256];
                        snprintf(buf, sizeof(buf),
                                 "iRAM[0x%02X] expected=%u got=%u",
                                 offset, expected, got);
                        first_error = buf;
                    }
                }
            }

            // Cycle count
            size_t expected_cycles = tc["cycles"].size();
            if ((size_t)cycles_consumed != expected_cycles && passed) {
                passed = false;
                char buf[256];
                snprintf(buf, sizeof(buf),
                         "cycles expected=%zu got=%d",
                         expected_cycles, cycles_consumed);
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
    printf("Total: %d tests, %d passed, %d failed, %d skipped\n",
           total_tests, total_passed, total_failed, total_skipped);

    if (!failures.empty()) {
        // Tally failures by opcode (first 2 hex chars of test name)
        std::map<std::string, int> opcode_tallies;
        std::map<std::string, std::string> opcode_first_error;
        for (auto &f : failures) {
            std::string op = f.test_name.substr(0, 2);
            opcode_tallies[op]++;
            if (opcode_first_error.find(op) == opcode_first_error.end()) {
                opcode_first_error[op] = f.detail;
            }
        }
        printf("\nFailures by opcode (%zu unique):\n", opcode_tallies.size());
        for (auto &kv : opcode_tallies) {
            printf("  0x%s: %d failures  [%s]\n",
                   kv.first.c_str(), kv.second,
                   opcode_first_error[kv.first].c_str());
        }
    }

    return total_failed > 0 ? 1 : 0;
}
