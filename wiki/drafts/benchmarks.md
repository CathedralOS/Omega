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

## Current coverage

| Subject | Target | Selection | Host |
| --- | --- | --- | --- |
| `cli_mvp` | `linux_x86_64` | `default` (empty) | Linux x86-64, dev-profile `omega` |

Measured by the w9 session on this host; the numbers live in
`tools/benchmark/records/cli_mvp__linux_x86_64__default.json`.
`cli_mvp` is the canonical compile-and-run smoke subject (expected exit
0, EOF-tolerant stdin); `prime_counter` was ruled out on this revision
because its `i32` remainder operation does not legalize to a native
artifact — see that record's notes when a selection row for it lands.

Unavailable host legs, explicit rather than absent:

- `windows_x86_64`, `macos_arm64`, `linux_arm64`: no Windows, macOS, or
  ARM64 host participated in this wave; their rows land when a matching
  host runs the same `measure` command.
- `uefi_x86_64`: requires QEMU or hardware acceptance; unavailable on
  this host.
- `peak_memory_bytes` on Windows: `os.wait4` is absent there, so the
  leg records `unavailable` with a reason instead of a guessed number.
- Cross-target compile legs (`--no-run`) measure compile-time and
  code-size but mark `runtime_ms` as `skipped`.

## Reading a row

`key.selection.enabled`/`disabled` name the exact rules in effect, not
a level or bundle. Comparing rows across `source_revision`s is a
compiler change plus everything else the revision carried; within one
revision, two rows differing only in the selection key isolate that
selection's cost on the subject.
