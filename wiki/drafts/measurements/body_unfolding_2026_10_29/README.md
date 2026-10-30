# Body unfolding: guard-query operand reuse rejected below target

The bounded iteration found that scalar guard proving accounts for nearly all
body-unfolding time on the unchanged operand-order first-preparation workload.
A query-local operand reuse candidate reduced the observed mean from 116.82 to
106.00 seconds (9.26%). Its ranges separated, but it missed the predefined 10%
benefit threshold. The candidate and diagnostic instrumentation were removed;
production compiler source is unchanged from the fixed base.

## Measured comparison

Fixed base: `c38d69bb14f8773414312e15388f7776ef966379`. Its compiler/library/toolchain
inputs match the preceding published operand-resolution producer; the intervening
commit contains only experiment records. Optimized dev producers and patch hashes
are recorded in [identity.json](identity.json). The candidate build took 3m30s.

| Order | Producer | Fresh first update (seconds) |
| --- | --- | ---: |
| 1 | A, baseline | 116.640684 |
| 2 | B, query-local reuse | 105.229664 |
| 3 | B | 106.773487 |
| 4 | A | 117.005544 |

A mean: 116.823114 s. B mean: 106.001575 s. Reduction: 9.2632%.
No valid trial was excluded. Each trial uses fresh independent project/cache
roots, unchanged root/Build inputs and ordinary package review/resume. Profiling
is disabled for this comparison. All 106 std inputs and the workload are pinned
in `timing-inputs.json`; timing and status observations are in `timings.json`.
The host was shared with other compiler work. These four trials do not establish
a universal percentage, tail-latency bound or warm-cache improvement.

All four native images are byte-identical, SHA256
`7ccbfb89151d286167c97d51c39092b03e33ccf79762dbd9802205578209eb66`.
Each separately executed with exit 0, and each actual host `run --both --keep`
reported native exit 0 and interpreter exit 0. An explicit target flag was not
used for those engine comparisons.

## Attribution and mechanism

The first diagnostic measured 150.047 s inclusive in unfolding and 147.954 s
in scalar guards; setup was 1.302 s and term construction 1.048 s. The finer
diagnostic attributed 124.655 s inclusive to integer observation preparation,
3.160 s to predicate normalization and 0.052 s to structural equation judgment.
Scopes overlap and include recursion: they are not additive savings or exclusive
CPU measurements. Both diagnostic preparations completed ordinary review and
actual host native/Terminal execution. Their differing instrumentation and host
conditions make their elapsed times unsuitable for a speedup comparison.

The candidate enabled the existing exact-payload operand-answer vector inside
`integer_predicate_observations`. Goal and premise projection share one immutable
judge, so repeated operands could reuse completed depth-zero resolutions. Each
occurrence still registers its own atoms, carrier facts and operation formation
obligations. Nested queries own separate observation records. No conclusions,
cross-query answers or altered recursion limits were introduced. The exact
five-added/two-removed-line patch is retained in `candidate.patch` for inspection.

## Correctness and stopping point

After the performance screen failed, the candidate ran the existing
`selected_contract_equation_reentry_exit` control with separate native exit 0
and actual native/Terminal agreement. Its reject twin still reported the
authored unproved selected requirement. Together with the four timing trials,
this gives five native observations, five actual host comparisons and one
focused refusal. The two diagnostic host comparisons are separate observations.
The control's unnecessary `update --resume` returned 1 because no review was
pending; subsequent compile and actual executions succeeded. Raw logs preserve it.

The wider guard/snapshot/formation suite and Rust isolation unit test were
deliberately skipped after the failed performance screen. No full corpus,
other-host execution, independent semantic review, new language repair or
native publication qualification is claimed. No source or fixture changes are
retained. The reusable build binaries and their PDBs remain under ignored
`build/body-unfolding/`; this directory retains patches, scripts, inputs' digests,
build logs and complete case transcripts.

The next question is whether repeated integer premise projection and admission
can be prepared once for an exact immutable guard context. Measure the work and
reuse frequency first; do not broaden operand-answer lifetimes without retaining
storage epochs, active recursion context and every child-formation obligation.
The present result does not justify optimizing body environment construction.

## Replay and provenance

[research.md](research.md) records the premeasurement decision rule, source review,
online research and diagnostic findings. The diagnostic patches are separate
from the uninstrumented candidate patch. Build scripts assert the exact source
diff before and after their chains. Every local build uses this worktree's own
Cargo target and the repository build-slot hold; waits are excluded from compiler
timings. Producers report optimized `opt_level=1` bound to their binary digests.

At the fixed base, apply the selected recorded patch and use the corresponding
build/profile script with fresh output roots. Scripts retain the original Windows
paths, so relocate them consistently when replaying elsewhere. `quality.py`
records its conditional suite explicitly: only the two controls listed above ran
because `performance-screen.json` is false. Correctness cases reuse the candidate
trial's package cache; those durations are not performance evidence. The prior
rejected empty-equation shortcut is absent from every producer in this iteration.
