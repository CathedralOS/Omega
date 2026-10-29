# Cathedral UI before/after comparison: setup blocked

The subsequent [Matrix component comparison](matrix/README.md) has a qualified
warm compilation result. This page preserves the unsuccessful **UI** attempts.

**No qualified end-to-end UI performance result.** Both matched release
producers exceeded the ten-minute package-preparation deadline. Neither reached
an accepted project lock or native publication. There are no warm A/B samples,
native products, medians, speedup percentages or confidence intervals to report.
The earlier small-workload speedups remain unvalidated on the UI workload.

## Workloads and bounded attempts

| Attempt | Producer | Preparation limit | Observed outcome |
| --- | --- | ---: | --- |
| Current Windows gallery snapshot | Retained before, opt-level 1 | 1,800 s | `review-timeout` |
| Previously qualified combined UI exports | Retained before, opt-level 1 | 300 s | `review-timeout` |
| Combined UI exports, diagnostic with phase collection | Retained before, opt-level 1 | 180 s | timeout; only source-record path emitted |
| Combined UI exports | Matched release before, opt-level 3 | 600 s | `review-timeout` |
| Combined UI exports | Matched release after, opt-level 3 | 600 s | `review-timeout`; 602.453 s including termination/cleanup |

These are censored **setup attempts**, not compile-time observations or a
before/after latency comparison. The timeout helper terminates the owned process
tree; its post-termination exit code is not a compiler diagnostic. Its original
timeout route discards partial stdout/stderr. The separate diagnostic and final
candidate attempt retain partial streams explicitly.

The gallery snapshot captures Cathedral `e493d887eee66f1769a79998209db393c832f4ad`
plus its current working source changes: 170 files, 3,100,360 bytes, including
159 Omega files. Both copied authored trees were checked byte-for-byte.

The alternate case is the prior qualified **combined layout/frame export
composition consumed by Cathedral's Rust desktop**. It is a previously frozen
source variant, including its recorded compatibility migrations, not a current
gallery result or a whole OS build. It contains 172 files, 3,101,352 bytes,
including 161 Omega files. These counts describe the frozen tree, not a measured
reached-input count. No authored source was changed between either pair's roles.
The two extra files are the combined export probe and its Build companion.
Four shared files differ between these variants: `platform/native/{build,main}.omg`
and `tests/native_component/{frame,layout}/build.omg`. Neither variant is mixed
between a comparison's roles.

`frozen-inputs.zip` preserves both variants once, with complete manifests. The
source checkout at `C:/Development/Cathedral` was read only. All setup state,
temporary builds and caches belong to this Omega worktree.

## Exact producer states

The retained opt-level 1 executables are the original experiment's producers:

- Before: `adefab2059598ae84b412a4d67b6abd08cbd7f134c8c2f1d80bde7f11308216a`.
- After: `24d724782914175d7f0a001c1287399fca017cbcda3ac6e1288e1c411c561012`.
  It was verified but not run on Cathedral: the baseline preparation failed first.

Both release variants were built with `mbx build --locked --release -p omega`,
under one build-slot hold, with the pinned nightly and six Cargo build jobs:

- Before: `2f726243218ac08b9723f85c22aa9d75e1924bfa0fd5a2530f0beee1befac194`.
- After: `a39a2b65325fa5e113e084eb85282b8892ebf0b030749dee000454c6a71326b6`.

The release source base is `4092626edbefe57280f352dd75010c2d34352a66`. Baseline
restores only compiler-storage's `buffer.rs` from pre-optimization
`b1d401fd1d3ddc5470d915bc5ab5044c2e093677`; candidate uses the committed registry
optimization. The test-only fixture and controls are present in both source
states. The baseline buffer SHA-256 is
`57a4bd9bf2a2feb897956e22026fffcfd5b90f63d14c992d3fc26ff571e8b595`;
candidate is `900d8abb96dc8b378e855ed049cdf5f7b0eaea39b88ef5b54bb6e82747d2bf83`.
The original source bytes were restored after building. Both executable
`--build-profile` observations report opt-level 3. Version strings are not used
as source provenance.

The release builds took 13m22s and 11m45s. Those are Rust build setup costs,
not Omega compilation measurements and not part of a speedup denominator.

## Measurement contract that did not reach execution

The fixed plan called for 32 initial alternating AB/BA pairs, then 16 separate
confirmation pairs, with two warmup pairs per batch. Every role owns a private
`LOCALAPPDATA` cache and independently reviewed lock. Both use the same explicit
Omega library. No cache envelope, compiler identity, proof or admission check
was changed or bypassed.

For the export case, the oracle requires all six native outputs: two shared
components, two descriptions, the probe and export index. Cathedral's existing
ELF validator checks the components; the bootstrap probes must execute under
WSL Ubuntu. Every timed compile must reproduce the complete validated byte set.
WSL availability was checked, but **no fresh Cathedral probe ran**.

The adapter uses existing `capture_compile` for process startup through output
capture and completion, reporting nanoseconds per one full compile through
`omega-bench`'s reported-work protocol. Output hashing, structural checks and
native validation stay outside that timer. Adapter-level resource counters are
not compiler-only measurements. The gallery has variable frame-timing stdout,
so its proposed oracle uses the existing headless pixel checks separately.

None of this validation was reached. No `omega-bench compare` run or report was
produced. Missing samples are not assigned zero time, and no fast partial stage
is substituted for a complete compile.

## What the progress diagnostic establishes

A separate temporary diagnostic binary adds a 15-second snapshot of existing,
process-wide phase counters. Its source patch is retained; the CLI source was
restored afterward. This binary is not either member of the matched pair and
its instrumented observations are not performance samples. Its release build
took 11m01s; the update attempt timed out at 180 seconds (182.727 seconds with
termination/cleanup).

Twelve checkpoints cover nominal elapsed times 15 through 180 seconds. By the
second checkpoint, completed scope `05.d.7.3b.1.cap.2d.local_init` totals
**21,746.016 ms across 348 calls**, the largest completed inclusive scope. All
reported completed rows remain unchanged from that checkpoint through the end.
The source-graph cold counter is one, and neither release cache contains a
source-graph record. This is not a qualified warm decoding workload.

The scope lives at
`omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/structural_case_arms.rs:1329`.
It surrounds local-initializer contract replay: `judge.normal_return_paths` for
an initializer expression, or zero-value structural-term construction and judge
cloning for uninitialized storage. This is an actionable lead for further
attribution, not proof that one of those operations owns the entire stall.
Only completed scopes are recorded; an active scope or the work between scopes
can consume the unreported remainder. The trace does not establish nontermination
or an exact currently active function. Nested inclusive rows must not be summed.

[`progress-summary.json`](progress-summary.json) gives the checkpoint summary;
the archive retains all raw phase output. The next investigation should narrow
this contract-entailment path before attempting another warm comparison.

## Evidence and remaining requirement

`setup-observations.zip` retains commands, source identities, compiler build logs,
preparation outcomes, partial streams, diagnostic patches and replay scripts.
Optimized executables and owned caches remain under ignored
`build/performance/cathedral-ui-generation-domains/` and
`build/performance/cathedral-generation-domains/`. Restoring source from its Git
revision alone is insufficient: use the frozen authored input archive.

The shared desktop had other Rust builds during setup. We did not alter host
settings, stop other work or open the gallery window. No corpus, full OS build,
cold/warm compile comparison or desktop-interaction acceptance was run.

The remaining prerequisite is a completed, normally admitted Cathedral package
preparation with these exact producers. Its CPU-consuming stall needs attribution
before a trustworthy warm compile comparison can start. Timeout does not prove
that the compiler can never finish, and these observations do not establish a
decoder regression or a Cathedral speedup. Nothing was pushed.
