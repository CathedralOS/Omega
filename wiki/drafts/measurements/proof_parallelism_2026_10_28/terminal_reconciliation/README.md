# Dungeon: reference graphs are constructed before shared-reference demand

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared host. Continues [full compilation attribution](../full_compilation/README.md)
and reuses the earlier [reconciliation split](../reconciliation/README.md).
**Decision: retain attribution; select deeper lazy graph initialization as the
next experiment. No compiler optimization or speedup is retained.**

Six fresh-output observations (three cold, three reusing their own cold cache)
show that payload membership transport accounts for almost all of the grouped
reconciliation/residences region. Reference-result helper time is almost
entirely three full borrowed-storage graph builds. A separate typed-demand
counter confirms that the calls triggering those builds have no shared-reference
leaves. This identifies premature preparation without weakening a proof rule.

## Disjoint owners

The six timing observations span different host loads. Ranges are descriptive;
their endpoints must not be added. Each raw membership column partitions its
own enclosing elapsed duration exactly.

| Region | Observed elapsed range |
| --- | ---: |
| Place-declaration reconciliation | 0.156-0.866 ms |
| Descriptor residence pass, early-exit route here | 0.066-0.125 ms |
| **Payload membership transport** | **2,044.60-3,822.71 ms** |

Inside membership transport:

| Disjoint part | Observed elapsed range |
| --- | ---: |
| **Source-contract replay queries** | **1,338.90-2,648.78 ms** |
| **Reference-result queries** | **466.72-770.93 ms** |
| Type clone, ordinary graph and analysis preparation | 147.13-459.04 ms |
| Operand queries | 30.86-49.45 ms |
| History destruction | 13.12-22.80 ms |
| Unassigned loop/logging/bookkeeping remainder | 17.20-32.35 ms |
| Other individually timed operations | each below 11 ms |

Reference-result queries split further: **466.43-770.46 ms** constructing graphs,
and only **0.286-0.477 ms** outside construction. Construction accounts for
**99.92-99.94%** of that query interval and **18.44-25.91%** of the enclosing
membership interval in the respective runs. This nested split is already
included in the table. It is not another cost to add.

Each timing observation constructs exactly **three** reference-context graphs,
each containing **202 machines**. There are **127** lazy context constructions,
not 127 graph builds. The separate ordinary graph prepared for source replay
is a fourth construction owner and is not counted in those three.

## What the demand counter establishes

The first producer times the existing `OnceCell` initializer. A second producer
adds metadata observations after that timer closes, leaving all real selection,
validation, construction and replay unchanged. Its cold and warm observations
agree at every initializer:

| Graph-initializing call | Result mappings | Shared-reference leaves | Projected parameter candidates on those leaves |
| --- | ---: | ---: | ---: |
| 1 | 1 | **0** | **0** |
| 2 | 1 | **0** | **0** |
| 3 | 1 | **0** | **0** |

`ReferenceResultMemberships::for_operation` binds result origins and initializes
the full borrowed-storage graph before calling
`reference_call_preserved_memberships`. That operation validates the mappings
and typed leaves, then only shared-reference leaves can reach a backing query.
The three initializing calls reach no such leaf. All four source-detail traces
also record zero completed payload-history replays inside reference-query
windows. Zero replay alone would have been insufficient: an unresolved backing
query could stop before starting history, which is why the typed counter was
added.

**Scope limit:** the metadata counter runs at each initializer, not at later
calls sharing that initialized context. It establishes that initialization is
premature for the triggering calls. It does not prove every graph is unused
for the entire context, count how many builds a lazy implementation would
eliminate, or establish a 0.47-0.77 s whole-compile saving. A future A/B must
measure the actual removed builds and user-visible effect.

## Next experiment and correctness boundary

Move graph/analysis preparation to the first supported projected shared-referent
query **inside the common reference-membership operation**. Keep carrier
agreement, duplicate mapping detection, typed path formation and origin
validation in their existing order. Preserve all candidates and their
intersection. An empty or malformed mapping must retain its existing result or
diagnostic; a new early success based only on the result type is not equivalent.

The receiving result annotation remains masked at every dynamic occurrence,
including loop backedges. Reference queries observe normal completion after
effects and postconditions. Source-copy queries observe before-operation state.
Those cuts and masked source inputs cannot share an expression-only cache.
No cross-sweep graph cache is needed for this experiment. New operation
membership rows can affect graph adaptation, so equal machine/operation IDs
alone would not justify one.

Promotion requires fewer actual graph builds, unchanged exact outputs and
observable compile-time improvement under matched conditions. Exercise positive
shared-reference preservation, exclusive restoration, mixed origins and
receiving-annotation loop self-justification; retain rejects such as
`tests/omega/fail/borrows/returned_reference_missing_membership/main.omg` and
`returned_reference_invalidated_membership/main.omg`. These are future
implementation checks, not qualification claimed by this profiling record.

Source replay remains the larger owner. Within its four detail observations,
edge processing including joins takes **1.033-1.450 s**, pre-operation scalar
retirement **0.146-0.206 s**, and incoming-state cloning **0.046-0.101 s**.
The nested join interval is **0.281-0.412 s**, included in edge processing.
The earlier cache/sharing/borrowing/hoist/fusion experiments already addressed
parts of this mechanism without establishing a repeatable whole-compile gain.
They are not retried here. Deeper graph laziness is the smaller correctness
change to evaluate before another proof-state representation or cache.

## Cold/warm behavior and controls

| Observation | Process wall, s | Process CPU, s |
| --- | ---: | ---: |
| Original A / B | 31.601 / 19.670 | 40.250 / 26.359 |
| New hooks disabled A / B | 29.175 / 17.172 | 34.641 / 23.516 |
| Cold outer-only A | 29.701 | 35.938 |
| Warm outer-only A | 14.359 | 13.734 |
| Cold detail B / C | 30.738 / 17.594 | 37.266 / 23.812 |
| Warm detail B / C | 7.484 / 7.661 | 7.422 / 7.531 |

Each warm observation emits `SOURCE_GRAPH_CACHE loaded` once and `prepared_hit`
twice, reusing only its paired cold private cache. Cold observations emit one
`cold` marker. Four counter observations (cold/warm on each probe producer)
have identical semantic work rosters: 109 candidate machines, 127 sweeps,
18 changing sweeps, 3,313 operation visits, 418 eligible/reference queries,
173 payload-target queries and 439 proposed updates. Proposals are not newly
established proof obligations. Detail observations retain 418 source query
windows, 173 replay results, 96 entry initializations, 15,219 positions and
17,444 successor edges.

Warm B/C membership transport takes 2.127/2.045 s, versus cold B/C
3.101/2.182 s; warm A takes 3.823 s versus cold A 3.618 s. The previous
full-compilation cohort's warm reconciliation penalty does not repeat
consistently. Same work counts do not prove equal internal allocation state,
and these ordered, shared-host observations do not establish a cache defect
or an uninstrumented CPU cost. No valid slow observation is discarded.
Counters and shared-cache engine comparisons are excluded from timing ranges.

The old general phase profiler is disabled. Local timers and thread-correlated
source/reference windows avoid its global Mutex. Within each query, nested
edge/join/closure/publication timers are not added twice. Formatting, output,
extra clocks, code layout and scheduling still perturb these diagnostic spans.
Original/off ranges do not isolate a precise probe-overhead correction. Whole
process CPU is separate from wall time and does not establish a serial fraction.

## Identity, behavior and archive checks

All 16 observations use the unchanged Dungeon EOF program and its 203 authored
inputs from the [fixed manifest](../input-manifest.json). Fixed semantic Rust
base is `456c5e984452a45d977295e254b3ca9ae3cc7750`; measurement patch base is
documentation descendant `9901ccc4146b00fdcc4e4280be12fd57a79ceab5`.
[identities.json](identities.json) records each optimization-level-1 executable
and its normal producer-specific accepted lock. Timing producer SHA-256:
`20518efd27fcb0afce5d940f050a2f576883d98d5c073f3983932c463fbb410f`;
demand producer:
`76fa5c6e6df8b2128fda5d7b5691d3f2471609b9a32334018ebffe760bcc214c`.
No review/consent gate or lock policy was edited manually.

Every compilation exits 0. All 14 ordinary native images were actually run
against EOF, exit 0 and print the authored 463-byte output including
`A bottomless dark room near the dungeon heart.` Every image has SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
Two explicit `omega run --both --keep` observations confirm byte-identical
Terminal/native stdout and that same retained image. These are behavior checks,
not cold-cache performance observations.

The timing build passed 22 focused payload tests (6 filtered) and one probe
bookkeeping test (6 filtered). Both optimized CLI builds succeeded; the demand
refinement was then exercised by its real cold/warm program and engine checks.
Both complete [timing](timing.patch) and [demand](instrumentation.patch) patches
apply independently to the recorded base; scratch-index application checks
passed. All eight temporary source files were restored/removed afterward, and
the compiler tree matches the fixed semantic base. Shared main was untouched.

The initial outer-only observation compiled and executed successfully, then a
harness postcondition incorrectly required source-detail windows while that
mode was disabled. Corrected the mode assertion and finalized those existing
raw bytes without rerunning or excluding the observation. The initial failure
log and [resume script](resume.py) preserve this distinction.

Replay archive verification from the repository root:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/terminal_reconciliation/analyze.py
```

The analyzer checks every manifest digest, all 16 records, producer/lock and
program identities, disjoint/nested accounting, cold/warm cache markers, matching
counter rosters, graph/demand rows, actual native outcomes and both-engine
agreement. [summary.json](summary.json) separates all cohorts.
[Raw streams](raw/) and compressed build/run/update logs preserve provenance.
Large producers, accepted locks and private caches stay ignored under
`build/proof-profile/`. [Research](research.md) includes the prior experiment
reuse, exact new primary-source queries, applicability and stopping rule.

No production optimization, full corpus, new invalid-source qualification,
non-EOF gameplay or other host/target was run. The next bounded task is the
lazy-initialization implementation and matched A/B, not another claim from
these attribution timers.
