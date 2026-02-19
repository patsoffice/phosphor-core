CXX ?= c++
CXXFLAGS = -std=c++17 -O2 -Wall -Wextra -I.
BINDIR = bin

# Suppress warnings from vendored mame4all code
MAME4ALL_WARN = -Wno-unused-parameter -Wno-unused-but-set-variable \
                -Wno-unused-variable -Wno-unused-function -Wno-deprecated-declarations

MC6809_SRC = mc6809/src/mc6809.cpp \
             mc6809/src/mc6809_addressing_modes.cpp \
             mc6809/src/mc6809_instructions.cpp

all: $(BINDIR)/validate_m6809 $(BINDIR)/validate_m6800 $(BINDIR)/validate_i8035

$(BINDIR):
	mkdir -p $(BINDIR)

$(BINDIR)/validate_m6809: validate_m6809.cpp $(MC6809_SRC) include/nlohmann/json.hpp mc6809/src/mc6809.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) -o $@ validate_m6809.cpp $(MC6809_SRC)

$(BINDIR)/validate_m6800: validate_m6800.cpp m6800/m6800.cpp m6800/m6800.h m6800/mame_shim.h \
                          m6800/6800ops.cpp m6800/6800tbl.cpp include/nlohmann/json.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) $(MAME4ALL_WARN) -o $@ validate_m6800.cpp m6800/m6800.cpp

$(BINDIR)/validate_i8035: validate_i8035.cpp i8039/i8039.cpp i8039/i8039.h \
                          i8039/mame_shim.h include/nlohmann/json.hpp | $(BINDIR)
	$(CXX) $(CXXFLAGS) $(MAME4ALL_WARN) -Ii8039 -o $@ validate_i8035.cpp i8039/i8039.cpp

clean:
	rm -rf $(BINDIR)

.PHONY: clean all
