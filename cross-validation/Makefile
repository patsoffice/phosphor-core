CXX ?= c++
CXXFLAGS = -std=c++17 -O2 -Wall -Wextra -I.

# Suppress warnings from vendored mame4all code
M6800_WARN = -Wno-unused-parameter -Wno-unused-but-set-variable \
             -Wno-unused-variable -Wno-unused-function -Wno-deprecated-declarations

MC6809_SRC = mc6809/src/mc6809.cpp \
             mc6809/src/mc6809_addressing_modes.cpp \
             mc6809/src/mc6809_instructions.cpp

SRCS = validate.cpp $(MC6809_SRC)

all: validate validate_m6800

validate: $(SRCS) include/nlohmann/json.hpp mc6809/src/mc6809.hpp
	$(CXX) $(CXXFLAGS) -o $@ validate.cpp $(MC6809_SRC)

validate_m6800: validate_m6800.cpp m6800/m6800.cpp m6800/m6800.h m6800/mame_shim.h \
                m6800/6800ops.cpp m6800/6800tbl.cpp include/nlohmann/json.hpp
	$(CXX) $(CXXFLAGS) $(M6800_WARN) -o $@ validate_m6800.cpp m6800/m6800.cpp

clean:
	rm -f validate validate_m6800

.PHONY: clean all
