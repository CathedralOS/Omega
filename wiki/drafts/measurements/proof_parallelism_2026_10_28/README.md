# Dungeon: checking parallelism and repeated arithmetic queries

Measured 2026-10-28 on Windows x86_64, Ryzen 9 7950X (16 cores, 32 logical processors).
Base: `456c5e984452a45d977295e254b3ca9ae3cc7750`. This is a measurement,
with no compiler optimization retained and no changes to shared main.

The real workload is `samples/cli/games/dungeon_crawler_cli/main.omg`, with
closed stdin. Every admitted observation requires exit 0 and the authored
opening description, `A bottomless dark room near the dungeon heart.`

## Findings

Two fresh-cache runs per worker setting, using the same instrumented executable:

| Checker workers | Whole compile wall, seconds | Mean wall | Mean process CPU, seconds | Main machine batch, mean |
| --- | --- | --- | --- | --- |
| 1 | 23.176, 22.606 | 22.891 | 22.531 | 543.46 ms |
| 8 | 18.471, 19.186 | 18.828 | 22.336 | 75.46 ms |
| 32 | 22.047, 17.503 | 19.775 | 26.031 | 45.94 ms |

The body batch contains 1,920 machines, including imported library bodies.
Eight workers reduced this batch by 7.20x, and whole compile wall by 17.7%.
The worker override also affects other consumers of the checker-owned pool,
including flow facts and checking; the whole-compile difference cannot all be
attributed to this body batch. Other compiler pools retain their defaults.
The shared host was busy, there are only two observations per setting, and
32-worker wall times vary substantially. This does not establish an optimal
worker count or a reliable 8-versus-32 whole-program advantage.

Within the 8-worker main batch:

- Overlapping machine intervals cover **97.7–99.3%** of batch wall time.
- Exactly one machine is active for **0.44–1.52 ms**; no machine is active for
  **0.11–0.16 ms**. These are interval occupancy, not CPU scheduling samples.
- The ordered merge after the batch takes **0.83–0.87 ms**.
- Before the batch, the enclosing validator spends **312–369 ms**; after the
  merge, **24.6–25.3 ms**. These regions contain other work and may themselves
  invoke parallel helpers. They are not an inherently serial fraction.

At eight workers, the main body batch is only about 0.4% of whole compile wall.
This does **not** mean 99.6% is serial: it includes other parallel passes,
source acquisition, checking, lowering, cache encoding, publication and waiting.
These probes do not establish a global intrinsically serial percentage.

A separate fresh-cache attribution run reports 6.132 seconds in the inclusive
`05` checked-tree stage and 3.484 seconds in selected-machine lowering. A
concrete synchronous lowering region, `05.10_reconcile_and_residences`, takes
**2.064 seconds** across three invocations. It reconciles emitted places,
completes descriptor residences, and retains payload memberships. That region
is a stronger next profiling target than the sub-millisecond ordered merge.
The detailed profiler adds a global mutex and records nested/concurrent scopes;
its rows must not be summed into a serial-time total. Its timings are diagnostic,
not samples in the worker table.

The [follow-up split](reconciliation/README.md) attributes over 99.98% of that
reconciliation region to payload membership transport on this program. Its
median is 2.278 seconds; place reconciliation and descriptor residences together
take about 0.23 milliseconds. This later observation does not replace the
original run's timing or imply an optimization speedup.

## Are expressions checked repeatedly?

Yes, but most revisits have different recorded inputs. In a separate counts run:

| Scope | Visits | Distinct expression/body pairs | Distinct expression/context pairs | Repeated context visits |
| --- | ---: | ---: | ---: | ---: |
| Build preparation 1 | 1,209 | 789 | 1,205 | 4 |
| Build preparation 2 | 1,174 | 770 | 1,174 | 0 |
| Small preparation | 21 | 21 | 21 | 0 |
| Main program and library | 20,529 | 14,598 | 19,888 | 641 |
| Total | **22,933** | **16,178** | **22,288** | **645** |

Expression-only counting would label 6,755 visits (29.5%) as revisits.
Only 645 visits (**2.8%**) repeat the same recorded context. The other 6,110
revisits have changed recorded inputs. For example, expressions in
`enemy_retaliates` and `CombatSystem::fight` appear six times in three contexts.

Counting hooks cover `ExpressionWalk::analyze` **inside machine validation**,
not every expression/type/bound query in the pipeline. Identity is scoped to
validation job, machine, program data address and expression handle. Context
comparison includes machine/state/lexical owner, complete formal parameters,
declaration facts, a structural snapshot of `ValueEnvironment`, destination
primitive/domain, owner text, and the four analysis-policy flags. Environment
addresses alone are never used as equality evidence.

This is a conservative inventory of repeated recorded queries, not a sound
memoization design or a measured speedup opportunity. Diagnostic/decision output
history is not compared, and query side effects are not replayed. Structural
equality can also distinguish mathematically equivalent environments; NaNs, if
present, can prevent equality. Caching only an expression handle would merge
queries with different proof facts or checking modes. No checks were skipped.

## Controls, failures, and evidence

- Both producers report optimization level 1. Exact executable hashes, fixed
  base and instrumentation patch digest are in [identity.json](identity.json).
- [input-manifest.json](input-manifest.json) records 203 Omega source files and
  the original accepted lock. All source hashes were checked before/after the
  final series. Each producer has its own accepted lock bound to its compiler
  identity; each run records that lock's hash. Locks differ intentionally.
  [The generated policy comparison](producer-policy-comparison.txt) shows equal
  policy fingerprints and zero changed source entries; no review gate was bypassed.
- Twelve successful fresh-cache compilations (ten timing/control runs, one
  counts run, one attribution run) produced the identical native image SHA-256
  `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
  Every image was actually executed against the EOF oracle.
- Both the untouched and instrumented producers passed public `omega run
  --both --keep` with exit 0 and **byte-identical native/Terminal stdout**, 463
  bytes. These execution observations are separate from compilation timing.
- The interval sweep has assert-based calibration for overlap and touching
  intervals. Windows process CPU measurement was calibrated with a bounded
  CPU-burning child. The observer context comparison unit test passed
  (1 selected test, 24 skipped); see [observer-test.log](observer-test.log).
- The untouched producer's two fresh-cache wall times were 20.549 and 17.511
  seconds; instrumented collection-off times were 18.336 and 18.760 seconds.
  Observer overhead is unresolved within that host variation. Counting and
  attribution runs are excluded from timing comparisons.

Rejected setup/exploratory observations are retained in
[exploratory.json](exploratory.json). They include a missing initial package
acceptance, an initial compile-only `run --target` invocation caught by the
runtime-output assertion, and the candidate's refusal to consume a lock bound
to the other compiler. A review edit initially changed framing; regeneration
and byte-preserving edits completed ordinary review. The first diagnostic build
hit LNK1181 at a 259-character artifact path; rebuilding through temporary `P:`
succeeded. No failed build was used as a producer.

The initial series used the shared user source cache. Some runs reused prepared
source/check work; others rechecked it, and one cached run correctly failed the
harness requirement for observer events. Its timings are **excluded**. The
earlier 44 ms checked-stage observation described a reused route, not the cost
of full checking. The final series sets `LOCALAPPDATA` only in each compiler
child to a new private directory per run. It changes neither user settings nor
the shared cache. Accepted policy and program source remain fixed.

## Replay and scope

[run.py](run.py) records wall/CPU time, both streams, compiler/profile identity,
lock identity and native image identity, then executes and checks the image.
[private_batch.py](private_batch.py) supplies the bounded case order.
[instrumentation.patch](instrumentation.patch) preserves the diagnostic hooks;
they are removed from the worktree's compiler sources after measurement.
The patch adds no semantic decision and does not modify the source program.

For a new replay, use an isolated checkout at the recorded base, build/freeze
an optimized baseline, apply the patch, and build/freeze the candidate. Use a
short Windows checkout/build path. Place producers under
`build/proof-profile/producers/{baseline,candidate}/omega.exe`, prepare each
producer's lock using normal `omega update` review, and save its `omega.lock`
beside it. Copy the input manifest into `build/proof-profile/` and use new case
labels/private cache directories. Scripts refuse existing result directories.
Fresh producer hashes and accepted-lock hashes are expected for a rebuild;
record them rather than claiming the archived binaries were reproduced.

Compressed original traces and stream/status observations are under [raw](raw/),
with [raw-manifest.json](raw-manifest.json). Analyze them from the repository:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/analyze.py wiki/drafts/measurements/proof_parallelism_2026_10_28/raw
```

The analyzer checks interval partitions and visit/context accounting. It reads
both plain and gzip TSV traces. Large binaries, accepted locks and private
caches remain in ignored `build/proof-profile/`, outside the committed record.

Decision: retain the measurement and reproduction patch; apply no optimization.
Next question: split the synchronous reconciliation/residence region by its
three owners, and attribute the remaining checked-tree flow work before choosing
any cache or parallelization repair. No full corpus, other host, gameplay route,
or global serial-fraction qualification was attempted.

Follow-up: [payload replay](payload_replay/README.md) attributes the remaining membership work and records two rejected optimization candidates.

Follow-up: [edge rebinding experiment](edge_rebinding/README.md) records the next matched A/B and its rejected implementation.

Follow-up: [cache diagnostics](cache_diagnostics/README.md) counts real reuse and measures the rejected cache's costs before any further optimization.

Follow-up: [rebound delta experiment](rebinding_delta/README.md) proves a 95.8% append-allocation reduction, with inconclusive timing and no retained compiler change.

Follow-up: [individual premise reuse](premise_reuse/README.md) measures 68.37% exact fact/edge reuse within replay queries and separates gross retirement/substitution costs; no cache or speedup is claimed.

Follow-up: [unchanged fact sharing](unchanged_sharing/README.md) avoids 98.05% of rebound fact reconstructions without a cache; six alternating A/B pairs leave compile-time benefit inconclusive, so the implementation is parked.

Follow-up: [borrowed fact handles](borrowed_handles/README.md) defers ownership until append and avoids 1,683,529 handle clones versus eager sharing. Six balanced three-producer rounds leave user-visible speed inconclusive; no compiler change is retained.

Follow-up: [remaining scans and joins](scans_joins/README.md) attributes retirement, intersections, publication and repeated shared-fact searches with exact visit counts and explicit profiler limits. No compiler change is retained.

Follow-up: [shared predicate membership hoist](shared_membership/README.md) removes 93.4% of shared comparisons and preserves exact payload maps, but six paired whole-compile observations remain mixed. The compiler change is parked.
