# Proof preparation benchmark

This benchmark times proof preparation and search. Two private Rust examples
replay real successful-obligation contexts, so small, flat and unused facts stay
measured. Public compiler interfaces and the portable proof format stay
unchanged. Run these commands from the checkout root under the host budget in
`tools/docs/build_slot.md`. On Windows, add `.exe` to executable paths; all
commands are Rust and need no PowerShell.

```text
mbx build --release -p omega-bench
mbx build --release -p terminal-psi-to-terminal-psi --example proof_preparation --example proof_preparation_timing
mbx build --release -p compiler --example route_probe
```

Keep each version's executables before changing it. The examples take
`<main.omg> [repeats]` for a nongeneric source without external imports. They
parse, resolve, type and lower the program, prove it, and rebuild from the
successful certificates the full ordered assumption and axiom lists, including
unused premises. Source setup and independent kernel checks run outside the
timers. Each replayed proof must equal that build's production proof. Also
compare context shapes, proof fingerprints and complete kernel acceptance
fingerprints with the frozen baseline.

`proof_preparation` prints allocation diagnostics and never reads stdin:

```text
target/release/examples/proof_preparation tests/fixtures/run/ownership/borrowed_root_replacement/main.omg 100
```

`CONTEXT` records give the conjunction-expansion shape, facts, equations,
retained proof-node occurrences and the selected proof. Outside measurement,
debug output counts literal `ProofNode {` records; compact origins have none.
Retained bytes are live requested payload for one `Known`, excluding fixed
inputs and allocator metadata. Their sum is not what a stage holds at once.
`MEASURE` records time preparation and full proof search separately. The
allocation counters slow those diagnostic times.

`proof_preparation_timing` compiles the allocator counters out with a constant
generic parameter and calls System directly. Its third argument, `prepare` or
`prove`, emits one `omega-timed-work/1` result. `prove` covers preparation and
search, so deferred construction stays timed; independent kernel verification
stays outside. The `omega-bench` runner handles warmups, alternating pairs,
child EOF, resource capture, source and binary identities, oracle validation and
statistics.

The same runner creates and runs portable plans:

```text
target/release/omega-bench proof-plan prepare BEFORE/proof_preparation_timing AFTER/proof_preparation_timing tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/prepare.json --repeats 200000 --work-units 400000 --checksum 2800000
target/release/omega-bench compare build/performance/prepare.json build/performance/prepare-paired
target/release/omega-bench proof-plan prove BEFORE/proof_preparation_timing AFTER/proof_preparation_timing tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/prove.json --repeats 200000 --work-units 400000 --checksum 400000
target/release/omega-bench compare build/performance/prove.json build/performance/prove-paired
```

Replace `BEFORE` and `AFTER` with the kept executable directories. Work units
count obligation contexts across repeated full source passes. Freeze counts and
checksums from the chosen baseline: each pass of the example above has two
contexts, 14 prepared facts plus equations, and two successful proofs. Recompute
them if a compiler change alters the source's obligations. Plans and result
directories must be new; `--pairs` defaults to eight.

The source-through-product control needs an explicit hosted target:

```text
target/release/omega-bench proof-plan source BEFORE/route_probe AFTER/route_probe tests/fixtures/run/ownership/borrowed_root_replacement/main.omg build/performance/source.json --target linux_x86_64
target/release/omega-bench compare build/performance/source.json build/performance/source-paired
```

Pick `linux_x86_64`, `linux_arm64`, `macos_arm64` or `windows_x86_64`. Source
mode sets `OMEGA_LIBRARY_ROOT` to this checkout's `source/library`, records it
as an input, and requires exactly one accepted part and accepted final verdict,
without warnings. It skips Build and native publication. `prepare` and `prove`
are target-neutral and reject `--target`.

Rust rebuilds the exact pinned square-sum source and checks its hash:

```text
target/release/omega-bench proof-square build/performance/proof-square
```

It reads Git commit `9b4898865ecd5789bea4def6c2da078192a4ab82` locally, keeps
the algorithm, constants and checksum, and adds the exact measured literal
qualifications. A shallow checkout lacking it must fetch it first. The default
Build keeps six optional optimization selections; `--identity-build` produces
the separately pinned identity Build. Both source hashes are checked before
writing the new directory. `wide64` and `nested8` are small extra conjunction
controls either example accepts; they are not representative source.

Keep raw records under the ignored `build/performance/`. Windows observations
from the retired local scripts stay archived there; the portable runner records
new per-platform resource observations. The permanent report at
`wiki/drafts/measurements/proof_preparation_2026_11_14.md` ties historical runs
to their recorded sources and producer identities.
