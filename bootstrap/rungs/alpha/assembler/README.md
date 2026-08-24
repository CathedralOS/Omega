# `bootstrap/rungs/alpha/assembler/` — the assembler (an Alpha tool)

This folder holds the **assembler**: it turns `.alpha` assembly text into a tape the Alpha
seed runs. It is **written in Alpha** (`assembler.alpha`), so it belongs to the Alpha tier.
The compatibility path and committed executable retain historical `beta` names;
canonical gates call this role `alpha-assembler`.
(The **Beta language** — the first structured language, one tier up — lives in
`../../beta/` and is compiled by `bc`.) The assembler reads human mnemonics directly
(opcode names, `rN` registers, decimal immediates, labels, commas as whitespace), with no
Rust and no numeric-opcode step in normal use.

- `beta_x64_windows.exe` — the historically named Alpha seed with the assembler
  tape stamped into its hole. This is the working assembler executable.
- `assembler.alpha` — the assembler source, written in Alpha.
- `build.sh` — `./build.sh PROGRAM.alpha` → `build/PROGRAM.exe`: assembles the program with
  `beta_x64_windows.exe` and memcpy's the bytecode into a fresh copy of the alpha seed.
- `selfhost.sh` — the Alpha assembler assembles its own source; the result is
  byte-identical to `beta_x64_windows.exe`. That fixed point establishes
  deterministic dependency closure, not correctness.
- `examples/` — small alpha programs to build and run.
- `asm_ref.py` — an untrusted reference assembler in Python, written from the
  encoding rather than ported from `assembler.alpha`. `asm-diamond.sh` compares
  outputs over a corpus and is useful for catching encoder bugs. Agreement is
  diagnostic evidence, not source-to-artifact authority.
- `../../../onramps/alpha-assembler-rust/` — a throwaway Rust on-ramp, used
  **only for a cold start**: minting the
  very first `beta_x64_windows.exe` when no assembler exists yet. Normal use never
  touches it; the Alpha assembler rebuilds itself (see `selfhost.sh`).

```
./selfhost.sh                                                  # Alpha assembler rebuilds itself, no Rust
./build.sh examples/multiply.alpha && ./build/multiply.exe       # exits 42
./build.sh examples/echo.alpha     && echo hi | ./build/echo.exe  # echoes
```
