# Remaining first-time checking: rejected empty-equation shortcut

Rejected on fixed source `0ac181bf065e398d6d6ae455c2659416895d6b49`.
Skipping unproductive specification-result guarantee queries reduced the observed
first-update mean by 8.09%, below the 10% threshold declared before timing.
The sixteen-line candidate and diagnostic instrumentation were removed from
production source. Their patches remain here as experiment evidence.

## Matched measurement

The unchanged `pass/calls/runtime_numeric_operand_order_exit` workload used
separate fresh project/cache roots, ordinary package update/review/resume, and
optimized producers with profiling disabled for A/B. All four trials are retained:

| Order | Producer | First update (seconds) |
| --- | --- | ---: |
| 1 | A, published operand-resolution baseline | 133.178849 |
| 2 | B, baseline plus candidate.patch | 130.567961 |
| 3 | B | 122.301691 |
| 4 | A | 141.946432 |

A mean: 137.562640 s. B mean: 126.434826 s. Ranges separated, but the useful-gain
screen failed. Two observations per variant on a shared host do not establish a
general speedup. Warm-compilation ranges overlapped; no warm speedup is claimed.
Queue waiting is excluded from these compiler timings.

All four native images have SHA256
`7ccbfb89151d286167c97d51c39092b03e33ccf79762dbd9802205578209eb66`.
Each separately executed with exit 0, and each actual host `run --both --keep`
reported native and Terminal interpreter exit 0. No `--target` argument was used
for those execution comparisons.

## Hypothesis and diagnostic

The detailed diagnostic found 224,375 non-Boolean specification guarantee
queries without expression ensures. Their prerequisite proving accumulated
28.84 seconds inclusive. The candidate returned no normalization answer for
that restricted result kind, preserving captured runtime results, Boolean body
equations, source formation and ordinary body unfolding. See [research.md](research.md)
for the premeasurement reasoning and acceptance rule.

Body unfolding accumulated 128.05 seconds across 1,388,873 entries. This is the
next investigation lead: identify repeated unfolding work under the same proof
context and measure its exclusive contribution before selecting another change.
Recursive inclusive scopes overlap and cannot be summed or treated as removable
elapsed time. Active-question bookkeeping (1.326 s) and Boolean body discovery
(0.011 s) are smaller leads on this workload.

## Correctness observations and harness correction

The focused formation pair uses a restricted callee with no ensures. The positive
establishes its prerequisite before using its result in a specification contract;
the negative removes only that prerequisite. Both producers ran the positive
with actual native/Terminal exit 0 and rejected the negative with
``cannot prove requires contract for specification call `restricted` ``.

The original quality harness ran the candidate positive, then failed creating
the negative's input directory because pass and fail fixtures shared a basename.
This was a harness failure, not a compiler rejection. `quality.json` records only
the completed positive. `formation-replay.py` corrected directory identity to
include variant and pass/fail, then ran the candidate negative, baseline positive
and baseline negative; all assertions passed. See `formation-replay.json` and
the corresponding raw transcripts. The positive's unnecessary `update --resume`
returned 1 because no pending review existed; compilation and both engines then
succeeded. Do not count that resume status as a semantic failure.

The planned broader positive/refusal suite was skipped after the performance
screen failed. No full corpus, other-host execution, new fact-row closure or
publication qualification is claimed for this rejected candidate. The temporary
formation fixtures are archived under `inputs/` rather than retained as an
unrelated permanent coverage change.

## Replay inputs

`identity.json` binds the fixed source, producer digests/build profiles, candidate
and diagnostic patch hashes, and focused-control inputs. `timing-inputs.json`
pins the unchanged workload and library inputs. `timings.json` retains every
trial; `performance-screen.json` records the decision. Phase profiles and raw
case transcripts are retained separately from uninstrumented timing evidence.
Optimized executables and caches remain locally under `build/` and are not
committed.

To repeat at the fixed checkpoint, restore `inputs/pass/*` and `inputs/fail/*`
into their paths recorded in `identity.json`. The retained scripts record the
original Windows worktree/build paths and require equivalent fresh output roots.
Use the recorded patches to reproduce producers; `diagnostic.patch` is
authoritative, because `instrument.py` predates the final non-Boolean split.
`formation-baseline.py` was generated but never executed; use the corrected
`formation-replay.py` for the boundary replay. Build/test chains use the shared
build-slot hold and this worktree's own Cargo target directory.

## Publication of the preceding retained optimization

[PR #849](https://github.com/CathedralOS/Omega/pull/849) publishes operand-resolution
reuse, stacked on #822. Manually dispatched compiler admission
[40988721734](https://github.com/CathedralOS/Omega/actions/runs/40988721734) passed
at exact head `0ac181bf065e398d6d6ae455c2659416895d6b49`.

The dependency's conflict was resolved in a separate worktree by preserving
main's already-landed convergence implementation and cycle test. PR #822's
integration head `55b69e34896c17f6aa013351f95c8b3bc73f1163` passed compiler admission
[40991844074](https://github.com/CathedralOS/Omega/actions/runs/40991844074).
Neither PR was merged during this iteration. These CI records do not qualify
the rejected candidate.
