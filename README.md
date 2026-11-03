# PR 1568 validation after conflict resolution

A constant helper must establish a call's precondition before using its result.
These records show that the integrated compiler rejects circular proof while
preserving independently justified calculations and valid recursive calls.

R13 was built from exact source 4b50ded92ae9cb9f5216e8e8ed6115f9054d7cc2,
with no compiler-input patch, on fixed main e3c4ae4fb406f0a40e8988e2b79100ceeeaff5b3.
All 30 positives check and run with native and Terminal exit 0. All 28 negative
updates reach their intended proof diagnostic. Nine subsequent checks stop at
the compiler-identity review fence; those setup refusals receive no proof credit.
The unchanged layout customer updates normally and exits 70 in both engines.

All 48 focused Rust tests pass, including rejection of incompatible formats
236 and 237. The 23-fixture corpus run checks 13 positives, rejects 10 negatives
for the intended reasons, and executes the portable calling policy after its
source session is released. Both clippy commands pass.

The 11 delegated termination controls check five positives and observe three
runtime agreements at zero. All six negatives reject; four match their expected
text. Two have stale expected text, byte-identical to R12 and the retained R11
attribution; neither is counted as a matched-diagnostic control. The separately
run layout customer completes the 12-fixture termination selection.

manifest.json records the unchanged source bundle, its SHA-256, exported
commit/ref and published prerequisite. receipt-files.json lists every archived
file's hash. observations.tar.gz retains raw receipts, streams, source/oracle
hashes and original producer profiles. Absolute paths name the original machine;
archive paths retain the corresponding relative filenames.

To restore source in a clone containing the listed prerequisite:

    git bundle verify /path/to/COMMIT.bundle
    git bundle unbundle /path/to/COMMIT.bundle

Unbundling restores objects without switching a branch. Inspect source with
git show COMMIT:path/to/file. Extract observations.tar.gz separately for receipts.

Earlier baseline and R12 evidence, including the original wrongful acceptance,
remain unchanged at:
https://github.com/CathedralOS/Omega/blob/f7790912afea928f7a4fbbbd412ca51991159e9c/README.md
Those are historical observations, not runs of the newly integrated compiler.

Crash-capable helper admission, direct runtime observation of the unused
constant's value eight, GUI qualification and execution on other hosts are
not established here. The scalar-range migration remains open.
