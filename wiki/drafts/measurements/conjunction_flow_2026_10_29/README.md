# Carry conjunction environments forward

Retained. On the unchanged 32-argument stack-calling witness, median whole-compile
time fell from **2980.7 to 2705.9 ms (9.2%)**, and checking time fell from
**1505.2 to 1254.8 ms (16.6%)**. The native executable is byte-identical.
This is an ITERATION-PERFORMANCE improvement, not a new language capability or
closure of a fact row. [Research, attribution and semantic review](research.md)
explain the selected mechanism and the rejected cache-lifetime alternative.

## Change

`call_result_bounds::project_possible` used to evaluate a conjunction's growing
left prefix again before projecting each right operand. It now walks the ordinary
left conjunction spine and carries the completed condition environment forward
in evaluation order. Folded and prepared mathematical predicates remain atomic;
disjunctions, effects, impossible intervals and unknown projections retain their
existing handling. There is no new memo or wider proof lifetime.

The new positive control nests both left and right conjunctions, including a
folded true pair, and consumes two exact call results. Its reject twin changes
only the second actual from 9 to 8 while retaining subtraction by 9. Both were
run on the original source before implementation; see
[baseline observations](new-controls-baseline.json).

## Fixed inputs and producer identities

Fixed source base: `894c94a2eaef8c2e8c362b7282d017f5d53e5516`.
The production change is [candidate.patch](candidate.patch). Both timed producers
use the pinned Rust toolchain and optimized dev profile (`opt-level = 1`) on
Windows x86-64. The original candidate build was interrupted. Two resumed builds
refused corrupt dependency metadata; the second restored the corrupt bytes from
cached build state. Their [first](build-corrupt-metadata.log) and
[second](build-corrupt-restore.log) logs are retained.

Recovery used a fresh process-local mbx cache and disabled learned incremental
reuse for this build. No global settings or other worktrees were changed.
The original source was rebuilt with the same settings for the decisive A/B,
then the candidate source was restored. [Matched build inputs](matched-build.json),
[candidate build](build-candidate.log), [baseline build](build-matched-baseline.log)
and [build script](matched_build.py) retain the provenance.

| Role | Executable SHA256 |
| --- | --- |
| Original baseline, source-oracle checks | `0fe9f17da920857de5356f47cc46727623b5717e81b74ce4c9bcab5e1f610046` |
| Matched baseline, decisive timing | `6dea2214eb78f83d8e2d1a62219dae0b34370038eea7bcee148c45ac785ba566` |
| Candidate, checks and timing | `12ca439bfc0fd52daa45cd186195030b29af4a3bf1f3c5032909fda272f6cd22` |

The six-run [preliminary screen](preliminary-screen.json) uses the original
baseline. It is preserved but excluded from the retained speedup claim because
the build-cache recovery could otherwise confound that comparison.

## Measurement

The existing three-subject runner uses `ABABBAABBAAB` per subject: two warmups,
eight unprofiled observations (four per producer), then two diagnostic profiles.
Every observation gets a fresh project and process-local source-cache directory.
Every successful compile must publish an executable and run it at exit zero;
source/Build hashes, producer digests and optimized profiles are checked.
All 36 observations succeeded with one exact native image per subject across A/B.
No observation was excluded. Warmups and profiles are kept out of the medians.

| Witness | Baseline median, range (ms) | Candidate median, range (ms) | Decision |
| --- | --- | --- | --- |
| stack32 / other_machine | 2980.7, 2957.4-3009.9 | 2705.9, 2662.6-2707.3 | 9.2% lower; ranges do not overlap |
| length / local | 2585.8, 2026.3-3166.5 | 2195.0, 2022.1-3258.2 | Too variable to claim a gain |
| dispatch / match_join | 2101.0, 2011.2-2405.2 | 2089.4, 2055.3-2161.8 | Effectively flat at this resolution |

[Summary](summary.json), [all runs](runs.json), [metadata](metadata.json) and
[raw transcripts](comparison/) retain timings, image hashes, exits and inputs.
The stack32 image is 81,408 bytes, SHA256
`ad8b1cc4a2bc4e5a4d39d7d789c0107a4edfccbd3d9b8e0002e558cefd494aca`.
This newer fixed base differs from the prior experiment's backend and image;
do not multiply this reduction by old-baseline percentages as a measured total.

This is a shared desktop, not a controlled idle host. No rustc/omega/mbx processes
were observed in the snapshots immediately before and after the decisive run,
but those snapshots cannot establish continuous absence of contention. The
length observations are visibly noisy and remain inconclusive. Recursive
profile scopes are inclusive; never sum them into elapsed time.

## Behavior

[Validation](validation.json) records 78 A/B observations:

- All nine existing independent-call fact places execute through both native and
  Terminal engines under both producers; their nine circular-promise twins reject.
- Twenty-one focused controls cover conjunction grouping, exact actuals, composed
  guarantees, formation, collection descriptors, disjunctive return bounds,
  mathematical application and mutation invalidation.
- There are 28 native/Terminal observations in total: 26 use authored exit zero,
  and the short-circuit runtime control uses authored exit 70 for both engines.
- Seventy-six observations meet their authored/intended oracle. The two existing
  `collection_result_rebound_descriptor` observations reject identically at the
  unsafe index, while their old authored diagnostic asks for missing reference
  origin. Both retain `authored_oracle_matched: false`; no fixture was weakened.

The reused validator initially assumed every runtime success meant zero, so it
falsely failed the existing exit-70 control for both producers. The runner now
reads `expected_native_exit.txt` and checks that exact exit from both engines.
The [initial record](validation-initial.json) and [failure log](validation-initial.log)
are retained. Only those two observations were rerun; the other 76 successful
observations were reused after verifying source/Build hashes and producer digests.
[Inputs](validation-inputs.json) also freeze the authored oracle files.
[Validation producer identities](validation-producers.json) distinguish the
original baseline from the separately rebuilt timing baseline.

## Publication and next question

The preceding two optimizations landed in [PR760](https://github.com/CathedralOS/Omega/pull/760).
Its submitted head was `fbac55325b3766f5688609127f9015b6122bfc01`; published main is
the fixed base above. PR compiler-check run 40840698152, merge-queue run
40841224751, and post-merge smoke run 40841499791 succeeded. Published-main
ancestry was verified after fetching. [Publication identities](publication.json)
separate submitted and published revisions. This new conjunction change remains
local until separately published.

The next measured question is permission-event preparation inside flow checking:
the candidate's diagnostic reports 439.1 ms there, with 428.5 ms in its state
loop. Attribute that preparation before attempting another cache or changing
proof ownership. These are diagnostic inclusive scopes, not an additional
measured speedup opportunity to add to this result.

## Replay and limits

Use one `tools/build_slot.py` hold for each local chain. Build with
`mbx build -p omega --locked --target-dir build/pass-producer`; retain separate
executable copies. The matched build script requires the fixed base above with
the candidate patch applied and the candidate executable already retained.
Run `python validate.py BASELINE CANDIDATE FRESH_OUTPUT` for the focused checks,
and `python ../checking_simplification_2026_10_28/compare.py BASELINE CANDIDATE FRESH_OUTPUT`
for the decisive comparison. Paths are relative to this record directory;
the build script runs from the repository root.

No full corpus, whole-workspace suite, Clippy or other-host native execution was
run. The focused results do not establish those gates or publication of this new
candidate. The benchmark measures compiler invocation time after package review,
not the full package-update workflow.

Follow-up: this conjunction optimization landed in
[PR767](https://github.com/CathedralOS/Omega/pull/767), published main
`9774b83a2f6f57359386f4984c83fef348985eb3`. PR, queue and post-merge checks
passed. The next [permission preparation experiment](../permission_flow_2026_10_29/README.md)
preserved native outputs but did not establish a whole-compile gain; its
candidate was discarded and its profile evidence retained.
