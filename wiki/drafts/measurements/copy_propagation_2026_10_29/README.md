# Existing CopyPropagation: confirmed configured compile-time win

Selecting the existing `CopyPropagation` family lowered the representative
stack32 complete-process median from **796.3 to 457.2 ms (42.6%)** versus its
ordinary unchanged Build. Median CPU fell **43.6%**, and median peak private
commitment fell **15.3 MiB**. All four direct-default blocks improved
(36.2–44.1%). This is an opt-in configuration result on one Windows host,
not a new Rust optimization, default-policy change or parallelism gain.

Inside a root's existing `build(builder: &mut Build)`, the measured recipe is:

```omega
Optimizations::enable(&mut builder.optimizations, Optimization::CopyPropagation);
```

The pass removes redundant scalar forwarding parameters and corresponding
incoming edge arguments in **Psi 06 before Terminal publication**. Fewer
transport positions produce fewer selected registers/copies and spill rewrites.
Bounded spills fell **2,159 to 131** in every profiled off/on comparison.
The [production-route review](result-review.md) corrects the initial research's
legacy Stage 01 attribution and verifies that native allocation policy stays
unchanged. No new equivalence or receipt bypass was introduced.

## Independent comparisons and tradeoff

| Subject | Balanced comparison off → on, ms | Change | Fresh-cache reverse-order confirmation, ms | Change |
| --- | ---: | ---: | ---: | ---: |
| length | 497.1 → 527.5 | 6.1% slower | 470.1 → 497.5 | 5.8% slower |
| dispatch | 474.9 → 462.5 | 2.6% faster | 506.9 → 489.8 | 3.4% faster |
| stack32 | 899.7 → 562.9 | 37.4% faster | 872.9 → 494.8 | 43.3% faster |

Each cohort contains eight complete warm-process observations per flavor and
subject in four symmetric ABBA/BAAB blocks, with fresh per-flavor ordinary-depth
private caches. First visits and profiles are separate. Every valid observation
is retained. The comparison includes a losing stack32 block (5.4% slower);
all four confirmation stack32 blocks improved (41.9–48.4%). Small-program
blocks were mixed, and length consistently regressed. This supports selecting
the family for this transport-heavy workload, not enabling it universally.

The initial two-leg screen independently measured 49.0% stack32 reduction;
it is retained separately and is not pooled with either later cohort.
Stack32 CPU medians decreased 41.1% in the comparison and 45.0% in confirmation;
private commitment fell about 33 MiB versus rollback-off.

Off and on used the **same frozen compiler, private program path and authored
Build**, with only `--disable-optimization CopyPropagation` subtracting the
family in the off leg. To test that comparator explicitly, a final three-arm
cohort used ordinary-empty/default, authored-plus-rollback/off and authored/on:

| Stack32 flavor | Median process, ms | Median CPU, ms |
| --- | ---: | ---: |
| Ordinary unchanged Build, no selection or rollback | 796.3 | 789.1 |
| Same private selected Build, rollback-off | 800.5 | 789.1 |
| Same private selected Build, selected-on | 457.2 | 445.3 |

This cohort used eight observations per flavor in four symmetric six-run
blocks, fresh private caches and separate seeds/profiles. Rollback-off's median
was 0.53% above ordinary default, with equal median CPU; do not claim zero
overhead. Default and off images matched exactly, and all three flavors
executed with exit 0. The direct default/on reduction corroborates the two-leg
result without importing their times into this cohort. Private commitment in
this direct comparison fell 15.3 MiB; its default graph/config is smaller
than the authored-plus-rollback graph, so keep memory denominators separate.

## Identity and quality

Frozen ordinary-O3 producer SHA256:
`9ef68e447263104d505bf03e8109ca1cd24f43d51c49ecb4ad7a7dc4836c6615`,
observed compiler source `244e4de7fabae4c8a2ee43f4002805abe0ec63ee`.
During the recorded experiments, compiler, selected library, Cargo manifests/lock
and toolchain inputs matched the build manifest before and after execution. No rebuild,
PGO/LTO/allocator override or worker-count change was part of this experiment;
all legs used 32 Rayon workers and one native Windows target.
Original authored program/helper bytes and oracles remain unchanged. Private
Build copies add only the existing selection; repository benchmark Builds and
compiler defaults are unchanged. Fresh outputs and actual process exit, error
lines, image presence, image digests and native outcomes are checked.

All **170** performance seed/timed/profile images were deterministic within
policy and executed with exit 0. On/off image bytes intentionally differ:
stack32 images shrink from 81,408 to 8,704 bytes. All profiled warm processes
loaded their graph and recorded a prepared hit. Native image difference is
an optimization outcome, not evidence reuse across policies.

**Ten native/Terminal agreements** passed: nine existing affected facts plus
[the scalar ordering control](scalar-order/main.omg), which checks zero/one/two
rotations, duplicate arguments and both distinct branch arrivals. **Nine authored
fact-sensitive rejects** retained their expected diagnostics. The earlier
source-grounded empty-middle capture shortcut was rejected before compilation:
mandatory edge homes can make scalar swap backedges infeasible. Its exact
[prototype and unbuilt test](unexecuted-edge-capture-prototype.json.gz) are
archived without a test/runtime claim. [Research](research.md) records the
local contracts and pinned LLVM precedent; no such shortcut remains in Rust.

[Unrounded summary](summary.json) and [674 exact source/build/receipt/raw records](evidence.json.gz)
bind 552 generated output records. The initial receipt-spacing failure and
later missing-private-reject-metadata failure remain in the archive. Neither
supplies a successful invalid-control result or an excluded timing sample.
The completed comparison retains observed controller `8a2dc20ceb5`; only quality
metadata lookup and output directories changed before continuation. Producer,
comparison script, timing sources/Builds and raw outputs stayed unchanged;
they were explicitly bound as artifacts before new quality and confirmation.
Successful source-bound observations remain screen `40ad58e4bd5`, quality and
confirmation `23246481983`, and default control `b51cbffc87f`; original compiler
source remains `244e4de7fab`. Receipts, including both failed runs, keep those
identities rather than relabeling them as the result commit or latest main.

The scoped substantial-win target is met and retained in this record. No full corpus,
foreign-host runtime, cold-cache, latest-main compiler or universal speedup is
claimed. [PR864](https://github.com/CathedralOS/Omega/pull/864) already landed
the prior custody optimization; this configured result is a separate experiment.

Publication preserves the original experiment revisions and producer identities.
These records are historical observations, not a benchmark of the publication
commit or a new compiler/default-policy change.
