# Eager and lazy graphs in one compiler binary

**Removing the separate-executable confound does not establish a compilation
speedup.** Cold wall time is essentially tied, while lazy mode uses more CPU in
5/6 cold blocks (2.82% median penalty). Warm wall improves in 4/6 pairs, with a
4.92% median reduction. Same-mode variation is substantial. The cold CPU warning
remains visible; the optimization and diagnostic selector stay out of production.

This follows [cold variation controls](../cold_variation/README.md) and uses the
same Dungeon EOF program and 203 unchanged authored inputs. It changes only when
one frozen executable constructs its reference graphs.

## The controlled difference

Both modes share the same code image, common membership kernel, monomorphizations,
producer profile, accepted lock and source input roster. A diagnostic environment
selector is read after the existing actual-binding/formation checks. Eager mode
forces the existing OnceCell graph provider at the original initialization site;
lazy mode leaves it to the first real backing query. Both use the same lazy
invocation-local analysis setup. This isolates graph initialization rather than
reconstructing every detail of the old eager producer.

Carrier/mapping/path/extent/origin checks, receiver masking, normal completion,
all-origin intersections and the graph-supplied public API remain intact. There
is no cross-sweep cache, source recognizer or new authority rule. The exact
[diagnostic patch](diagnostic.patch) is preserved; the working compiler source
was restored before measurement. No runtime mode switch is retained in source.

Separate counter runs confirm actual demand in this binary:

| Cache | Eager graph builds | Lazy graph builds | Ready calls per mode |
| --- | --- | --- | --- |
| Cold | 3 x 202 machines | 0 | 4 |
| Warm | 3 x 202 machines | 0 | 4 |

Counters are disabled in all timing observations. Their elapsed times are
excluded; fewer builds alone is not a speedup claim.

## Whole-compilation comparison

Six cold four-run blocks use E/L/L/E or L/E/E/L, three of each in seeded shuffled
order. Both modes have equal mean positions, cancelling linear drift, and each
block supplies one eager/eager and one lazy/lazy repeat. After each block, one
warm pair runs in balanced E/L or L/E order, each reusing its mode's last cold
cache. All 24 cold and 12 warm observations are retained without exclusions,
retries or extra unchanged trials.

Every cold run has a new private compiler cache/output directory; every warm run
has a new output directory. OS file and CPU caches are not flushed. No own builds
overlap timing and no user processes, priorities or affinity are changed.

Positive reductions mean lazy faster. Cold rows compare within-block means;
warm rows compare the two runs. Summary percentages are medians of those effects,
not ratios of global medians.

| Cache / metric | Lazy faster | Median reduction | Effect range |
| --- | --- | --- | --- |
| Cold whole wall | 3/6 blocks | 0.04% | -11.36% to +14.77% |
| Cold whole CPU | 1/6 | **-2.82%** | -4.37% to +10.68% |
| Cold checking wall | 3/6 | -4.57% | -16.16% to +9.49% |
| Cold lowering wall | 4/6 | 3.54% | -2.84% to +37.44% |
| Warm whole wall | 4/6 pairs | 4.92% | -46.60% to +11.83% |
| Warm whole CPU | 4/6 | 4.37% | -44.99% to +10.98% |
| Warm lowering wall | 3/6 | 1.12% | -53.74% to +12.08% |

Every cold block, with mean wall times in seconds:

| Block | Order | Eager | Lazy | Wall reduction | CPU reduction |
| --- | --- | --- | --- | --- | --- |
| 0 | LEEL | 20.0437 | 22.3202 | -11.36% | -2.95% |
| 1 | LEEL | 34.3762 | 29.2995 | +14.77% | +10.68% |
| 2 | ELLE | 29.3079 | 31.2141 | -6.50% | -3.16% |
| 3 | LEEL | 21.3668 | 21.1696 | +0.92% | -1.84% |
| 4 | ELLE | 23.4599 | 22.8676 | +2.52% | -4.37% |
| 5 | ELLE | 25.0899 | 25.3018 | -0.84% | -2.70% |

Every warm pair, wall times in seconds:

| Pair | Order | Eager | Lazy | Wall reduction | CPU reduction |
| --- | --- | --- | --- | --- | --- |
| 0 | EL | 8.3512 | 7.3635 | +11.83% | +10.98% |
| 1 | LE | 14.0779 | 14.4375 | -2.55% | -1.90% |
| 2 | LE | 11.5571 | 11.1399 | +3.61% | +3.71% |
| 3 | EL | 9.5047 | 8.7365 | +8.08% | +7.69% |
| 4 | EL | 8.2225 | 7.7104 | +6.23% | +5.04% |
| 5 | LE | 12.0612 | 17.6821 | -46.60% | -44.99% |

The slow final warm pair is valid and remains included. Host demand differs,
but this record does not assign that run's exact cause.

## Variation and the remaining CPU warning

Twelve identical-mode cold repeats have median symmetric absolute differences
of 14.96% wall, 9.02% CPU, 11.26% checking and 12.99% lowering. Wall variation
reaches 31.67%; checking variation reaches 48.61%. The symmetric measure is
`100 * abs(second - first) / mean(first, second)`, not a confidence interval for
the block effect. Raw individual runs and every repeat are in [summary.json](summary.json)
and [raw](raw/).

For example, unchanged lazy runs take 26.49 and 19.25 seconds, with checking at
9.93 and 6.04 seconds. Checking precedes the selector. The same executable and
accepted lock can therefore exhibit the mixed pattern without different static
producer layout. This does not prove the old separate-binary penalties were
entirely layout effects, entirely host noise, or harmless.

Non-target CPU demand spans 22.3-96.3% of the 32 logical processors' capacity
across observations. These are aggregate accounting observations, not scheduler
ready time or attribution to a named process. Page faults include soft/hard
faults; peak pagefile usage is commit, not swap traffic. Elapsed sampler calls
reach 3.29% of wall; that includes descheduling, is not calibrated observer
overhead, and is not subtracted. The [preceding API research](../cold_variation/research.md)
documents these counter contracts.

Cold CPU is worse in 5/6 blocks even after removing the separate-executable
confound. That is a material possible cost, not dismissed as noise. Process-total
CPU does not locate it before versus after the selector, and the checking/control
variation prevents a clean causal assignment to graph removal. No zero-regression
bound or repeatable whole-compilation gain is established. The original acceptance
is unchanged and the candidate remains parked.

The next useful discriminator would record process CPU at checking/lowering
boundaries within this same-binary experiment, using lighter endpoint accounting
or scheduler evidence if needed. More unchanged wall-time repetitions on this
saturated host are not justified by these results.

## Behavior, identities and replay

The optimized build passed 24 focused tests (six unrelated skipped). Each mode
matches the retained semantic baseline across four positive fixtures executing
in both engines and three invalid controls. All four corpus diff-gate invocations
return **1** for the same existing missing-positive/changed-diagnostic golden
discrepancies; no golden was recorded and no diff gate is called green.

All 42 Dungeon observations succeed: 36 timings, four separate count runs and
two explicit both-engine comparisons. All 40 ordinary compiled images were
actually executed; the two comparisons each run native and Terminal engines.
Native exit is 0, all 463 stdout bytes match the previous experiment, and every
native image has SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.

There is exactly one CLI digest,
`f9937869a10f89a4de04d4dd70be13bf89db74c3f4eb3f36a91b327ca9c58df9`,
and one accepted-lock digest,
`ec7b832157f461d51da0c7b8c94b867610573bfab546a0be18ce1050f7435cfb`.
Both CLI and corpus profiles report `opt_level=1`.
[Producer identity](producer.json), [source identity](source.json) and
[patch replay checks](patch-checks.json) bind the build to base
`1c358b5fd09bb57a45081cc707c052fbe6c2dad3`, semantic base
`456c5e984452a45d977295e254b3ca9ae3cc7750` and the exact diagnostic patch.
Normal offline `omega update` prepared the accepted lock. The input manifest's
historical baseline-lock row is excluded from authored-source checks; the common
current lock is verified separately. No lock is manually edited.

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/same_binary_graph/analyze.py
```

The analyzer checks the file manifest, exact one-producer/lock identities, source
and input bindings, actual graph counts, every native/comparison outcome, warm
cache ancestry, telemetry accounting, every block/pair/repeat and exact focused
quality outcomes. [research.md](research.md) records the predeclared scope,
research reuse, stopping rule and decision. Scripts and compressed build/run
streams are retained. Binaries, accepted locks, caches and build trees remain
ignored local evidence. Execution replay requires the recorded frozen producer
or an explicitly identified rebuild and fresh case labels.

Compiler sources are restored; only this evidence record is retained. No full
corpus, other host/target, alternate gameplay, isolated-machine qualification,
scheduler trace, production optimization or intrinsic serial fraction is claimed.
