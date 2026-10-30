# Fresh three-producer custody composition experiment

The [linear ancestry screen](../windows_ancestry_postqueue_2026_10_29/README.md)
and [sibling verification](../windows_sibling_custody_2026_10_29/README.md)
did not individually establish the requested stable large latency gain. Both
preserved measured behavior and reduced repeated filesystem work, but their
standalone wall evidence was mixed. This is a new composition question: does
removing both repeated prefixes and repeated sibling ancestry produce a large
whole-compile improvement? Do not add their percentages or repeat either
standalone comparison as if it were new evidence.

Fixed local composition base `3b3cbfed2aa` retains the sibling candidate only as
an experimental control. Three frozen producers participate in each cohort:

- Original qualified post-queue producer `808ee3ff1725`, source `19554817237`.
- Qualified sibling-only control `32c4253867e6`, source `0e5382e807d`.
- The exact newly built combined producer, with both fresh sibling verification
  and parent-relative linear ancestry. No other compiler mechanism changes.

The original complete-image catalog supplies the oracle for all three; each
compile must produce those bytes and actually run with exit 0. Tests cover all
three sibling lanes, wrong parent/retained identities, non-direct descendants,
deep concrete ancestry, child replacement after classification and retained
leaf reconciliation after parent replacement. Fresh pathname classification,
no-follow opens, classified/opened identity comparisons, current retained-root
comparisons and before/after sibling-parent checks all remain. No check is
cached across an operation, no custody subject is removed and no cache depth,
allocator, CPU tuning, Rayon count or workload is changed.

Each cohort uses independently seeded ordinary-depth private caches for three
Windows-only subjects, fresh processes/output directories and four symmetric
six-observation blocks: original/sibling/combined/combined/sibling/original and
its reverse, alternating twice. This gives eight timed observations per flavor
and subject. Three rotated-flavor seed groups and nine profiles remain separate
from the 72 timed compiles. Independent confirmation reverses the block order
in new caches. All valid observations remain, including slow outliers.

Both small-program screen medians must be at least 15% below the original to
trigger the existing full quality/confirmation chain. A substantial accepted
result requires roughly 20% or better direct original-to-combined improvement
for both small subjects in both cohorts, positive original-to-combined effects
in every small-program block, and no material stack32 median regression (5%
screening tolerance). Report the direct sibling-to-combined effect separately,
not as an additive gain. CPU/private commit, cache hits, exact image checks and
all failures remain visible. The two inconclusive standalone receipts do not
substitute for this measurement or the combined producer's quality checks.

Full quality uses all eight focused Windows tests, affected release Clippy and
touched formatting; nine native/Terminal cases and nine rejects; six malformed
cache recoveries; five source/root/pruning controls and real junction controls
on original and combined producers. No full corpus, foreign-host execution or
publication is inferred. If composition also fails, restore all owned custody
Rust changes to `2cfeab613fd`, preserve the exact patches and stop this family.

## Result: scoped substantial win, retained locally

Both cohorts satisfied the predeclared acceptance. These are directly measured
original-to-combined complete Windows compile medians, not added percentages:

| Subject | Screen, ms | Reduction | Independent confirmation, ms | Reduction |
| --- | ---: | ---: | ---: | ---: |
| length | 642.4 → 450.5 | 29.9% | 715.4 → 467.1 | 34.7% |
| dispatch | 632.6 → 455.5 | 28.0% | 721.6 → 443.5 | 38.5% |
| stack32 | 1108.7 → 869.6 | 21.6% | 1024.7 → 823.0 | 19.7% |

Every subject's four blocks improved in both cohorts; small-program block
reductions ranged from 18.3% to 49.8%. Screen ranges separated for all three
subjects. Confirmation length and stack32 ranges overlapped; all observations,
including the 1206 ms original length and 1165 ms combined stack32 observations,
remain. No valid sample was excluded. The eight-sample medians and two cohorts
support this scoped result on the shared host, not a universal compiler ratio.

The sibling-only control medians were 571.3/592.8/1002.1 ms in the screen and
622.4/605.2/923.7 ms in confirmation (length/dispatch/stack32). The combined
producer was directly 21.2–26.7% faster than that control on small programs and
10.9–13.2% faster on stack32. This demonstrates the incremental effect in the
composed setting; neither weak standalone result was relabeled or pooled.

Original-to-combined median root CPU dropped 601.6 → 429.7 ms for length,
609.4 → 429.7 ms for dispatch and 1023.4 → 835.9 ms for stack32 in the screen.
Confirmation was 656.3 → 429.7, 648.4 → 429.7 and 968.8 → 781.3 ms. Peak job
private commit medians increased about 8 MiB for length and 2.7 MiB for stack32,
while dispatch stayed within 0.03 MiB. These are private commitment observations,
not RSS, and no memory reduction is claimed. Full ranges and host-busy counters
are retained with the raw observations.

All 180 seed/timed/profile compiles across the three producers emitted the
pinned original image and actually executed with exit 0. Nine native/Terminal
agreement cases and nine authored rejects, six malformed-cache recoveries,
five source/root/pruning controls and four original/combined junction controls
passed. Leaf junctions disabled graph reuse on both producers with native exit
0; ancestor junctions refused with exit 1, with outside target bytes unchanged.
All eight focused Windows custody tests passed, with 112 unrelated tests
filtered; affected release Clippy and touched formatting passed. No new native
launch timeout occurred; this does not resolve the earlier PR850 incident.

Candidate source: `3cd59926410ee48793ccf652a46e181fa6e2c6e2`; frozen ordinary-O3
producer SHA256 `e6012e35728b6c892dfa1211b44d87bc520bbaab023098ebabde9fedd5db9aa8`.
Source-bound qualification run `56dfa3a4e4bf4dbba0d9d4e1923be129` exited 0 with
no input invalidations. Its verified receipt SHA256 is
`81ba58e71fad4b7d7d44518524cc0d64db2c3fc1638302a48f4b0d12fe7f2990`.
All 658 captured-output digests matched before the result archive was created.

[Unrounded timings, controls, resources and acceptance calculation](summary.json)
and [697 source/build/receipt/raw evidence records and exact combined patch](evidence.json.gz)
retain the complete experiment. New claim `ac740a390e314f57afb67870454d5d2c`
registered successfully and released the completed attribution claim; preceding
uncertain sibling registration was confirmed absent before this claim.

This completes the requested next substantial-win search after PR850. The new
change is committed locally; publication was authorized for PR850, not inferred
for this result. No latest-main requalification, full corpus, foreign-host
runtime, cold-latency or new multi-target speedup claim is made. Do not multiply
these Windows-only results by the prior four-target queue improvement.
