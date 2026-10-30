# Snapshot intersection scan fusion

**Decision: reject this candidate for promotion.** It removes repeated membership scans and preserves the checked intersection, but does not establish a useful full-workload gain. Replay is slower in 5/6 paired observations and whole compilation in 4/6. The pure candidate is retained as a patch alongside all diagnostic evidence; compiler sources are restored.

## Scope and invariant

The original `retain_common_snapshot` scans until it finds a missing retained premise. If any fact is lost, it calls `Arc::make_mut` and restarts a complete ordered filter from the beginning of both rosters. [candidate.patch](candidate.patch) keeps the first scan's cursor, preserves the already-proved prefix, drops the first loss and compacts only the remaining suffix with read/write indices. Each retained row gets one membership classification. This is one logical membership pass; copy-on-write cloning still traverses shared rosters when an intersection changes them.

For sorted rosters, incoming facts already passed by the cursor cannot match a later retained row. Every survivor is moved to the next gap in its original order. Truncating the suffix discards precisely the lost rows. An unchanged roster returns before requesting mutable ownership. All comparisons still use complete propositions, including exact storage/value versions; there is no fact union, new equality relation, cache, normalization or retirement rule. The generic `retain_common_premises` remains for its independent callers. The two snapshot callers stay in `State::meet`.

This preserves the published [premise availability and exact substitution](../../../../spec/terminal-psi/fact_validity.md) contract. The implementation uses safe standard operations. Rust documents order-preserving [Vec retention](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain) and [Arc copy-on-write including weak-owner behavior](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut). Source review found immutable, derived-order compiler propositions rather than executable user cleanup. The candidate defers removed-handle drops until truncation and performs explicit survivor swaps; memory and elapsed time must be measured rather than inferred from fewer comparisons.

Experiment HEAD is `918d31d10acb687b5d81bb827f8e7f3e6e78b146`; its compiler equals semantic base `456c5e984452a45d977295e254b3ca9ae3cc7750`. [source-identity.json](source-identity.json) binds the pure, outer-timed and differential-count patches. [identities.json](identities.json) pins the frozen original `payload` producer and new `snapshot-fusion` producer, both verified opt-level 1 and prepared with normal offline producer-specific accepted locks. The previous shared-predicate hoist and other parked candidates are absent.

## Six pre-scheduled alternating pairs

Same `samples/cli/games/dungeon_crawler_cli/main.omg`, same 203 authored inputs, EOF stdin, `windows_x86_64`, fresh private caches and the same original outer reconciliation probe in both binaries. No scan counters or shadow reference algorithm run in the timing producer. All valid observations are retained. Cells are original / fused; process CPU may exceed elapsed time because other compiler work is parallel.

| Pair | Order | Source replay, ms | Whole compile, s | Process CPU, s |
|---|---|---:|---:|---:|
| 1 | original then fused | 2370.04 / 2598.65 | 27.689 / 30.316 | 34.141 / 36.969 |
| 2 | fused then original | 2838.66 / 2409.66 | 34.749 / 30.001 | 39.625 / 35.609 |
| 3 | original then fused | 2385.81 / 2462.34 | 31.826 / 31.104 | 37.078 / 37.188 |
| 4 | fused then original | 2034.27 / 2198.93 | 28.174 / 30.822 | 34.109 / 36.875 |
| 5 | original then fused | 1977.16 / 2012.05 | 25.595 / 27.178 | 33.359 / 34.781 |
| 6 | fused then original | 1856.69 / 1890.62 | 22.897 / 26.861 | 29.297 / 33.109 |

The fused producer is faster in 1/6 pairs for replay and 2/6 for whole compile. Median paired changes are **+2.52% replay, +7.79% whole compile, +6.19% process CPU**, all slower. CPU is lower in only 1/6 pairs. Paired ranges remain broad: replay -15.11% to +9.65%, whole compile -13.66% to +17.32%, CPU -10.13% to +13.01%. These shared-host observations fail the promotion gate; their magnitudes remain descriptive rather than a calibrated causal slowdown estimate.

Producer medians are 2202.16 / 2304.29 ms replay, 27.931 / 30.159 s whole compile and 34.125 / 36.242 s CPU. Use paired changes for the experiment decision. [summary.json](summary.json) also retains every peak-working-set/peak-commit observation; no memory saving is claimed. The separate native/Terminal agreement run uses the shared user cache and is excluded from A/B timing. Diagnostic elapsed times are also excluded.

## Exact work and fact equivalence

Two separate count runs yield identical complete rosters. In the 202-machine batch:

| Actual work | Original | Fused |
|---|---:|---:|
| Retained-row membership visits, snapshot plus generic filter families | 9,698,672 | 8,267,548 |
| Ordered/equality comparisons, those families | 28,805,456 | 24,530,520 |
| COW Arc-handle copies in snapshot intersections | 5,076,178 | 5,076,178 |
| Shared snapshot detachments | 14,456 | 14,456 |
| Unchanged snapshot returns | 4,760 | 4,760 |
| Successful real-proposition/changed-flag equivalence checks | Not instrumented | 28,540 |
| Explicit survivor swaps in the candidate | Not instrumented | 5,466,640 |

The fused helper removes **1,431,124 repeated row visits (14.76%)** and **4,274,936 comparisons (14.84%)**. There are 23,780 changed snapshot intersections and 311,696 removed premises. It performs 5,466,640 explicit survivor swaps and retains all 5,076,178 COW handle copies. These are logical operations, not measured machine instruction or allocator counts. The COW work is not reduced by scan fusion, and the new compaction has its own movement cost; the timing data does not isolate their individual CPU costs.

All 28,540 snapshot intersections per run compare the exact ordered real propositions and changed flag against the original helper. The reference implementation receives an independent outer roster containing cloned handles; it never clones the actual snapshot Arc, so it does not inflate the candidate's outer ownership count or force extra COW. The reference's own scans/copies are excluded from candidate counters. All other work-family counters match the original record, including query roots, worklist/graph work, edge retention, normalization, publication and retirement. Only `retain_snapshot` and its now-independent `retain_premises` family change.

Counter labels: `retain_snapshot.visited` is actual membership classifications and equals its 8,267,548 input-row sum; `compared` counts executed comparisons and `advanced` incoming cursor steps. `cloned`/`detached` count the actual candidate's shared COW copies/detachments; `changed` counts survivor swaps in this diagnostic variant. `rosters` counts successful full-result/boolean equivalence assertions. `same`, pointer `alias` and alias input-row counters keep their prior meaning and match the original. The remaining generic helper has 420,860 calls with empty retained rosters, so zero actual row visits, versus the original's 444,640 calls including changed snapshots. This is why the before/after sum includes both families without counting available roster lengths as actual visits.

[counts.patch](counts.patch) preserves this untimed source and [counts-identity.json](counts-identity.json) its separate optimized producer. Diagnostic elapsed times include shadow copies/checks and are excluded from performance comparisons.

## Validation and replay

The exact original and candidate function bodies are extracted into [intersection_cases.rs](intersection_cases.rs) and compiled with the pinned Rust toolchain using an ordered root/version proposition model. It runs **196,864 cases**: every pair of subsets of an eight-value ordered universe under unique, shared and weak-only ownership, plus incoming aliases. It checks contents, order, changed status, survivor Arc identity, unchanged allocation sharing and matching copy-on-write/weak-owner behavior against the old helper. This is a source-free algorithm check; the surrogate is not a language/compiler proof. The real-proposition diagnostic below provides workload-specific equivalence. The analyzer verifies the extracted candidate body's digest against the recorded production candidate.

The timing build passes 22 existing selected payload tests (6 skipped); the diagnostic build passes 23 selected tests (6 skipped), including the probe's source-free timer-partition test. No new compiler-internal replay behavior test is added. [build.ps1](build.ps1) keeps the algorithm check, focused tests and optimized CLI build in one build-slot hold. Formatting uses Rust edition 2024.

All 12 timed compiles and two diagnostic compiles return zero, emit image SHA256 `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`, and execute natively with EOF, exit 0 and the authored bottomless-room output. Explicit timing-candidate `omega run --both --keep` returns zero with identical 463-byte native/Terminal stdout and the same retained image. The input manifest is checked before and after each observation. Retained baseline count evidence remains applicable because semantic compiler inputs and authored sources are unchanged; the new diagnostic additionally executes the original helper on every actual intersection.

[snapshot_fusion_run.py](snapshot_fusion_run.py) reuses the committed scans/cache harness for fresh directories, input verification, identities, exit codes, image/oracle checks, process CPU and memory. [patch-check.json](patch-check.json) verifies every patch against the recorded base using a separate Git index. Large frozen binaries, accepted locks and private caches remain ignored under `build/proof-profile/`. The standalone algorithm check can be rebuilt directly from its archived source with `rustc --edition 2024 -O`. Fresh compiler reproduction needs newly built/verified optimized producers with their own normal accepted locks, recorded digests and new case labels. [research.md](research.md) records the initial hypothesis, exact web query/primary sources, code audit, acceptance and stopping rule.

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/snapshot_fusion/analyze.py
```

The archived check verifies every manifest digest, all 15 observations, patch/source/producer identities, extracted algorithm source, repeated counter rosters, equality of all non-target work families, storage-copy counts, runtime outcomes and paired summaries. Full compiler instrumentation is removed after the experiment and production compiler sources remain equal to the semantic base.

No full corpus, new invalid-source qualification, other hosted target or alternate gameplay is claimed. Stop at the pre-scheduled six pairs. The remaining copy-on-write work is a distinct bottleneck question; this result does not justify continuing to extend the same one-pass variant.
