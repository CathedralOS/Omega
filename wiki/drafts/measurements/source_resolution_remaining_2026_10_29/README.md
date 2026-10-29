# Remaining source-resolution attribution

Decision: **retain the Windows ancestry traversal change locally**. All three
warm workloads improve with non-overlapping observed ranges; stack32 also
improves on first compile. First-compile length and dispatch remain inconclusive.
No valid slow observation was removed. This is a measured Windows workflow gain,
not a claim that all source-resolution or compiler work is optimized.

Parent: ITERATION-PERFORMANCE. Fixed warm workbench `4fba237cd10`; optimized
baseline `build/source-resolution/candidate/omega.exe`, SHA-256
`7a042d736c07ca783eceeb282f41067b28cb320c05f703dfcb24c2737928467c`.
The preceding candidate profile measured `build.resolve_sources` at 252.940 ms,
after avoiding irrelevant review-snapshot graph decoding. That scope includes
source acquisition, preparing the Build frontend, checking and executing Build,
and joining the resulting graph. It is not a pure file-resolution timer.

Hypothesis: one of acquisition, parsing/typing or Build preparation dominates
the remaining cost and exposes repeated work. Temporary guards divide those
owners without skipping admission, changing sources or replacing Build results.
Six ABBAAB diagnostic comparisons use the existing runner, fixed source and
Build inputs, fresh private caches, ordinary package review outside the timed
interval, exact native image equality and exit 0. These profiles attribute cost;
they cannot establish an unprofiled production speedup.

Before a production edit, identify removable work and its correctness contract.
Preserve exact source custody, inputs, complete graph lineage, Build effects,
source admission and all cache validation. Any candidate needs affected quality
controls and an interleaved unprofiled whole-compile comparison. If no safe
removal is supported, retain the attribution and stop this bounded iteration.

Publication discovery: main already contains the equivalent root-graph selection
from [PR #762](https://github.com/CathedralOS/Omega/pull/762), with domain-separated
keys and a 64-record/512 MiB limit. [PR #802](https://github.com/CathedralOS/Omega/pull/802)
publishes our preceding experiment's additional historical evidence without
replacing that implementation. Continue this attribution on the fixed warm
baseline; verify any proposed changed operation against current main before
publication. Never relabel these producers as current main.

The diagnostic queued behind both normal build slots. Other sessions retain
ownership of those processes; no slot policy or process was changed.

## Attribution and candidate

The first diagnostic built in 1m37s after a 216-second slot wait. One diagnostic
native launch timed out at 30 seconds after compilation emitted the exact same
SHA-256 image as the successful baseline. Its traceback and full compiler output
are retained separately. Both exact images subsequently ran with exit 0 in
about 50 ms; the original timeout's cause remains unresolved. A fresh replacement
observation completed the six-run sequence. The failed observation is not a
successful compile/native pair, and the replay is not a replacement timing.

The detailed follow-up built in 1m49s and completed six more exact-image native
observations. Build checking took 160.845-177.125 ms, filesystem setup
0.334-0.446 ms, interpreter preparation 4.415-6.272 ms, and retained-root setup
108.606-143.745 ms. Nested costs are not additive. This supports investigating
retained-root setup instead of treating file capture or filesystem staging as
the dominant cost.

Windows ancestry validation reopens every path prefix through
`open_absolute_directory_nofollow`, whose own walk traverses the whole prefix.
The candidate retains the validated parent handle and opens its immediate child
without following links, as the existing macOS owner already does. Each absolute
pathname is still classified and compared to the opened child identity. Final
root identity and every before/after caller check remain unchanged. This changes
traversal from repeatedly opening all prefixes to one open per ancestor within
each validation call. It does not memoize custody across validation calls.

Scope: the sole Windows ancestry caller, its platform helper, and three focused
Windows controls for classified-child replacement, different-parent identity, and
junction ancestry. [Research and acceptance](research.md) records the reused
macOS design and official API. Acceptance required the existing custody-test
group, three-workload native/cache controls, and the 36-observation comparison
runner before a retention decision. No whole-corpus sweep was run.

The initial focused group ran ten tests: nine passed, and the parent-replacement
fixture failed before invoking the verifier because Windows refused to rename
the open directory (OS error 32). Preserve that log and initial patch. The final
fixture renames before opening the original parent, then creates a new named
parent/child; the verifier must reject the mismatch between the named child and
the opened parent's child. This tests identity separation without assuming that
Windows permits renaming a held directory. The production implementation did
not change in response to this fixture correction.

For latency, adapt only the private cache placement in the existing comparison
runner: fresh random directories under the host temporary directory instead of
the deep experiment output tree. Record each actual cache path and component
depth. All source, Build, producer, order, timing and native assertions remain
the same. This avoids using an artificially deep cache as the sole performance
denominator for an ancestry-depth optimization; it is still an isolated cache,
not the user's production cache. The deeper quality paths remain covered.

First-compile observations showed a clear stack32 separation but overlapping
length timings. Retain every sample; do not repeat that batch to seek a favorable
result. A separate 24-observation warm comparison answers a different question:
does the changed ancestry traversal help the actual retained-graph path? Reuse
the same measured projects and shallower caches after exactly one successful
compile, keep their interleaved order, use fresh output directories, leave phase
profiling off, and require each exact image to match its first compile. First
and warm cache regimes remain separate claims.

## Results

Both producers report opt-level 1 on Windows x86_64, rustc 1.100.0-nightly
(a69a63265 2026-09-03). Candidate SHA-256:
`10373901d2eee2c7d278da3e56e0b6dc17d8e2e5822a2ec85f69b84b74335b1d`.
[Metadata](comparison/metadata.json) binds source/Build and producer identities;
[candidate.patch](candidate.patch) includes the complete implementation and
focused tests. No changes to Cargo dependencies, cache naming, record limits,
source matching or compiler judgments were made.

The timed cache base has seven path components, including the drive, under
`C:\Users\User\AppData\Local\Temp\omega-ancestry-<unique>`. The ordinary
`LOCALAPPDATA` base has five. This is a shallower isolated approximation, not a
benchmark of the user's shared production cache. Review is outside the timed
first compile. Warm means exactly one preceding successful compile on that
project and cache, followed by a fresh compiler process and output directory.

| First compile | Baseline median [range], ms | Candidate median [range], ms | Decision |
| --- | ---: | ---: | --- |
| stack32 | 2545.9 [2478.8, 2633.4] | 2358.0 [2309.8, 2415.3] | 7.4% lower |
| length | 1919.0 [1854.6, 2181.7] | 1907.7 [1748.6, 2379.7] | Inconclusive |
| dispatch | 1889.8 [1852.0, 2149.3] | 1863.4 [1755.9, 2334.5] | Inconclusive |

| Warm compile | Baseline median [range], ms | Candidate median [range], ms | Median reduction |
| --- | ---: | ---: | ---: |
| stack32 | 1087.3 [1030.0, 1183.3] | 944.7 [931.1, 948.0] | 13.1% |
| length | 616.5 [584.6, 740.0] | 494.8 [458.4, 550.4] | 19.7% |
| dispatch | 581.7 [566.6, 613.6] | 491.8 [465.4, 514.1] | 15.5% |

Each measured cell has four unprofiled observations in interleaved order.
The first-compile batch additionally retains two warmups and two profiles per
workload; these are excluded from the unprofiled latency summary. All observations,
including the slow candidate length/dispatch runs, remain in
[comparison/runs.json](comparison/runs.json) and [warm/runs.json](warm/runs.json).
[summary.json](summary.json) retains complete latency/memory vectors, counts and
attribution scopes. Memory savings are not claimed.

The single stack32 profile pair attributes source resolution at 256.352 to
206.662 ms and session creation at 74.079 to 33.697 ms. Those scopes support the
traversal mechanism; they do not replace the unprofiled whole-process results.
These figures are not directly comparable to the earlier deep-cache profiles.

## Validation and review boundary

The final focused custody run passes **10/10** tests, including all three added
Windows controls. The initial fixture failure is preserved in
[custody-tests-initial-failure.log](custody-tests-initial-failure.log), with the
corrected run in [custody-tests.log](custody-tests.log). Optimized `mbx build -p
omega --locked`, touched rustfmt, diff whitespace and evidence-script syntax
checks passed. Warnings in the build/test logs are retained.

There are **94 successful compile/native pairs and two additional successful
native replays**, plus **two static rejects**: 12 diagnostic pairs, 22 quality
pairs, 36 first-compile pairs and 24 warm pairs. The eight damaged-record
recoveries are part of the 22 quality pairs. The invalid control rejects through
ordinary package review with `count > 0`; it is not separately run under a stale
manifest. Changed source at the same live path produces a changed image that
matches A/B. Every unchanged workload's native bytes and exit 0 match across
producers. The one original diagnostic launch timeout remains an unresolved
failed observation in [interrupted/](interrupted/); its successful replay does
not erase that result. Package review's internal compilations are not counted.

[collect.py](collect.py) asserts counts, exact producer/source/Build hashes,
native outcomes and image agreement, cache depths, final test success, and the
candidate's unchanged source patch before collecting raw evidence. Diagnostic
instrumentation was restored before each measurement batch and is absent from
the retained implementation.

Implementation-owner review covered the sole Windows caller and helper, every
ancestor's fresh classification/opened-identity comparison, no-follow immediate
child opening, final root comparison, and the unchanged caller-side before/after
checks. The walk retains only its immediate verified parent; it does not turn
an earlier validation into permanent authority. Invalid paths can now report
an earlier ancestor first because traversal is top-down. Windows replacement,
different-parent and junction controls prove the affected refusal boundaries.
This is not an independent review claim.

Limits: one Windows drive-based host, three fixed workloads and four measured
observations per variant/regime; no full corpus, UNC/network-share run, other-host
runtime, or separate Terminal comparison. Machine contention was not suppressed.
The change remains local pending separate publication. PR #802 publishes the
preceding root-selection evidence and its required compiler-check passed; it
does not contain this ancestry traversal change.

This bounded iteration stops with a supported warm-compile improvement and
preserved focused behavior. The next highest-value question is the remaining
Build-check preparation (about 161-177 ms in the detailed baseline), keeping its
semantic admission and effects boundary intact.
