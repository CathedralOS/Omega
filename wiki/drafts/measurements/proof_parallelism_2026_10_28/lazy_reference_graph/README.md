# Deferred reference-graph construction

Warm compilation improved in all six pairs; cold compilation improved in four
of six. The candidate eliminates all three graph builds on this program, but
misses the promotion rule declared before testing. **The patch is parked and
the compiler source is restored.** No unchanged trials were added to rescue it.

## Fixed program and hypothesis

The workload remains `samples/cli/games/dungeon_crawler_cli/main.omg`, with the
same 203 authored inputs, EOF stdin, exit 0 and 463-byte output containing
`A bottomless dark room near the dungeon heart.` The existing input manifest
and producer-independent native-image oracle are unchanged.

The preceding [reconciliation experiment](../terminal_reconciliation/README.md)
found three 202-machine reference-context graph builds costing 0.47-0.77 seconds.
Their initializing calls needed no shared-reference leaf query; whether later
calls would need the retained graphs was still unknown. This experiment delays
construction until an actual projected shared-referent backing query.

The pure [candidate patch](candidate.patch) changes only
`reference_origins/memberships.rs` in `terminal-semantics`. A private kernel
accepts a graph provider and initializes an invocation-local graph/analysis pair
with `Option::get_or_insert_with` at the existing backing-query call. The public
graph-supplied API stays unchanged. Existing carrier, duplicate, path, extent,
origin, root and query-formation checks, receiver annotation masking and normal
completion rules remain in place. No cross-sweep cache or new semantic rule is
introduced. Two new tests cover unused providers, malformed mappings and a real
projected shared query that forces the provider.

## Matched whole-compilation results

Six rounds use pure original and candidate producers, each with its own accepted
lock. Every cold run receives a new private cache and output directory. A warm
run reuses only its own producer/round's cold cache and receives a new output
directory. AB/BA orders are balanced independently in both modes. None of our
builds overlapped these measurements. Counter runs are separate and excluded.

Positive percentages below mean a reduction from original to candidate, computed
per pair before taking the median. They are not ratios of cohort medians.

| Cache | Faster wall pairs | Median paired wall reduction | Faster CPU pairs | Median paired CPU reduction |
| --- | --- | --- | --- | --- |
| Cold | 4/6 | 3.65% | 4/6 | 9.78% |
| Warm | 6/6 | 11.93% | 6/6 | 12.42% |

All wall observations in seconds, without timing-based exclusions:

| Round | Cold original | Cold candidate | Warm original | Warm candidate |
| --- | --- | --- | --- | --- |
| 0 | 17.2402 | 18.0586 | 9.9281 | 7.4414 |
| 1 | 31.8187 | 30.6970 | 13.7197 | 11.9669 |
| 2 | 23.0505 | 27.5768 | 12.4588 | 11.4827 |
| 3 | 22.5816 | 18.8306 | 12.0393 | 10.4976 |
| 4 | 25.2397 | 18.3385 | 7.6517 | 6.8032 |
| 5 | 16.7125 | 16.0809 | 7.1681 | 6.5348 |

Cold paired wall effects range from -19.64% to +27.34%; warm effects range from
+7.83% to +25.05%. Shared-host variation limits attribution; it does not establish
the cause of either cold regression. The observed warm gain does not establish
a gain across both cache modes or another program.

The predeclared rule requires at least 5/6 positive wall pairs in **each** mode,
at least 4/6 positive CPU pairs and positive median wall/CPU reductions, alongside
the quality checks. [summary.json](summary.json) recomputes to
`promotion_supported: false`. Acceptance was not changed after seeing results.

## Actual graph demand

The independently frozen counter producers observe:

| Mode | Original full graph builds | Candidate ready calls | Candidate full graph builds |
| --- | --- | --- | --- |
| Cold | 3 x 202 machines | 4 x 202-machine contexts | 0 |
| Warm | 3 x 202 machines | 4 x 202-machine contexts | 0 |

All four qualifying candidate call preparations stayed short of a graph-backed
query. This measures actual eliminated builds, rather than assuming later calls
also have the initializing call's requirements. The [diagnostic patch](diagnostic.patch)
adds only ready/build counters in the changed file; it is independently applicable
to the recorded base, not stacked on the pure patch. Counter overhead and all
counter-producer timings are excluded from the pure A/B result.

## Behavior and provenance

The pure candidate passed 24 focused tests (22 existing payload tests and two new
tests; six unrelated tests skipped). Optimized CLI and focused corpus-runner
builds completed. Four positive source fixtures execute with native exit 0,
Terminal exit 0 and agreement; three invalid source controls reject with the
expected fragments. The untouched semantic-base runner produced exactly the
same status, facts and diagnostics for all seven fixtures.

The corpus diff gates nevertheless returned **1 for both producers**: one
positive fixture is absent from the retained golden, and one mixed-origin
negative's diagnostics moved relative to that golden. The exact baseline replay
establishes that these discrepancies predate this candidate. No golden was
recorded. [quality-summary.json](quality-summary.json) and the individual reports
preserve the nonzero gate results; these are not claimed as green diff gates.

All 31 compiler observations succeeded: 24 pure timing runs, two pure both-engine
comparisons, four counter runs and one flag-disabled counter-producer comparison.
The 28 ordinary compiles each execute their native image; the three explicit
comparisons run both engines. Every output is the same 463 bytes and every
native image has SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.

The recorded base is `d9c92de878a42e495f45fdf49ba228d64294f4aa`; its compiler
source equals semantic base `456c5e984452a45d977295e254b3ca9ae3cc7750`.
[Candidate source identity](candidate-source.json),
[diagnostic source identity](diagnostic-source.json),
[pure producer identities](identities.json),
[counter identities](counter-identities.json) and
[corpus identities](corpus-identity.json) bind patches, binaries, normal accepted
locks and optimized profiles (`opt_level=1`). The original counter producer is
the verified retained `terminal-breakdown` producer from the preceding record.
[Patch checks](patch-checks.json) verify both patches apply independently to the
exact base and reproduce the pure candidate's source digest.

## Evidence replay and limits

From the repository root:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/lazy_reference_graph/analyze.py
```

The analyzer checks the manifest, identities, all 31 observations, native/Terminal
agreement, graph counts, warm-cache ancestry, every paired result and the exact
baseline-equivalent focused corpus outcomes. Raw records and compressed streams
are under [raw](raw/). [research.md](research.md) records the original hypothesis,
acceptance, API research, scheduling correction and final decision. Build and
quality logs are compressed alongside the scripts. The optional reported phase
rows are post hoc diagnostic observations; they cannot rescue promotion or be
summed into a serial fraction.

To repeat execution, use an isolated checkout at the exact base, apply only the
pure patch for the candidate, rebuild/freeze optimized producers and create
producer-specific locks through normal `omega update` review. Reuse the existing
203-input manifest and measurement runner; scripts refuse existing case labels.
Build the diagnostic producer separately. Binaries, locks, build trees and caches
remain ignored local evidence, not committed artifacts. A rebuild receives new
binary/lock identities and is not the archived producer by assertion.

No full corpus, other host, alternate gameplay route or global serial-fraction
qualification was attempted. The fixed workload resolves the graph-demand
question and shows a warm benefit; cold benefit remains unestablished. Any
revisit needs a changed measurement condition or new representative evidence,
rather than additional unchanged trials. Only this evidence record is retained
in the branch; compiler, program, fixtures and goldens are unchanged.
