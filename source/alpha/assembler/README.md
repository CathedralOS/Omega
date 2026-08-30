# `source/alpha/assembler/` — the assembler (an Alpha tool)

This folder holds the **assembler**: it turns `.alpha` assembly text into a tape the Alpha
seed runs. It is **written in Alpha** (`assembler.alpha`), so it belongs to the Alpha tier.
The compatibility path and committed executable retain historical `beta` names;
canonical gates call this role `alpha-assembler`.
(The **Beta language** — the first structured language, one tier up — lives in
`../../beta/` and is compiled by `bc`.) The authoritative grammar and encoding
are fixed by [`../ASSEMBLY.md`](../ASSEMBLY.md). The assembler reads human mnemonics directly
(opcode names, whole-token `rN` registers for decimal `N` in `0..255`, decimal
immediates, labels, commas as whitespace), with no
Rust and no numeric-opcode step in normal use.
Decimal immediates cover the complete unsigned 64-bit Alpha word range; the
encoder walks the stored word bytes directly and does not reinterpret high-bit
values through signed division.

- `beta_x64_windows.exe` — the historically named Alpha seed with the assembler
  tape stamped into its AlphaBootstrapV2 hole. This is the working assembler
  executable on Windows; `beta_arm64_macos` is the corresponding Darwin
  realization of the same tape.
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
  diagnostic evidence, not source-to-artifact authority. The pair is temporary
  development scaffolding and cannot survive completion of the checked direct
  assembly relation.
The historical Rust cold-start producer has been retired. The checked-in seed,
written assembly/VM semantics, and self-hosting reconstruction
are the maintained path.
Both committed assembler containers carry the same 6,816-byte raw tape in the
selected 256-MiB/one-MiB AlphaBootstrapV2 seed layout. Their native bytes differ;
`selfhost.sh` compares the extracted platform-independent tape.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `examples/` | `echo.alpha`, `factorial.alpha`, `fib.alpha`, `gcd.alpha`, and `multiply.alpha` are small source cases consumed by the assembler diamond and calling-convention documentation. | Delete a case when another retained case covers the same encoding boundary; delete the directory when generated discriminators fully replace it. |
| `.gitignore` | Excludes the disposable `build/` containers produced by the local builder. | Delete with `build.sh` or when build output moves outside this owner. |
| `assembler.alpha`, `beta_arm64_macos`, `beta_x64_windows.exe` | One authoritative Alpha assembler source and its two stamped platform realizations. | Replace only atomically with an exact reconstruction and both platform consumers. |
| `build.sh`, `selfhost.sh` | Disposable tape stamping and exact assembler reconstruction. | Delete `build.sh` when raw-tape execution replaces stamped local builds; delete `selfhost.sh` when a stronger exact construction gate subsumes it. |
| `asm_ref.py`, `asm-diamond.sh` | One independent assembly relation and bounded comparison. | Delete together when the checked source-to-tape relation fully subsumes their failure detection. |
| `register-label-regression.sh` | Closed lexical/operand/width discriminator against the fresh assembler and independent relation. | Delete when the checked assembly relation covers every retained case. |

The root currently retains one authoritative Alpha source, the two platform
assembler realizations, exact self-host/build entry points, one temporary
independent reference, and focused encoding regressions. Historical producer
routes are not retained; the Python reference leaves when its deletion
condition is met.

```
./selfhost.sh                                                  # Alpha assembler rebuilds itself, no Rust
./build.sh examples/multiply.alpha && ./build/multiply.exe       # exits 42
./build.sh examples/echo.alpha     && echo hi | ./build/echo.exe  # echoes
```
