# Windows sibling custody experiment

Fixed local base `2cfeab613fd` has the qualified post-queue compiler/library
closure of O3 producer `808ee3ff1725`, source `19554817237`. All temporary
attribution timers and the parked per-path ancestry candidate are absent.

[Setup attribution](../invocation_setup_2026_10_29/README.md) found storage
preparation and acquired-root cache-directory setup together taking roughly
168–207 ms in warm small compiles. The source resolver repeatedly verifies its
root and three sibling lanes, reopening common prefixes independently. Test
sharing that prefix within one verification operation, preserving all four
subjects, rather than changing which lanes must be checked.

Windows verification checks the retained parent normally, then every direct
child with fresh full-path classification, a no-follow parent-relative open,
classified/opened identity equality and opened/retained equality. It then
performs the original full retained-parent reconciliation again. A direct-child
guard prevents substituting a deeper or foreign root. No result or ancestry
validity survives across operations; every call performs its own checks. The
per-path ancestry implementation and non-Windows verification order remain.
The shared identity/ACL validator preserves the old non-Windows root behavior.

The baseline/candidate screen uses four balanced ABBA/BAAB blocks per subject,
eight observations per producer for Windows-only length, dispatch and stack32.
Use ordinary-depth independently seeded private caches, fresh processes/output
directories, 32 configured Rayon workers and the existing accounting harness.
Keep every valid observation, exact expected image hashes, and actual Windows
execution on every compile. First visits and phase profiles stay separate.
Both small-program medians must improve at least 15% to trigger focused full
quality and an independently seeded reverse-order confirmation. The goal is a
repeatable roughly 20% whole-compile win, not a local filesystem timing.

Quality requires focused sibling/root identity tests, source acquisition Clippy
and touched formatting, the existing nine native/Terminal and reject cases,
malformed-cache recovery, source/root/pruning and real junction refusals.
All Git/workspace/external-local lanes remain checked, including controls for
the wrong retained child, wrong opened parent and a non-direct descendant.
No foreign-host execution, full corpus or publication of this candidate is
inferred. Preserve the exact patch and all results even if the screen fails.

## Result: no standalone promotion

The screen passed its preliminary gate: length 721.5 → 600.0 ms (16.8% lower),
dispatch 753.5 → 620.2 ms (17.7%), stack32 1155.9 → 1011.0 ms (12.5%). The
independently seeded confirmation weakened: length 643.5 → 582.0 ms (9.6%),
dispatch 658.5 → 619.4 ms (5.9%), stack32 1049.6 → 1084.8 ms (3.3% higher).
Ranges overlapped broadly; confirmation dispatch had two losing blocks and
stack32 also had two. Retain all valid observations and claim no stable big win.

Five focused tests, affected release Clippy and touched formatting passed.
Both cohorts passed all 120 pinned-image and actual native checks. Nine
native/Terminal agreement cases and nine authored rejects, six malformed-cache
recoveries, five source/root/pruning controls and four baseline/candidate
junction controls passed. Ancestor junctions refused with exit 1; leaf junctions
disabled graph reuse while compiling successfully on both producers, with
outside target bytes unchanged. Refusal is scoped to the cache operation.

The first qualification amendment rejected an abbreviated base SHA before any
measurement; the resumed amendment used the full resolved SHA and completed
source-bound qualification. No rebuild or result replacement was used for that
command correction. [Unrounded results and resources](summary.json) and
[source, build, receipt and raw evidence](evidence.json.gz) retain both cohorts.

Keep the frozen sibling-only producer as a control for a separately declared
composition experiment with the parked linear-ancestry patch. This changes the
tested work domain; do not rerun this standalone candidate unchanged or add the
two historical percentages. The composed result needs its own matched original,
sibling-only and combined measurements, quality checks and independent replay.
