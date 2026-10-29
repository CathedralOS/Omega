# Cumulative compiler win; return-bound memo rejected

**Retained result:** two compiler changes reduce the fixed stack32 whole-compile
median from 5664 to 4420 ms, **22.0%**. This directly meets the approximate 15%
whole-compile stopping criterion for this continuation. Stop here, retaining
difference-constraint decisions and equality-endpoint indexing. The proposed
return-bound memo was rejected and removed before this final comparison.

| Workload | Starting compiler, ms | Retained compiler, ms | Reduction |
| --- | ---: | ---: | ---: |
| Stack32 other_machine | 5664 [4792, 6450] | 4420 [4364, 5412] | 22.0% |
| Length local | 2120 [2068, 2189] | 2085 [2069, 2124] | 1.6% |
| Dispatch match_join | 2394 [2270, 2895] | 2231 [2160, 2295] | 6.8% |

Stack32 checking median is 2455 -> 1674 ms (31.8% lower).
Each cell shows median [minimum, maximum] for four unprofiled observations,
following warmups and alternating ABBAABBA. This is a local fixed-workload result,
not an all-program claim. No material regression is established in the two
smaller subjects; their spreads overlap. All 36 compiles and native executions
succeed with byte-identical images per subject, including the unchanged stack32
114176-byte image. No valid observation was excluded.

[Retained runs](retained-runs.json), [summary](retained-summary.json),
[metadata](retained-metadata.json), [transcript](retained-comparison.txt).
A is the start-of-continuation producer SHA256 `67c79dda...` (already includes
PR750); B is `139ad7c5...`, the exact retained producer from source commit
`5f5f5c64e5a2d8f8fb78b2a649ccb3025779bf2f`. Full hashes and optimized profiles
are in metadata. The two implementation commits are
`cb77f31d5eff1d3dbde5a0a59b5b45162d6edf58` and
`5f5f5c64e5a2d8f8fb78b2a649ccb3025779bf2f`.

The behavior records below name this retained producer as **A** in the
rejected-memo A/B test. Its 35 observations therefore apply directly: 13 native /
Terminal agreements, intended source refusals, and focused checking successes,
with one pre-existing authored-diagnostic mismatch explicitly retained. Its
previous graph unit tests and stack32 nine-place validation also remain applicable;
no production source changed after their commit.

## Rejected follow-up

The hit/miss diagnostic proves eight misses and three cheap hits; all six
expensive projections are misses at 103-118 ms each. [Actual cache events](cache-attribution.json)
and [complete profile](cache-attribution-profile.json) supersede the earlier
address-based roster. Different context lifetimes can reuse an allocator address;
address equality was insufficient evidence for shared memo ownership. The cache
correctly reset at those boundaries, so its useful reuse was negligible. Remove
it rather than broaden its proof lifetime. The diagnostic still produced the
exact expected native image and exit zero. No diagnostic logging remains.

The final retained comparison is a **changed candidate**, with the memo removed.
Do not compare absolute timings between the two runs to claim that removing the
memo itself caused the difference; shared-host conditions varied. The experiment
below remains intact as rejected evidence, including its inconclusive incremental
screen and original oracle-check failure.

**Rejected:** the diagnostic shows only three cheap hits and no expensive reuse.
The production memo was removed. The candidate timing results below describe
the rejected experimental bundle, not the final retained compiler.

ITERATION-PERFORMANCE, fixed candidate base
`5f5f5c64e5a2d8f8fb78b2a649ccb3025779bf2f` plus [candidate.patch](candidate.patch).
Continues [equality endpoint indexing](../statement_remainder_2026_10_28/README.md).
This is a compiler-workflow optimization; no new language rule or fact-row
completion is claimed.

## Mechanism and attribution

The initial arithmetic diagnostic puts 785 ms in call-result analysis, including
512 ms in declared guarantees and 260 ms in body inference. Finer attribution
finds declaration dependency collection (4441 calls, 7.74 ms) and independence
checking (991 calls, 18.35 ms) too small to justify another cache. That hypothesis
was rejected without implementation. Complete contract projection takes 650 ms
for 33 calls. These are inclusive scopes from separate profiles, not additive
whole-compile costs.

The [query roster](query-roster.json) contains 22 distinct complete keys among
33 requests; two expensive 32-formal keys occur twice each. Reuse those complete
answers within the existing immutable checking context. The key includes the
callable, result type, declared interval, and complete formal-keyed scalar and
collection-length actual bounds. Only exact stored parameter/contract slices
qualify. Context reconstruction resets this field; projected trees and guarded
layout views cannot reuse it. Admission, independence and partial-operand
formation remain ahead of the query. Projection order is unchanged. No lock
spans computation, and a poisoned lock falls back to computing the answer.

See [research](research.md) for source tracing, primary-source applicability,
rejected work and the predeclared comparison. Diagnostic-only changes were
removed before the candidate build. The three diagnostic patches are retained
for attribution replay, not production; zero-context patches need
`git apply --unidiff-zero` and LF line endings.

## Exact comparisons

The incremental profile screen compares endpoint-indexing baseline
`139ad7c5be5564fc9633e50e1d0a41744e7770c4bea909d476cd0845d7e8a558` with
candidate `d84f93b4bc336d955acb50feb91345ee813db519c72faef2caf85ebf382ba664`.
Both are optimized dev producers (opt-level 1).

The cumulative comparison instead starts from the producer at the beginning
of this continuation, `67c79dda5d51d0034320ae0244b957a041794c751b69f07cdd42a08d453702b3`.
That includes PR750's existing equality-closure reuse; it does not compare with
the older pre-PR750 compiler. Its candidate contains the difference-constraint
shortcut, endpoint indexing and complete return-bound memo. The measured
cumulative result must not be inferred by multiplying earlier percentages.

Each timing subject uses fresh project and LOCALAPPDATA paths per observation,
settles package review outside the timed compile, verifies optimized executable
identities and frozen source/build hashes, checks successful compilation and a
nonempty publication, runs the native executable, and compares image hashes.
The cumulative harness has two warmups, eight unprofiled ABBAABBA observations,
and a separate profiled A/B pair per subject. No valid timing is excluded.

## Rejected memo bundle: cumulative measurements

Four unprofiled observations per producer after warmups, interleaved ABBAABBA.
Median [minimum, maximum] in milliseconds:

| Workload | Starting compiler | Candidate | Median reduction |
| --- | ---: | ---: | ---: |
| Stack32 other_machine | 7630 [6587, 8122] | 6539 [5454, 7176] | 14.3% |
| Length local | 2328 [2154, 2353] | 2249 [2154, 2301] | 3.4% |
| Dispatch match_join | 2389 [2265, 2496] | 2285 [2223, 2380] | 4.4% |

Stack32 checking falls from 3113 to 2323 ms (25.4%). All four adjacent
stack32 pairs favor the candidate, but the wall-time ranges overlap. The
separate profile pair is 4607 -> 4111 ms; profile scopes are attribution, not
substitutes for the unprofiled comparison. These are local matched-workload
results, not a claim about all programs or hosts. No timing was discarded.

All [36 cumulative observations](runs.json) and [six incremental profile
observations](incremental-runs.json) compiled and ran natively at zero. Each
subject's native image is byte-identical across producers. Stack32 remains
114176 bytes, SHA256
`55adb749405f33a9dcefcadb5a0c3f165452d1a8f05993f11f30ce8114aeb8b8`.
See [summary](summary.json), [metadata](metadata.json) and
[transcript](comparison.txt). Peak job commitment medians for stack32 are
1.353 -> 1.350 GB; no memory saving is claimed. The [host snapshot](host-memory.json)
and [build-process snapshot](competing-builds.json) are point-in-time evidence,
not exclusive access guarantees.

The incremental six-run profile screen alone is inconclusive: wall medians
6523 -> 6872 ms, with large chronological drift (7554 ms first observation,
3938 ms last). It remains in the record. The cumulative claim comes from its
separate direct comparison and must not be assigned wholly to the last memo.

## Behavior validation and limits

[Seventy A/B observations](validation.json) agree, including 26 actual native /
Terminal pairs at exit zero. The existing nine positive call-guarantee places
and nine circular-promise twins pass on both producers. Additional controls
exercise distinct scalar actuals, collection lengths, mutation, nested calls,
mathematical applications and partial-operand formation. Every new fixture
passes or rejects for its intended reason on both producers.

Sixty-eight observations satisfy their intended/authored oracle. Two observations
of the existing `collection_result_rebound_descriptor` reject identically, but
its authored diagnostic is stale on the baseline: it expects missing reference
origin; both producers instead reject `count` outside length 2. This is explicitly
marked `authored_oracle_matched: false`, with source/oracle hashes and the exact
baseline refusal pinned. The original failed oracle check is preserved in
[validation-initial.txt](validation-initial.txt). No canonical fixture was
weakened or edited. [Inputs](validation-inputs.json) and
[producer identities](validation-producers.json) bind the observations.

This targeted validation does not establish full-corpus health, other-host
native behavior, independent promotion review, or publication. No full suite,
Clippy or cross-host execution was run. The remaining work stays local in the isolated
worktree; PR750's submitted head is unchanged.

## Replay

Hold one `tools/build_slot.py` slot around the local build/test chain. Use
PowerShell 7 on Windows: PowerShell 5 with Stop-on-error turned a native stderr
deprecation warning into an exception before the initial validation chain ran.
The retry's optimized incremental build exited zero.

- Build: `mbx build -p omega --locked --target-dir build/pass-producer`.
- Focused behavior: `python validate.py BASELINE CANDIDATE FRESH_OUTPUT`.
- Incremental profile: `python ../spill_loop_2026_10_28/ablate.py BASELINE CANDIDATE FRESH_OUTPUT`.
- Cumulative: `python ../checking_simplification_2026_10_28/compare.py START_BASELINE CANDIDATE FRESH_OUTPUT`.

The validator reuses the previous runner with its original defaults preserved,
selecting the existing independent-call row and its exact circular-promise
refusal, plus source-level actual-substitution, length, mutation and formation
controls. It freezes source, Build and authored oracle files and checks their
hashes after both producers finish. The new reject fixture was first refused by
the baseline for the intended `second_difference` exact-arithmetic obligation;
[new-reject-baseline.log](new-reject-baseline.log) retains that observation.
