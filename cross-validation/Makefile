CXX ?= c++
CXXFLAGS = -std=c++17 -O2 -Wall -Wextra -I.

MC6809_SRC = mc6809/src/mc6809.cpp \
             mc6809/src/mc6809_addressing_modes.cpp \
             mc6809/src/mc6809_instructions.cpp

SRCS = validate.cpp $(MC6809_SRC)

validate: $(SRCS) include/nlohmann/json.hpp mc6809/src/mc6809.hpp
	$(CXX) $(CXXFLAGS) -o $@ validate.cpp $(MC6809_SRC)

clean:
	rm -f validate

.PHONY: clean
