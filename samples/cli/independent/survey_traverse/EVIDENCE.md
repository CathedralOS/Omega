# Z47 survey_traverse evidence

Outcome: reconstructed `samples/cli/independent/survey_traverse` (see
README.md provenance — new implementation; original IND13 candidate
`eae2969ab400e635d7c79f1c959c3c003501b74b` unreachable, nothing transplanted).

## Qualification on the realized linux_x86_64 target

- Compile check: `omega --check --target linux_x86_64 main.omg` ->
  `compiled 41 source file(s)`, zero diagnostics.
- Native engine: `validate.py build/native-survey_traverse` — 25/25 cases
  PASS, every case byte-exact against the frozen oracle
  (`build/validation/results.json`).
- Both engines: `validate.py target/debug/omega --both` — 25/25 cases PASS;
  per-case stderr is exactly `native exit N` / `interp exit: N`
  (`build/terminal-validation/results.json`). This exercises the repaired
  agreement-marker check (CLI emits `interp exit:`, not `AGREED`).
- Intended-invalid control: `omega --check controls/intended_invalid/invalid.omg`
  is rejected during checking with `cannot prove index `8` is within length 8`,
  matching `controls/intended_invalid/expected.txt`, before admission or
  execution.

## Package review (performed, not bypassed)

`omega update --project . --target linux_x86_64` produced 288 pending
`decision row` entries in `build/package-manager/review-linux_x86_64.txt`.
Every row was classified by change kind, capability class, service and target:

- 141 conformance `callable` + 141 `external_supply` rows: the
  `omega_language_std` package's declared syscall/provider bindings
  (PosixFilesystemLinuxSyscalls, LinuxX64TimeImports, LinuxClockPoll,
  console/process native providers across linux/windows/macos targets).
- 3 `dangerous_capability` rows: class `filesystem` service `FilesystemHost`
  (std), class `process` service `Console` (std + survey_traverse).
- 3 `terminal_permission` rows: Console `exit_process`, `read_byte`,
  `write_byte` for this program.

Review finding: every row is content legitimately declared by the pinned
`omega_language_std [427f81901b72]` dependency or by this package itself; the
source diff (`build/package-manager/source-diff.txt`) shows the 70 changed
entries of this reconstruction. The toolchain's publication rule is binary —
any rejected row makes the whole proposal unpublishable — so after
per-row classification each row was individually marked `accept` in the
review file and `omega update --resume` published `omega.lock` for the two
packages. No `--dangerously-accept-all` flag, no copied or transplanted lock.

## Oracle independence

`data/reference.py` is a Python twin of the millidegree trig (identical
rational spline semantics) and Bowditch adjustment, frozen before engine
qualification. Independent millidegree sine check: max error 5.6e-16 against
`math.sin` (reference-only, matching the historical acceptance). Sweep:
sin max err 1.034e-15, cos max err 1.013e-15 vs libm.

## Provenance notes

- `survey/*.omg`, `main.omg`, `controls/` — newly authored under Z47.
- `fixtures/*.input/.stdout`, `data/oracle.json` — frozen oracle set authored
  by the reference twin under this reconstruction (earlier oracle bytes were
  not recovered; tolerances 1e-9/1/1 preserved from the recorded acceptance).
- `adjust_pass.input` was upgraded to a near-closure traverse that exercises
  a real Bowditch correction and the `1:N` precision path; `irregular_pass`
  added as a second non-trivial pass case.
- Commit authors/committers: Jarod Castillo
  <43973221+Smotto@users.noreply.github.com> via `original_git` env override.
