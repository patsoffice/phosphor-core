CXX ?= c++
CXXFLAGS = -std=c++17 -O2 -Wall -Wextra -I.
BINDIR = bin

# Suppress warnings from mame4all code
MAME4ALL_WARN = -Wno-unused-parameter -Wno-unused-but-set-variable \
                -Wno-unused-variable -Wno-unused-function -Wno-deprecated-declarations

# mame4all submodule paths
MAME4ALL = mame4all/examples/mame4all/src

# elmerucr/MC6809 submodule paths
MC6809_SRC = mc6809/src/mc6809.cpp \
             mc6809/src/mc6809_addressing_modes.cpp \
             mc6809/src/mc6809_instructions.cpp

all: $(BINDIR)/validate_m6809 $(BINDIR)/validate_m6809_mame \
     $(BINDIR)/validate_m6800 $(BINDIR)/validate_i8035

$(BINDIR):
	mkdir -p $(BINDIR)

$(BINDIR)/validate_m6809: validate_m6809.cpp $(MC6809_SRC) include/nlohmann/json.hpp mc6809/src/mc6809.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) -o $@ validate_m6809.cpp $(MC6809_SRC)

$(BINDIR)/validate_m6809_mame: validate_m6809_mame.cpp $(MAME4ALL)/cpu/m6809/m6809.cpp \
                                $(MAME4ALL)/cpu/m6809/m6809.h $(MAME4ALL)/cpu/m6809/6809ops.h \
                                m6809/osd_cpu.h m6809/memory.h m6809/cpuintrf.h m6809/state.h \
                                include/nlohmann/json.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) $(MAME4ALL_WARN) -Im6809 -o $@ \
		validate_m6809_mame.cpp $(MAME4ALL)/cpu/m6809/m6809.cpp

$(BINDIR)/validate_m6800: validate_m6800.cpp $(MAME4ALL)/cpu/m6800/m6800.cpp \
                          $(MAME4ALL)/cpu/m6800/m6800.h $(MAME4ALL)/cpu/m6800/6800ops.cpp \
                          $(MAME4ALL)/cpu/m6800/6800tbl.cpp \
                          m6800/osd_cpu.h m6800/memory.h m6800/cpuintrf.h m6800/state.h m6800/osdepend.h \
                          include/nlohmann/json.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) $(MAME4ALL_WARN) -Im6800 -o $@ \
		validate_m6800.cpp $(MAME4ALL)/cpu/m6800/m6800.cpp

$(BINDIR)/validate_i8035: validate_i8035.cpp $(MAME4ALL)/cpu/i8039/i8039.cpp \
                          $(MAME4ALL)/cpu/i8039/i8039.h \
                          i8039/osd_cpu.h i8039/memory.h i8039/cpuintrf.h \
                          include/nlohmann/json.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) $(MAME4ALL_WARN) -Ii8039 -o $@ \
		validate_i8035.cpp $(MAME4ALL)/cpu/i8039/i8039.cpp

clean:
	rm -rf $(BINDIR)

.PHONY: clean all
