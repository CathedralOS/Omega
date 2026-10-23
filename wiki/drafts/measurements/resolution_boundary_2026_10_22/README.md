# Resolution boundary measurement, 2026-10-22

**Result: repeated resolution and expression-table copying are measured; no
incorrect upstream input was demonstrated.** Every workload made one early
numeric-probe resolution, one deferred resolution, and two body-completion calls.
Every workload cloned resolved expression tables eight times. Both traced
repeats reproduced all counts and captured field totals exactly.

| Workload (`local/main.omg`) | Roots visited, early + later | Live expression rows copied, early + later | Total rows copied |
| --- | ---: | ---: | ---: |
| `length` | 321 + 319 | 5,575 + 29,055 | 34,630 |
| `dyn_dispatch_selector_payload_bound` | 322 + 320 | 5,559 + 29,013 | 34,572 |
| `nested_nine_level_record_field_bound` | 331 + 329 | 5,601 + 29,192 | 34,793 |

Each compile also made three syntax-forest clone calls: one before preparation
and two within it. All captured probe events were on `ThreadId(3)`, so this
capture's pre-resolution attribution does not cross an untracked worker boundary.
See [raw observations](observations.json) and [summary](summary.json).

**Decision: retain this measurement record, with no production optimization.**
The next question is whether placed-view applied-result discovery can select its
exact required declarations without materializing a whole resolved program. Its
selected declaration identities and resulting placement demands must be compared
before replacing that path. Do not start unrelated downstream representation
changes on the assumption that this upstream representation is settled.

This does not establish that eight copies are unnecessary or that copying is
the dominant cost. Across the six traced compiles the sum of expression-table
clone scopes was 0.668–1.254 ms; the early resolver scope was 8.382–14.367 ms.
These are diagnostic elapsed times under contention, not savings estimates.

## Question and scope

Before optimizing the layer after source assembly, determine whether compile-time
preparation repeatedly resolves source and copies resolved expression tables.
The user requested inspection and measurement because changes to an upstream
representation could invalidate downstream optimization work.

This bounded experiment uses the unmodified `local/main.omg` witnesses for
`length`, `dyn_dispatch_selector_payload_bound`, and
`nested_nine_level_record_field_bound`. It measures their complete native compile
path on macOS ARM64, with probes around preparation, resolver entry points,
root lowering, syntax cloning, and resolved expression-table cloning. It does
not establish saturation of a layer or audit every compiler representation.

The frozen base is `114906953e11e283b8082f03d8da99fbf39dff7e`. The diagnostic
compiler differs only by the saved instrumentation patch. Both compiler
executables are frozen separately with digest-bound effective optimization
profile receipts. Temporary probes are removed before capture.

## What the trace settles

The initial suspicion included repeated const-generic evaluation probes. These
three inputs instead show exactly one early `resolve_numeric_probe` call. Its
sole source call site at this base is
[`retain_applied_results`](../../../../omega-rust/psi/compilation/compile-time/src/layouts/placed_views/applied_results.rs),
which clones the syntax forest and resolves it to discover applied call-result
placement. Its early guard asks whether any call has machine arguments; the
measurement does not yet count how many useful placement demands this produces.

The eight expression-table copies are distributed across two table rebuilds,
two body-completion base copies, two body-table append operations, and two
additional copies under authored completion. The latter context is consistent
with the prepared-allocation joins and their `Arc::make_mut`; the probes do not
individually instrument those join calls. Body completion selects 108 machines
in preparation, then 2, 2, or 3 authored machines for the three workloads. Those
different ownership frontiers explain why deleting repeated work without
checking exact identities would be unsafe.

The direct handoff from syntax to deferred resolution consumes its input.
This is not evidence that every compiler boundary duplicates its entire input.
The constructor-path string/split issue found during the earlier source audit
is not measured here and remains part of the separate layer-0 investigation.

The [placed-result discovery follow-up](placed_discovery/README.md) measures the
consumed output and rejects unconditional removal: ordinary workloads produce
no selections, while returned/forwarded placed views lose invocation locations
needed by placement admission despite retaining identical native bytes.

## Quality and producer evidence

- All 12 compile observations exit 0, and their published native programs each
  execute with exit 0. For each subject, all four native image SHA-256 hashes
  match exactly across baseline, untraced diagnostic, and two traced diagnostic
  invocations. The original source hashes and frozen compiler hashes are stable.
- Baseline preflight observes native exit 0 and Terminal interpreter exit 0 for
  all three original workloads. The later control observes both engines again
  for `length` on each compiler. The instrumented interpreter was not separately
  exercised for the other two workloads.
- The `length/local/invalid` twin rejects on both producers with exactly the
  same cleaned diagnostic: it cannot prove that `length - 1` is within the
  unknown slice length of `window`. This is the intended fact obligation.
- Both producer receipts verify effective opt-level 1 for the compiler library,
  Omega library, and CLI executable. Baseline SHA-256 is
  `b0f43188ac2593634f6a3a30f7d262a83d734acaca9eeea268b4c5752d893d5c`;
  diagnostic SHA-256 is
  `6c39064e8f76e8a1aadff9a79f0286691a1dc6724d0bebba5d3a5f6fe9ced2dd`.
- The retained compiler diff is empty. No authored source, oracle, shared build
  tooling, semantic cache, or allocation strategy changed. This is a bounded
  investigation under `ITERATION-PERFORMANCE`, not closure of that board outcome.

Evidence: [controls](controls.json), [baseline engine observations](baseline-preflight.json),
[baseline profile](baseline-producer.json), [diagnostic profile](instrumented-producer.json),
[build records](build-record.json), and [temporary probe patch](instrumentation.patch).

The baseline build took 13m28s and the diagnostic build 7m18s after substantial
build-slot waiting. Neither duration measures compiler runtime performance.
Capture ran from 2026-10-22 23:59:46 UTC to 2026-10-23 00:00:05 UTC on an Apple
M4 with 10 logical cores and 16 GiB RAM. One-minute load averages during capture
ranged from 20.68 to 22.94. Baseline compile wall times were 1,861.7, 1,290.7,
and 1,080.0 ms; traced repeats ranged from 1,114.4 to 1,663.0 ms. These different
workloads and contended invocations do not support a speedup comparison.

Two setup mistakes are preserved. The first profile receipt could not map mbx's
public executable path to its artifact fingerprint; the corrected receipt maps
by identical executable digest before verifying flags. The first preflight used
`run --both --target macos_arm64`, which published without executing; the runtime
guard rejected it and the successful host-only invocations replaced it. See
[initial receipt failure](baseline-receipt-initial.log) and
[publication-only attempt](baseline-preflight-target-only.json). Neither is
counted as a runtime pass or a compiler defect.

## Measurement interpretation

The `Resolution` observer in `SourceCompilationRequest::prepare` starts after
`evaluate_pre_resolution`. Resolver work performed by that earlier evaluation
is therefore outside that observer's timing. The trace labels it separately.
The category `later` means outside pre-resolution; it includes deferred body
completion and is not synonymous with one final resolver call.

Root visits sum the ordinary root count at `lower_items` entries. They are
not unique source declarations or proof that each visit is unnecessary. A
changed input or a different demand may require another pass. Expression rows
are the live row counts at expression-table clone calls, summed with repetition.
They are not unique expressions, bytes allocated, peak storage, or RSS. The
resolved table's ordinary arenas deep-clone storage; syntax clone calls can
share pages and must not be interpreted as the same cost.

Nested scopes are reconstructed per thread with balanced begin/end checks.
Inclusive parent and child durations overlap. Trace logging and computing probe
details add overhead, and parent scopes include that overhead. Durations are
diagnostic attribution only. The instrumented binary with tracing disabled
still has probe checks and is not the baseline compiler.

The host was heavily contended by concurrent work. Two traced repeats per
subject test count repeatability; the one baseline and one untraced diagnostic
compile per subject do not establish a performance comparison or confidence
interval. No warm steady-state claim is made. Inputs and output bytes are
checked independently of timings.

## Replay

Use a separate checkout at the frozen base and copy this experiment directory
into it. From that checkout's root:

1. Create `build/resolution-measurement` and write the exact base SHA to
   `build/resolution-measurement/base.txt`.
2. Run `python3 wiki/drafts/measurements/resolution_boundary_2026_10_22/queue.py`.
   It uses the existing two-slot build protocol, builds and freezes the baseline,
   installs temporary probes, builds and freezes the diagnostic producer, restores
   the touched compiler paths, then runs captures, controls and summarization.
3. Inspect all process exits and the final `metadata.checks` in
   `build/resolution-measurement/observations.json`. A partial capture is not a
   result. Preserve failed attempts before retrying.

The local queue wrapper changes only its own poll interval to 0.05 seconds after
repeated overtaking under contention. It does not change shared slot capacity or
ownership. `--resume` is only for a completed baseline whose saved digest still
matches. If a build fails with probes installed, preserve its patch and restore
exactly the paths in `instrumented-paths.json` before resuming.

The scripts reuse the repository benchmark's compile, timing, RSS and native
execution helpers, and compiler-profile verification. They do not use a separate
semantic implementation. `run --both` controls omit `--target`: on this base an
explicit target takes the publication path, so successful compilation alone
would not establish engine execution.
