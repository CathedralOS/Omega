# Shared predicate membership once per edge

**Decision: park the compiler change.** The hoist reduces measured shared-fact searches while preserving the observed payload maps, but these six pairs do not establish a reliable whole-compilation improvement. No candidate or probe remains in production compiler sources.

## Hypothesis and exact scope

[The prior attribution](../scans_joins/README.md) counted 22,041,404 shared-predicate equality comparisons over 140,303 payload visits in Dungeon's main replay batch. Shared-predicate retention occupied 108-139 ms gross wall time there. The same immutable `arrival.shared_requirements` list was searched inside each payload's loop. [candidate.patch](candidate.patch) moves that search before the loop and skips the local loop if the shared predicate is present. If it is absent, the original local membership check and predicate append remain. No index, cache, owned fact reconstruction, normalization, retirement, rebinding or join policy is added or changed.

For every payload, old and new both append exactly when the predicate is absent from both its local roster and this edge's shared roster. Both equality comparisons are pure and the shared roster is unchanged throughout the loop. Hoisting may perform a search that the original short-circuit skipped when all local rosters already contain the predicate, or when no payload exists; the diagnostic counts measure the net effect. Exact proposition equality is retained. Applicable contracts are [premise availability and exact substitution](../../../../spec/terminal-psi/fact_validity.md) and [structural predicate identities](../../../../spec/terminal-psi/structural_predicates.md).

The semantic compiler base is `456c5e984452a45d977295e254b3ca9ae3cc7750`; experiment HEAD is `a186b662f5b39e1de7d6ebe3c79d5fd851301714`, with the same compiler sources. [source-identity.json](source-identity.json) binds the pure candidate, timing instrumentation and differential-count patches. [identities.json](identities.json) binds the retained original `payload` producer and new `shared-membership` timing producer, both opt-level 1. Each uses its own accepted lock produced by normal offline `omega update`. The earlier cache/sharing/borrowing candidates are absent.

## Six pre-scheduled alternating pairs

Same Dungeon program and 203 authored inputs, EOF stdin, fresh private caches, identical target/flags and the existing outer reconciliation probe in both producers. No new per-comparison/per-payload counters run in these timing observations. They include all valid runs, with no outlier removal. Values in each cell are original / hoisted. Full process CPU can exceed elapsed time because other compiler work is parallel.

| Pair | Order | Source replay, ms | Whole compile, s | Process CPU, s |
|---|---|---:|---:|---:|
| 1 | original then hoisted | 1886.79 / 1338.65 | 19.484 / 18.920 | 26.703 / 23.969 |
| 2 | hoisted then original | 1250.65 / 1178.73 | 16.567 / 17.336 | 22.797 / 23.688 |
| 3 | original then hoisted | 1229.54 / 1182.32 | 16.766 / 17.277 | 23.219 / 23.125 |
| 4 | hoisted then original | 1256.62 / 1263.09 | 18.738 / 17.578 | 25.344 / 24.531 |
| 5 | original then hoisted | 1866.47 / 1716.20 | 19.082 / 23.828 | 25.797 / 30.656 |
| 6 | hoisted then original | 2038.95 / 1874.53 | 25.963 / 24.120 | 32.641 / 30.281 |

Replay is lower in 5/6 pairs, with a **6.90% median paired reduction**. Whole compile is lower in 3/6 pairs; its **median paired change is +0.08%** (slower). CPU is lower in 4/6 pairs with a median paired change of -1.80%. The whole-compile paired range is -7.10% to +24.87%; CPU ranges from -10.24% to +18.84%. Those mixed full-workload observations do not justify promotion. They also do not establish a regression.

For completeness, medians across producers are 1561.55 / 1300.87 ms replay, 18.910 / 18.249 s whole compile, and 25.570 / 24.250 s CPU. Ratios of these unpaired medians look more favorable than the paired result; do not present them as an established speedup. Host load varies, and the phase measures elapsed time rather than scheduled CPU. No core-cycle attribution or global serial fraction is claimed.

## Work reduction and fact-equivalence diagnostic

Two separate diagnostic runs yield identical complete counter rosters. For the 202-machine batch:

| Work | Original | Hoisted |
|---|---:|---:|
| Actual shared-fact equality comparisons | 22,041,404 | 1,455,043 |
| Predicate-edge visits with full-map equivalence assertions | Not instrumented | 4,834 |
| Potential payload visits on those edges | 140,303 | 140,303 |
| Actual local payload-loop visits | 140,303 | 133,555 |

This is a **93.40% reduction in measured shared comparisons**, with 6,748 local payload-loop visits skipped. The shared predicate was present on 218 edge visits. The new available-shared-row sum is 1,457,543; it is a roster-length sum, distinct from the 1,455,043 comparisons actually executed.

The diagnostic build clones the incoming payload map, executes the old loop on that reference, executes the hoisted loop on the actual map and asserts exact map equality before continuing. This checks complete requirements, their ordering and all other contract fields. The 4,834 recorded source-window edge visits pass in each run. Other replay windows also execute the assertion, but no unrecorded check count is claimed. Every counter outside the predicate-retention family, including the query roots and replay work, matches the retained original count record exactly.

[counts.patch](counts.patch) contains the untimed instrumentation and shadow implementation. Its executable is separate from the timing candidate and pinned in [counts-identity.json](counts-identity.json). The reference loop's comparisons are deliberately excluded from the hoisted counter. Diagnostic elapsed times include the reference copies/checks and are **not** performance observations. The `alias` field in this diagnostic label counts successful full-map equivalence assertions; it does not mean pointer equality. `same` counts edges whose shared list contains the predicate; `rosters` counts potential payloads and `visited` actual local-loop iterations.

## Quality, artifacts and reproducibility

The timing candidate passes 22 existing selected payload tests (6 skipped), including predicate capture through both arrival routes, joins and retirement. The separate diagnostic build passes 23 selected tests (6 skipped), including the probe's source-free timer-partition check. No new compiler-internal replay behavior tests are added. Formatting checks use Rust edition 2024.

All 12 timed compiles and both diagnostic compiles return zero, emit the same native image SHA256 `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`, and execute natively with EOF, exit 0 and the authored bottomless-room output. A separate timing-candidate `omega run --both --keep` returns 0, with 463 identical native/Terminal stdout bytes and the same retained image. Its shared-cache elapsed time is excluded from A/B timing. Workload identity is checked before and after every observation through the existing input manifest. The retained previous baseline count record is applicable because the compiler's semantic inputs and authored workload are unchanged; the new count build additionally asserts equality on every predicate edge it processes.

[shared_membership_run.py](shared_membership_run.py) reuses the committed scans/cache harness for source identity, exit codes, native oracle, images, process CPU and peak memory; it refuses reused output directories. [summary.json](summary.json) preserves every paired wall/CPU/replay/memory observation. The [patch check](patch-check.json) applies each preserved patch against the recorded base with a separate Git index. Large binaries, accepted locks, and private caches remain ignored under `build/proof-profile/`. Fresh replays must build/freeze their own verified producers and accepted locks, record their digests and use new labels. [research.md](research.md) states the mechanism, acceptance and stopping rule before implementation.

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/shared_membership/analyze.py
```

The archived analyzer verifies every manifest digest, all 15 observations, producer/lock identities, six A/B orders, native outcomes, engine agreement, exact repeated counts, unchanged non-target work rosters and paired summaries.

No full corpus, new invalid-source qualification, other hosted target, alternate gameplay, or production speedup is claimed. Stop this experiment at the pre-scheduled six pairs; retain the small candidate patch for a future better-controlled measurement rather than repeating unchanged trials or shipping on comparison counts alone.
