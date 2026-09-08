# Third-party notices

Phosphor is MIT licensed (see `LICENSE`). It also carries material derived from
the projects below, each under its own notice. Where a notice requires that it
travel with the work, it is reproduced in full here rather than at the head of
every file it touches.

## MartyPC

<https://github.com/dbalsom/martypc>

The bus interface unit and the microcode sequencing of the 8088 core in
`core/src/cpu/i8088/` are derived from MartyPC's `cpu_808x` module: the bus and
address state machines and their transitions, the prefetch queue with its
preload, and the per-instruction microcode routines and their clock counts.

The 8088 test vectors under `cpu-validation/test_data/8088/` were produced by
the same author with the Arduino8088 interface
(<https://github.com/dbalsom/arduino_8088>) and MartyPC, and are vendored with
their own README.

    MartyPC
    https://github.com/dbalsom/martypc

    Copyright 2022-2026 Daniel Balsom

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
