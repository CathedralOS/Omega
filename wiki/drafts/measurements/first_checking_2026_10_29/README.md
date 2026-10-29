# First-time checking: reuse completed operand resolution within a proof batch

Retained on the fixed convergence-repaired compiler: reuse exact completed
operand-resolution answers only within one immutable proof batch. Ordinary
first package preparation of the original operand-order program falls from
187-217 seconds to 140-144 seconds in the A-B-B-A comparison, about 30% lower
on average. This is a Windows first-preparation result, not a general compiler
throughput or warm-cache speedup claim.

## Source and mechanism

Owner: ITERATION-PERFORMANCE, the user-assigned remaining first-time checking
bottleneck. Fixed base 2a087aa3ec325cfab27afae8f51b2f358b3bfc1d. The intervening
PR evidence correction changes documentation only, not compiler inputs.
Baseline A is the retained optimized dev convergence producer:
5a596bcaa6fb117eefd8735c6bb545e0ebbde1cbf1b9b8c3ba79a586a6b16308.
Candidate B is the same source plus candidate.patch, optimized dev build:
1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60.
Build took 5m06s; its exact source patch was checked before and after validation.

The first profile attributes 104 s inclusive to numeric association proofs and
80 s to their integer comparison projection. Finer diagnostic builds show
normal resolution dominates captured-value expansion; selected applications
account for nearly all resolution time. Inclusive nested/parallel scopes overlap
and are not independent savings. Three diagnostic updates took 221.85, 187.22
and 261.94 seconds with different timers and shared-host variation. None is a
performance A/B. See research.md, the diagnostic patches, logs and scope tables.

integer_predicates.rs now keeps a small answer vector while one immutable
prepared_predicate_proofs invocation processes its goals. Keys use the complete
same_resolution_payload comparison, including observation/type information that
ordinary semantic equality omits. Every miss calls the unchanged depth-zero
resolver. Each goal still registers its own atoms, bounds and required operations;
its proof and fallback do not inherit another goal's facts. The answer vector is
discarded before scratch returns to PreparedIntegerPremises. All ordinary,
premise and fallback observation owners start with reuse disabled. No global
cache, new proof rule, source exception or relaxed formation check is added.

The single focused unit test covers payload-distinct but semantically equal
observations and reuse of the same scratch under changed substitutions. The
production change adds 30 lines; the rest of the source diff is that test.

## Comparison and behavior

Every observation copies the original root/Build files unchanged into a fresh
project with its own LOCALAPPDATA cache. All 106 std sources and root inputs are
hash-pinned. Ordinary update, review decisions and resume remain in force.
The detailed phase profiler is disabled for A/B timings. All valid runs remain.

| Order | Producer | First update, seconds | Resume | Compile | Native image |
| --- | --- | ---: | ---: | ---: | --- |
| 1 | A | 217.33 | 4.24 | 7.72 | identical |
| 2 | B | 139.79 | 4.38 | 8.00 | identical |
| 3 | B | 143.99 | 5.26 | 8.78 | identical |
| 4 | A | 187.05 | 5.50 | 11.24 | identical |

First-update mean: A 202.19 s, B 141.89 s (29.82% lower). Ranges do not overlap;
both orderings support retaining this bounded change. Two observations per
producer on a shared host do not establish a universal percentage or a stable
tail latency. Warm compilation ranges overlap; no warm-compile win is claimed.
The four native images have SHA-256
7ccbfb89151d286167c97d51c39092b03e33ccf79762dbd9802205578209eb66.

The original single-evaluation control finishes fresh update in 133.58 s and
executes correctly; that is a behavior control, not a paired secondary speedup.
The selected-contract recursion, reference snapshot and copied snapshot through
a join also compile and run. All six authored numeric/formation/mutation
refusals match their expected diagnostic, including an unproved selected
requirement and the unrepresentable signed absolute value. Dependency-free
roots' no-pending-review resume refusal is recorded as setup, not a failed gate.

Counts for this candidate comparison: eight compile/native observations
(four timing, four focused); eight actual native/Terminal comparisons
(four host replays, four focused); six static refusals; one Rust isolation test.
Touched-file rustfmt, whitespace, script parsing and exact patch/input/image
identity assertions pass. The separate nine corrected convergence comparisons
belong to PR #822, not these counts.

## Harness correction and evidence boundary

Raw review found that run --both with explicit --target only compiles. The
original compare.py calls and three diagnostic runs used that combination;
they are not Terminal execution evidence. Separate native exe executions remain
valid. Those raw commands/logs are preserved unchanged. host.py replays all four
comparison projects without --target and requires actual native exit 0 and
interp exit: 0 reports. quality.py uses the corrected host invocation too.
All eight real host comparisons pass. The convergence PR receives its own
correction and nine actual host replays in the neighboring evidence folder.

No full corpus, other-host runtime sweep or independent semantic review was run.
The improvement is retained locally, separately from the published convergence
fix. The remaining question is the cost of first-time selected-application
resolution that cannot be shared within a batch; this experiment does not
justify extending answer lifetimes across proof contexts.


## Pinned native replay

The qualification runner records a fresh actual replay of all six retained
candidate native images at source checkpoint
308d50d1b7c08a575c57deffda5662cd50f3aebf. Each image digest and its authored
exit oracle are checked. Exit 0, no input invalidations; source, producer
optimization profile and actual executable digests are pinned. See
qualification-summary.json, qualification-native.stdout and producer-receipt.json.
The full immutable receipt/state stays in ignored build/first-checking as the
qualification tool requires. Its capture overhead is not a compiler timing.
This is native qualification only; the eight actual host/Terminal observations
remain the separately linked records above. Documentation added after this
checkpoint does not alter the exercised compiler, fixtures or validator.
