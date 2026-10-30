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
