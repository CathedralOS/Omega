# Fixed worktree reuse, 2026-09-05

The recovered session's remaining experiment was to build one revision at a
short fixed path, switch that clean worktree to a newer revision, preserve
`target/`, and measure the rebuild. This run completed that comparison.

| Setting | Value |
| --- | --- |
| Worktree | `C:\omega-advance` (16 characters) |
| Initial revision | `d3b124d612d588f4c59def62ecfb892ba8b8da6a` |
| Newer revision | `792f4e65a8f4932a2c2345e221855c215064d972` |
| Rust | `nightly-2026-09-04` |
| Wrapper | `mbx 1.8.1` |
| Target views | `MBX_TARGET_VIEWS=0` |
| Target | Worktree-local real directory; no deletion or shared target override |
| Build concurrency | One Omega command at a time |

The worktree was absent before creation. The initial revision already contains
the upstream package renames. The newer revision changes package acquisition
and installation, so this is a source-changing rebuild, not a docs-only or
unchanged-HEAD warm run. Revisions were pinned even while origin/main advanced.

| Run | Command | Cargo compiler phase | Full wrapper wall time | Exit |
| --- | --- | --- | --- | --- |
| Fresh target | `mbx test --workspace --lib --no-fail-fast` | 102 s | 361.30 s | 101 |
| Preserved target, newer source | `mbx test --workspace --lib --no-run` | 10.76 s | 11.65 s | 0 |

Only the compiler phases are comparable: the initial command also executes the
tests, while the second only builds the same library test binaries. The compiler
phase was about 9.5 times shorter in this pair. This does not measure a full-suite
speedup, cold global cache behavior, or a general distribution of task timings.

The rebuild compiled only `package-source`, `package-manager`, and
`package-advisory`. mbx reported 0 hits, 3 misses, and 2 bypasses. The initial
run reported 19 hits, 0 misses, 115 not looked up, and 153 bypasses. These are
wrapper counters, not Cargo freshness counts: the benefit is consistent with
retaining local target artifacts despite workspace action-cache misses. No
tool upgrade, source-file restore, target deletion, or target-view toggle occurred
between the two builds. The exact reason for mbx bypasses remains unmeasured.

The initial library run completed 113 suites: 7,406 passed, 3 failed, 12 ignored.
Failures were the bounded-process aggregate CPU-limit assertion and two
platform-custody journal-lock tests returning Windows OS error 33. The latter
are file-lock read/reacquire failures, not the historical symlink-privilege
failure. None is called green here. The rebuild did not rerun tests, formatting,
Clippy, all-target check, or architecture tests. Integration must run the full
baseline and resolve its failures before landing; timing evidence grants no
gate exception.

Raw local logs and captured process exits were saved outside the repository at
`C:\Users\User\AppData\Local\Temp\omega-advance-2026-09-05` as
`initial-lib.log`, `initial-lib.result`, `rebuild-lib.log`,
`rebuild-lib.result`, and `target-preserved.txt`. Wall time uses a PowerShell
stopwatch around the direct mbx command, with `$LASTEXITCODE` captured before
printing the summary. Compiler phase time comes from Cargo's `Finished` line.

Use fixed short worktree slots for consecutive iterations when their ownership
and cleanliness are established. This evidence does not justify resetting an
occupied slot or sharing one writable target among concurrent workers.
