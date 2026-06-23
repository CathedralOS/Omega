# `compiler/beta/` — the assembler, written in alpha

Beta is the first rung above the seed: an assembler that turns `.alpha` text into a tape
the alpha seed runs. It is **written in alpha**, so its source is `.alpha`. It reads human
mnemonics directly (opcode names, `rN` registers, decimal immediates, labels, commas as
whitespace) — **no Rust and no numeric-opcode step** anywhere in normal use.

- `beta_x64_windows.exe` — beta itself: the alpha seed with the assembler tape stamped
  into its hole. This is the working compiler.
- `assembler.alpha` — beta's source, in alpha.
- `build.sh` — `./build.sh PROGRAM.alpha` → `build/PROGRAM.exe`: assembles the program with
  `beta_x64_windows.exe` and memcpy's the bytecode into a fresh copy of the alpha seed.
- `selfhost.sh` — beta assembles its own source; the result is byte-identical to
  `beta_x64_windows.exe`. That fixed point is the proof beta is self-hosting.
- `examples/` — small alpha programs to build and run.
- `../beta-rs/` — a throwaway Rust on-ramp, used **only for a cold start**: minting the
  very first `beta_x64_windows.exe` when no beta exists yet. Normal use never touches it;
  once beta exists it rebuilds itself (see `selfhost.sh`).

```
./selfhost.sh                                                  # beta rebuilds itself, no Rust
./build.sh examples/multiply.alpha && ./build/multiply.exe       # exits 42
./build.sh examples/echo.alpha     && echo hi | ./build/echo.exe  # echoes
```
