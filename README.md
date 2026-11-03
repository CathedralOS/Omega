# PR 1568 validation evidence

The compiler must reject a constant helper that relies on its unfinished result
to justify an earlier call. These records show the original wrong acceptance,
the repaired rejection, and valid calculations that still run.

R12 checks and runs all 30 positive programs with native and Terminal exit 0.
All 28 negative updates reach their intended proof diagnostic. Nine later
checks stop at the compiler-identity review fence; they are not counted as
proof rejections. The unchanged layout customer exits 70 in both engines.
All 48 focused Rust tests and both clippy commands pass.

The 23-fixture source run checks 13 positives and rejects 10 negatives for their
intended reasons. A separate calling-policy run with
OMEGA_CORPUS_HOST_PROVISIONS=1 also observes the portable retained program
execute after the source session is released. Three focused termination
negatives match their expected text. Two other intended termination refusals
have stale expected text, byte-identical on R11 and R12; they remain uncredited
as matched controls.

manifest.json identifies each unchanged source bundle, its SHA-256, exported
commit/ref and required published ancestor. It also identifies the raw receipt
archive. receipt-files.json lists hashes of every archived file. The original
producer identities and failed or interrupted setup attempts are preserved.
The postprocessing interruption did not rerun or alter the completed corpus
measurement. Archives retain original filenames; absolute paths in receipts
name the machine on which the commands ran.

To restore a source bundle, use a clone containing its prerequisite commit:

    git bundle verify /path/to/COMMIT.bundle
    git bundle unbundle /path/to/COMMIT.bundle

Unbundling restores Git objects without switching a branch. Inspect an exported
commit with git show COMMIT:path/to/file. Extract observations.tar.gz into a
separate directory to read the original receipts and streams.

R12 was compiled from d76683cd67e98087a367a0d5de4e4cdad0fe3c80 plus its saved
source patch. The later checkpoint changes only documentation comments in
compiler inputs; its semantic equivalence is independently reviewed. These
remain R12 observations, not a claim that the later commit was rebuilt.

Crash-capable helper admission, direct runtime observation of the unused
constant's value eight, GUI qualification and execution on other hosts are
not established here. The scalar-range migration remains open.
