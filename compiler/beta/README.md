# `compiler/beta/` — the assembler, written in alpha

Beta is the first rung above the seed: an assembler (the hex0-like step) that turns
`.alp` text into a tape the alpha seed runs. It is **written in alpha**, so its source
files are `.alp`. (A future rung written *in beta* would have `.bet` source.)

- `assembler.alp` — the assembler itself, in alpha.
- `build.sh` — `./build.sh PROGRAM.alp` → `build/PROGRAM.exe`: assembles the program to
  bytecode and memcpy's it into a copy of the alpha seed
  (`../alpha/alpha_x64_windows.exe`), producing a standalone exe.
- `examples/` — small alpha programs to build and run.
- `../beta-rs/` — a throwaway Rust on-ramp that mints the bytecode today; it goes away
  once beta assembles its own source on the alpha seed.

```
./build.sh examples/multiply.alp && ./build/multiply.exe        # exits 42
./build.sh examples/echo.alp     && echo hi | ./build/echo.exe   # echoes
```
