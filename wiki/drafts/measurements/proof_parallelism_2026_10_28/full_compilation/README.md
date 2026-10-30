# Full compilation: three regions account for 77% of Dungeon compile time

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared host. Continues [handle-copy attribution](../handle_copy_cost/README.md).
**Decision: retain attribution evidence; no compiler optimization is applied.**

The final three fresh-private-cache observations take **16.04-17.53 s**.
Checking, frontend preparation and Terminal lowering account for 77.0-77.5%
of their respective complete process wall times. The earlier handle-copy
materialization measurement was only 16.87-18.87 ms. There are larger owners
to investigate before another handle-copy optimization.

## Disjoint complete-compilation attribution

These are elapsed wall regions in the final refined cohort, not scheduled CPU
costs or an intrinsic serialized fraction. Each column partitions the traced
CLI lifetime exactly. Process time outside that lifetime is shown separately.
Parent/child scopes and parallel workers are not added twice.

| Region, seconds | Refined A | Refined B | Refined C |
| --- | ---: | ---: | ---: |
| Typed preparation and checking | 6.119 | 5.668 | 5.962 |
| Source/frontend preparation | 4.115 | 3.283 | 3.532 |
| Terminal lowering | 3.357 | 3.401 | 3.416 |
| Other compiler/facade work, including settlement | 1.517 | 1.407 | 1.525 |
| Native production | 0.981 | 1.006 | 0.991 |
| Retained graph persistence | 0.726 | 0.709 | 0.725 |
| Source discovery and Build execution, exclusive | 0.327 | 0.238 | 0.251 |
| Native publication | 0.062 | 0.061 | 0.059 |
| Project preparation | 0.038 | 0.029 | 0.027 |
| Retained graph restore requests (cold misses) | 0.019 | 0.015 | 0.015 |
| Other CLI work | 0.001 | 0.001 | 0.001 |
| Process wall outside traced CLI | 0.268 | 0.220 | 0.215 |
| **Complete process wall** | **17.532** | **16.037** | **16.721** |
| Whole-process CPU, not additive to wall | 22.609 | 22.750 | 23.562 |

The archived raw category key `package orchestration and cleanup` is historical.
It means the residual under application/compiler facade scopes after known
children are removed. The refined trace places 1.325-1.443 s of it inside
`orchestration.compile_packages`. Admission, completion and returned-Build
settlement together take only 29-35 ms. That returned-Build settlement is
distinct from the later checked-to-selected execution settlement. The residual
includes untraced compiler work such as that later settlement and object
disposal; it is not evidence of 1.5 seconds of package administration or cleanup.
Implicit field disposal after a Rust Drop hook is not independently bracketed.

## The larger named operations

Children below are clipped to their disjoint parent category cells. They are
parts of the preceding table, never extra costs to add to it.

| Owner/operation | Final cold range |
| --- | ---: |
| Frontend: pre-resolution evaluation | **2.182-2.906 s** |
| Frontend: resolve and type an extension | 0.612-0.747 s |
| Frontend: activation selection | 0.233-0.242 s |
| Frontend: complete Build bodies | 0.187-0.242 s |
| Checking: construct check facts | **1.672-1.707 s** |
| Checking: finalize execution identities | **1.356-1.371 s** |
| Checking: check facts/contracts | 1.074-1.092 s |
| Checking: prepare typed trees | 0.789-1.057 s |
| Checking: prepare typed program | 0.385-0.508 s |
| Checking: selections/custody/types/boundaries | 0.344-0.353 s |
| Terminal: reconciliation and residences | **1.834-1.872 s** |
| Terminal: lower selected machines | 1.519-1.549 s |

Frontend preparation here includes `evaluate_pre_resolution`, not just parsing
or name lookup. Its existing constant-initializer and constant-expression scopes
account for much of that region; their inclusive aggregates are retained in
stderr, not presented as another additive partition. Retained-program cloning
is only 6-8 ms. The large frontend block is not explained by that clone.

Within check-fact construction, the existing flow fixed-point root is about
1.00-1.02 s, including permission-event preparation. Within execution identity
finalization, candidate-body work is about 0.919-0.934 s. These nested scopes
are possible follow-up owners, not additional checking costs. Every raw scope
and call count is available for inspection. This experiment does not establish
which of their visits are redundant or safely removable.

Retaining the source graph serializes **230,268,241 bytes**: preparation
85,024,175, sources 27,362,321, checks 117,881,745. Encoding the three records
takes 0.489-0.494 s; staging, hashing and writing takes 0.204-0.218 s. The
hash/write-digest child is included in that latter span. These are serialized
record sizes, not proof of an equal physical disk-I/O volume. Native production
is about one second; fine native children are nested and not added to its root.

For an incremental compile, investigate **Terminal reconciliation/residences**
first: it survives the retained-graph fast path. For a cold compile,
**pre-resolution evaluation** is the largest named single operation here.
Either follow-up needs exact visit/input denominators and a bounded matched
experiment before changing caching, ownership or proof rules. No new candidate
is implemented by this profiling assignment.

## Warm retained graph is a separate condition

Two refined reruns reuse only refined-A's private cache, with fresh output
directories. Both observe `source.graph.loaded=1` and two prepared hits, with
no retained-graph rewrite. They take **7.173 and 7.343 s** total:

| Warm region | Range |
| --- | ---: |
| Restore retained graph | 0.811-0.826 s |
| Checking, mostly the restore fast path | 0.022-0.023 s |
| Terminal lowering | **3.988-4.082 s** |
| Its reconciliation/residences child | **2.543-2.556 s** |
| Its selected-machine lowering child | 1.440-1.522 s |
| Native production | 0.966-0.979 s |
| Other compiler/facade work | 0.945-0.955 s |

All three refined cold and both refined warm traces emit 110 composed-body
scopes. Matching that count does not establish matching internal work or explain
why warm reconciliation is slower. The earlier coarse warm observation was
10.766 s with 6.788 s lowering. Preserve it as an earlier observation, not an
outlier to discard or evidence for a causal explanation. Cache reuse skips much
of the front half; it does not skip Terminal lowering or native production.

## Measurement controls and uncertainty

All **22 observations** are retained: three preflight observations, nine rotating
original/off/on coarse observations, a coarse warm observation and engine
comparison, then eight refined observations (off, three cold on, original, two
warm, engine comparison). They are not pooled into a speedup claim.

The pure-original fresh-cache observations span **16.149-28.765 s**. Coarse probe
off spans 23.387-26.403 s and on spans 17.766-24.670 s. In the refined cohort the
off control is 16.508 s, original is 16.149 s, and on spans 16.037-17.532 s.
Host variation and measurement order prevent estimating a small net probe
overhead. The trace ranks coarse regions; it is not an uninstrumented CPU
profile or a prediction that removing a region saves its entire elapsed time.
No benchmark run overlaps another build launched by this session.

Instrumentation uses one monotonic epoch and start/end thread IDs, selected
coarse scopes, no retained semantic owners, and rare cache counters. The old
fine-grained Mutex collector is disabled in coarse/refined traces. Only the
preflight `full-detail-a` enables it; its nested/concurrent aggregates remain
diagnostic hints. The analyzer sweeps intervals, assigns innermost categories,
retains non-nested overlaps explicitly, and then partitions child detail inside
those cells. No cross-category overlap occurs in these observations. Synthetic
assertions cover nesting, clipping, touching boundaries and partial overlap.
Whole-process CPU/wall ratios cannot establish the compiler's serial fraction.

## Identity, behavior and replay

Fixed semantic source is `456c5e984452a45d977295e254b3ca9ae3cc7750`;
the patch base is documentation descendant
`3c5a4ab0052af2f3040cd38b9beede83b038e623`. All 203 authored inputs match the
existing [input manifest](../input-manifest.json). Every ordinary compilation
uses a fresh private cache unless explicitly marked warm. Accepted locks were
produced through normal offline `omega update`; no trust/consent edits.

The original, coarse and refined producers have their own accepted lock digests
and verified optimization level 1 in [identities.json](identities.json) and
[refined-identity.json](refined-identity.json). Their executable SHA-256 values:

```
original e6142460b46c531e6e2e0ada4382663fe42353a12663f60a3f08e0909dbbddb5
coarse   e3a518bf53a9a8895065b1e3dcc3e9048fee7c3a8c03df05a24d60590a381635
refined  4d41227e58f0be09bf4b76cb15d758c9d10d4c8ecad69bbe5f81f3e4e20744dd
```

Every compile exits 0. Every ordinary native image is actually executed with
EOF, exits 0 and prints the 463-byte authored result including
`A bottomless dark room near the dungeon heart.` All native images have digest
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
Two explicit `omega run --both --keep` observations confirm byte-identical
Terminal/native output and the same image. Those shared-cache comparisons are
behavior validation, excluded from cold timing comparisons.

Replay the archive checks from the repository root:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/full_compilation/analyze.py
```

The analyzer verifies file digests, producer/lock identities, all 22 status and
oracle records, exact partitions, cache markers and retained byte counts.
[summary.json](summary.json) retains every cohort separately.
[Raw records](raw/) include stdout, stderr, process/native results and complete
timeline events; compressed logs retain both builds and all run schedules.
[instrumentation.patch](instrumentation.patch) and [coarse.patch](coarse.patch)
apply independently to the recorded base; both were checked with a scratch Git
index. Apply only one to rebuild the associated diagnostic producer. Runners
reuse the prior fixed-workload harness. Build through the repository build-slot
wrapper with `mbx build --locked -p omega`, optimization enabled. Binaries,
private caches and accepted locks remain ignored, not archived as source.

All nine temporarily instrumented Rust files were restored after collection;
the final compiler tree matches the fixed semantic base. One focused
phase-profile test passed before the first build, and both instrumented CLI
builds succeeded. This read-only attribution makes no language, proof, lowering
or backend change, and does not claim new capability qualification. No whole
corpus, non-EOF gameplay, other target/host, or precise serial-fraction
measurement was performed. No optimization is retained or speedup claimed.
