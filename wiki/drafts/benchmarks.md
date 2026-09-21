# Benchmark records

Versioned compile-time, peak-memory, code-size, and runtime
measurements of the reference compiler, one JSON record per (subject,
target, exact rule selection) row. The harness and schema live in
[tools/benchmark](../../tools/benchmark/README.md); committed rows live
in `tools/benchmark/records/`. This note holds methodology and host
coverage; it is not a leaderboard.

## Method

`tools/benchmark/benchmark.py measure` compiles the subject with
`omega --timings --target <profile>` into a fresh temporary build
directory per sample, records the wall clock and per-child peak RSS of
each compile, keeps the published executable for its size, then runs it
with stdin closed (`/dev/null`) for the runtime leg. Samples select
optimizations only through authored `optimizations.enable(...)` calls in
their `build.omg`; the harness reads the same file to key the record, so
the empty selection is the honest default and never an implied bundle.

Subjects that `depend()` on packages (every CLI sample depends on
`source/library/std`) must pass the package-review gate before any
compile. `benchmark.py prepare` performs the one-time settlement —
`omega update`, accept the generated `pending` decisions, `omega update
--resume` — publishing the host-local `omega.lock` beside the subject.
This records project acceptance, not a source audit, and the lock
(which embeds absolute paths) is never committed.

Units are milliseconds and bytes. The schema marker
`omega-benchmark-record/1` is pinned by `tools/tests/test_benchmark.py`:
a record that drifts from the documented fields, or a README that no
longer names the current schema, fails the check.

## Host-row matrix

`python3 tools/benchmark/benchmark.py matrix` renders one row per
committed record plus one explicit row per catalogued deployment
profile (`TargetProfile::ALL` order) that no committed record covers
yet, so unavailable host legs stay visible rather than implied. Cell
vocabulary: `measured` quotes the record's own status and headline
number; `measurable` legs run on any build host that invokes
`measure`; `pending <host>` needs the named runtime environment;
`unavailable (<reason>)` carries the structural gap the leg cannot
report a number for — `uefi_x86_64` runtime waits on QEMU or hardware.
Records for targets outside `HOST_LEGS` append after the catalogued
rows. Regenerate with the `matrix` command after a row lands and paste
the table between the markers; `tools/tests/test_benchmark.py` fails
when this block drifts.

<!-- benchmark-matrix:start -->
| Target | Host leg | Subject | Selection | compile_time_ms | peak_memory_bytes | code_size_bytes | runtime_ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| linux_arm64 | linux x86_64 | wrapping_square_sum | default | measured 28129.6 ms | measured 150441984 B compile | measured 8192 B | skipped (--no-run was passed) |
| linux_x86_64 | linux x86_64 | cli_mvp | default | measured 1.68124e+06 ms | measured 246046720 B compile | measured 8192 B | measured 1.93834 ms |
| linux_x86_64 | linux x86_64 | structural_proofs | default | measured 34932.9 ms | measured 150704128 B compile | measured 8192 B | skipped (--no-run was passed) |
| linux_x86_64 | linux x86_64 | wrapping_square_sum | sel-885944b13b84 | measured 3748.43 ms | measured 84189184 B compile | measured 8192 B | measured 4.02435 ms |
| linux_x86_64 | linux x86_64 | wrapping_square_sum | sel-9c09e32a82fb | measured 29846.9 ms | measured 148590592 B compile | measured 8192 B | measured 4.17697 ms |
| macos_arm64 | linux x86_64 | wrapping_square_sum | default | measured 24453.9 ms | measured 151724032 B compile | measured 16640 B | skipped (--no-run was passed) |
| macos_x86_64 | darwin arm64 | wrapping_square_sum | default | non-applicable (no bound required root slot `macos_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `macos_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `macos_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `macos_x86_64::ProgramEntry`) |
| macos_x86_64 | macOS x86-64 host | — | — | unavailable (native realization pending; see MACOS-X64-HOST-PROFILE) | unavailable (native realization pending; see MACOS-X64-HOST-PROFILE) | unavailable (native realization pending; see MACOS-X64-HOST-PROFILE) | unavailable (native realization pending; see MACOS-X64-HOST-PROFILE) |
| windows_x86_64 | linux x86_64 | wrapping_square_sum | default | measured 24117.5 ms | measured 147505152 B compile | measured 1024 B | skipped (--no-run was passed) |
| uefi_x86_64 | darwin arm64 | wrapping_square_sum | default | non-applicable (no bound required root slot `uefi_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `uefi_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `uefi_x86_64::ProgramEntry`) | non-applicable (no bound required root slot `uefi_x86_64::ProgramEntry`) |
| uefi_x86_64 | QEMU or UEFI hardware | — | — | measurable | pending (run leg needs a UEFI runtime) | measurable | unavailable (needs QEMU or UEFI hardware) |
| cross_platform_cli | build host | — | — | measurable | measurable | measurable | pending build host |
| local_unchecked | build host | — | — | measurable | measurable | measurable | pending build host |
| alpha_bootstrap | bootstrap chain | — | — | unavailable (realized by the bootstrap chain's own compilers, not this compiler) | unavailable (realized by the bootstrap chain's own compilers, not this compiler) | unavailable (realized by the bootstrap chain's own compilers, not this compiler) | unavailable (realized by the bootstrap chain's own compilers, not this compiler) |
<!-- benchmark-matrix:end -->

Two measured linux_x86_64 rows exist: the w9 session produced the
dev-profile `omega` compile/run row for `cli_mvp` (record
`tools/benchmark/records/cli_mvp__linux_x86_64__default.json`), and a
release-profile `omega` measured `wrapping_square_sum` under a non-default
selection (`CopyPropagation` disabled; record
`wrapping_square_sum__linux_x86_64__sel-885944b13b84.json` — the first
selection-keyed row, covering the enabled/disabled dimension of the record
space).
`cli_mvp` is the canonical compile-and-run smoke subject (expected exit
0, EOF-tolerant stdin); `prime_counter` was ruled out on this revision
because its `i32` remainder operation does not legalize to a native
artifact — see that record's notes when a selection row for it lands.
Cross-target compile legs (`--no-run`) measure compile-time and
code-size but mark `runtime_ms` as `skipped`.

## Frontier: `linux_x86_64` rows resumable, hosts still uncovered

A w9 benchmarks session attempted two further `linux_x86_64` rows on
this host at `e48558bd41` — `cli_mvp` with `CopyPropagation` disabled
and a `cli_mvp` default-selection re-measure. Both compiles ran ~23.5
minutes to the same rejection:

```text
cannot realize accepted package production:
  Terminal proposal must retain every integer comparison occurrence
  exactly once
```

The failure was selection-independent and subject-independent:
`cli_mvp`'s authored code contains no integer comparisons, so the
uncovered occurrence lived in the shared `std`/entry plumbing every
`depend()`-ing subject compiles. The only subjects without a
`build.omg` dependency — `math_proofs` and `structural_proofs` — emit
no runtime code (no selected `ProgramEntry`) and one fails earlier at
checked-call selection.

The blocking gate — the comparison-occurrence producer/validator pair
tracked under CRASH-CONTRACT, the same failure `euclid_gcd`'s README
records — landed as `29ca2fd46e`. A `cli_mvp` default-selection probe
on this host at `749794ddeb` reached `published native output` in
1240655 ms, so new `linux_x86_64` rows are producible again. None has
been committed yet: the remaining frontier is the record-production
legs (`tools/benchmark/records/`) and the uncovered host rows —
`linux_arm64`, `macos_arm64`, `windows_x86_64`, and `uefi_x86_64` each
still need their named runtime environment, and `macos_x86_64` stays
structurally unavailable under MACOS-X64-HOST-PROFILE.

Update (w9): the gate resolved at `f2f39039da` — `76dc49a99e`
("distinguish selected comparison custody from builtin operations")
counts selected integer occurrences against the artifact-bound checked
scope, so ordinary builtin comparisons and generated guards no longer
need provider rows. `wrapping_square_sum` (added `3dd805679c`, the
dependency-free CLI subject) was witnessed compiling and publishing on
`windows_x86_64`, `macos_arm64`, and `linux_arm64` (~24-28s each on a
Linux x86-64 host; the subject does not reach the deep-pipeline stage
where the rejection fired). No committed records exist for those legs
yet — `measure --no-run` rows resume once `tools/benchmark` frees; the
matrix block above is current against the committed record set.
`macos_x86_64` and `uefi_x86_64` are not valid CLI-subject targets:
both fail review settlement with "no bound required root slot
`<target>::ProgramEntry`" (MACOS-X64-HOST-PROFILE owns the x86-64
macOS host-profile gap) — record them as non-applicable, not failed
compiles. **Both are now recorded that way** (BENCHMARK-REJECTED-ROW-RECORDING):
the schema carries an optional row-level `applicability`
(`{"status": "non_applicable", "reason": ...}`), `measure` turns that exact
settlement rejection into such a record instead of exiting, and the matrix
renders the pairing as its own row. A non-applicable pairing speaks only for
its `(subject, target)` pair, so it does NOT retire its host leg's projected
row — `uefi_x86_64` still shows "needs QEMU or UEFI hardware" beside the
`wrapping_square_sum` row.

Update (z113, `1a772e4ae1`, linux x86_64 host): the remaining
measurable cross-target compile legs for `wrapping_square_sum`
produced schema-valid records — `windows_x86_64` 24.3 s compile /
148.8 MiB peak RSS / 1,024 B image and `macos_arm64` 24.4 s /
153.5 MiB / 16,640 B — each via `measure --no-run --compile-samples 1`
with `validate` clean. `uefi_x86_64`, `cross_platform_cli`, and
`local_unchecked` are non-applicable for this subject: review
settlement rejects each with "no bound required root slot
`<target>::ProgramEntry`" (the subject binds only the four hosted
targets). Note `omega.lock` settles per target — `prepare` must run
once per `--target` leg before `measure`, and a failed settlement
leaves the lock's earlier accepted sections intact. The produced JSON
rows await commit under `tools/benchmark/records/` once its claim
frees; regenerating them is one `measure` invocation per target.

Update (z27, `5e2d355a02`, linux x86_64 host): the first
dependency-free proof subject is committed — `structural_proofs` ×
`linux_x86_64` default selection
(`tools/benchmark/records/structural_proofs__linux_x86_64__default.json`):
3 compile samples, median 34932.9 ms, published 8192 B artifact,
runtime `skipped` (proof machines emit no runtime code). `math_proofs`
still fails earlier at checked-call selection
(PROOF-SUBJECT-CALL-SELECTION family).

## Reading a row

`key.selection.enabled`/`disabled` name the exact rules in effect, not
a level or bundle. Comparing rows across `source_revision`s is a
compiler change plus everything else the revision carried; within one
revision, two rows differing only in the selection key isolate that
selection's cost on the subject.
