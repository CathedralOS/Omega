# Proof preparation benchmark

The private Rust examples replay actual successful-obligation contexts so small,
flat and unused facts remain in the measurement. The public compiler interfaces
and portable proof format are unchanged. Run the following commands from the
checkout root under the host budget in `tools/docs/build_slot.md`. On Windows,
use the executable paths with `.exe`; all preparation and measurement commands
are implemented in Rust and need no PowerShell.

```text
mbx build --release -p omega-bench
mbx build --release -p terminal-psi-to-terminal-psi --example proof_preparation --example proof_preparation_timing
mbx build --release -p compiler --example route_probe
```

Retain each version's executables before changing its implementation. The
examples accept `<main.omg> [repeats]` for a nongeneric source without external
imports. They parse, resolve, type and lower the program, discharge it normally,
and reconstruct the full ordered assumption/axiom rosters from successful
certificates, including unused premises. Source setup and independent kernel
checks occur outside the timers. Each replayed proof must equal that build's
production proof. Compare context shapes, proof fingerprints and complete
kernel acceptance fingerprints against the frozen baseline as well.

`proof_preparation` prints allocation diagnostics and never reads stdin:

```text
target/release/examples/proof_preparation tests/fixtures/run/ownership/borrowed_root_replacement/main.omg 100
```

`CONTEXT` records give conjunction-expansion geometry, facts, equations,
retained proof-node occurrences and the selected proof. Debug rendering counts
literal `ProofNode {` records outside measurement; compact origins contain none.
Retained bytes are live requested payload for one `Known`, excluding fixed
inputs and allocator metadata. Summing them is not simultaneous stage retention.
`MEASURE` records separately cover preparation and complete proof search.
Allocation/reallocation counters affect those diagnostic times.

`proof_preparation_timing` compiles allocator counters out with a constant
generic parameter and forwards directly to System. Its third argument,
`prepare` or `prove`, emits one `omega-timed-work/1` result. `prove` includes
preparation and search, so deferred construction costs remain in its timer;
independent kernel verification remains outside. The existing `omega-bench`
runner owns warmups, alternating pairs, child EOF, resource capture, source and
binary identities, oracle validation and statistics.

Portable plan creation and execution use the existing runner:

```text
target/release/omega-bench proof-plan prepare BEFORE/proof_preparation_timing AFTER/proof_preparation_timing tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/prepare.json --repeats 200000 --work-units 400000 --checksum 2800000
target/release/omega-bench compare build/performance/prepare.json build/performance/prepare-paired
target/release/omega-bench proof-plan prove BEFORE/proof_preparation_timing AFTER/proof_preparation_timing tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/prove.json --repeats 200000 --work-units 400000 --checksum 400000
target/release/omega-bench compare build/performance/prove.json build/performance/prove-paired
```

Replace `BEFORE` and `AFTER` with retained executable directories. Work units
count obligation contexts across repeated full source passes. Freeze counts and
checksums from the chosen baseline: the example above has two contexts/pass,
14 prepared facts-plus-equations/pass, and two successful proofs/pass. Recompute
these values if a compiler change alters the source's obligation roster. Plans
and result directories must be new; `--pairs` defaults to eight.

The separate source/Product process control requires an explicit hosted target:

```text
target/release/omega-bench proof-plan source BEFORE/route_probe AFTER/route_probe tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/source.json --target linux_x86_64
target/release/omega-bench compare build/performance/source.json build/performance/source-paired
```

Choose `linux_x86_64`, `linux_arm64`, `macos_arm64` or `windows_x86_64`.
Source mode sets `OMEGA_LIBRARY_ROOT` to this checkout's `source/library`,
records it as an input, and requires the exact single accepted part/final
accepted verdict without warnings. It bypasses Build and native publication;
prepare/prove are target-neutral and reject `--target`.

The exact pinned square-sum source is reconstructed and hash-checked in Rust:

```text
target/release/omega-bench proof-square build/performance/proof-square
```

It reads Git commit `9b4898865ecd5789bea4def6c2da078192a4ab82` locally, retains
the algorithm/constants/checksum, and adds the exact measured literal
qualifications. A shallow checkout missing that commit must fetch it first.
The default Build retains six optional optimization selections;
`--identity-build` produces the separately pinned identity Build. Both source
hashes are verified before the new directory is written. `wide64` and `nested8`
are bounded secondary conjunction controls accepted by either private example;
they are not representative source distributions.

Keep raw records under ignored `build/performance/`. The Windows observations
made with the retired local scripts remain archived there; the portable runner
supplies new platform resource observations. The permanent report at
`wiki/drafts/measurements/proof_preparation_2026_11_14.md` binds historical runs
to their recorded sources and producer identities.
