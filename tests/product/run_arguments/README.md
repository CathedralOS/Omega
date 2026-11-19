# Case-run argument validation

Run `python3 tests/product/run_arguments/check.py --omega <fresh-omega-binary>`
on a supported native host (Windows, macOS or Linux).

The public CLI must reject missing, empty, or option-valued `--input`, `--inputs`
and `--outputs` arguments with status 64 and the relevant usage diagnostic,
before creating output directories or preparing a project. In particular, a
missing path must never consume `--both` and silently disable comparison.
An option-looking filename remains usable as `./--both`.

Positive cases review a private copy of the existing correlated-store parameter
fact, then exercise directory discovery, ordered repeated input files and native
execution without comparison. Five native cases and four Terminal cases must
report exit 0 and preserve the fact's empty stdout/stderr in the requested files.
The test asserts comparison was actually requested and completed, including the
Terminal status and streams; a decline does not count as agreement. Review
acceptance is automated only for this local test project. All temporary files
are removed after the assertions.
