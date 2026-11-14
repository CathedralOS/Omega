Proof preparation benchmark
===========================

The checker used to construct owned derivations for all available facts before
search chose evidence. The private `proof_preparation` example replays actual
successful-obligation contexts so ordinary small and unused facts remain in the
measurement. The compiler's public interfaces and portable proof format are
unchanged.

Build both retained versions under the host budget with:

```text
mbx build --release -p terminal-psi-to-terminal-psi --example proof_preparation
mbx build --release -p compiler --example route_probe
mbx build --release -p terminal-psi-to-terminal-psi --example proof_preparation_timing
```

Keep each executable before changing the implementation. The example accepts
`<main.omg> [repeats]` for a nongeneric source without external imports. It
parses, resolves, types and lowers the real program, discharges it normally,
and reconstructs the full ordered assumption/axiom rosters from each successful
certificate's evidence. Preparation of the source and independent kernel
checks occur outside the timers. Every replayed proof must equal that
executable's production proof. Compare context shapes, proof fingerprints and
complete kernel acceptance fingerprints against the retained baseline too.

`CONTEXT` lines describe conjunction-expansion geometry, available facts,
equations, ordinary retained proof-node occurrences and the selected proof.
Debug rendering counts the literal `ProofNode {` records outside measurement;
compact origins contain no such records. Retained bytes are live requested
allocation payload for one `Known`, excluding fixed inputs and allocator
metadata. Summing them describes a source pass's context footprints, not
simultaneously retained memory.

`MEASURE` lines separately cover preparation and complete proof search. The
counting System allocator records allocation/reallocation calls, requested
bytes, and maximum live payload above fixed inputs. Its atomics affect the
reported time: use allocation results diagnostically and use the separate
uninstrumented source-route process comparison for latency claims. OS process
peak memory is a separate field in `observe.ps1` output.

The `proof_preparation_timing` example compiles allocation counters out with a
constant generic parameter and forwards directly to System. Its optional third
argument `prepare` or `prove` suppresses diagnostics and emits one
`omega-timed-work/1` result for `omega-bench`'s existing reported-work mode.
`timing_plan.ps1` creates that plan with frozen context work count and checksum.
Timing is per actual obligation context; each pass keeps the source program's
original successful-obligation mix. This separates preparation cost from the
much larger source-frontier cost and from counting-allocator overhead.

The `wide64` and `nested8` inputs are bounded secondary conjunction controls,
each selecting one leaf. They are not representative source distributions.

`observe.ps1` captures one process with closed stdin and retained streams,
resource counters and binary identity. `plan.ps1` creates an ordinary
`omega-bench compare` plan for two retained `route_probe` executables. That
comparison includes source preparation and the Product checker, bypasses Build
and native publication, and checks the exact accepted-verdict output. Keep raw
observations under ignored `build/performance/`; measurement reports in
`wiki/drafts/measurements/` contain Markdown only.
