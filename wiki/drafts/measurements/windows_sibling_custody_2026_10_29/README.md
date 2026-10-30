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
