# RC native matrix — windows_x86_64 row

Row status: **open** — no Windows x86-64 runner has executed the emitted
PE programs. Recorded at revision `e76d715c8e` (2026-09-20) from a Linux
x86-64 host; this file exists to name the open row, the exact procedure a
Windows runner follows, and the cross-target coverage already witnessed.
Per `wiki/drafts/rust_compiler_completion.md`, cross-target byte generation
on a different host cannot replace matching-host execution, and a missing
runner leaves the row open; nothing below is a pass claim.

## Required observation (per the contract runner table)

| Runner | Product identity | Required native observation |
| --- | --- | --- |
| Windows x86-64 | `windows_x86_64` | Directly execute the emitted PE x86-64 programs. |

Unlike the linux_arm64 row, this runner has no emulator escape hatch:
execution must be direct on a Windows x86-64 host.

## Runner procedure

From a clean checkout of the same commit on Windows x86-64, with the pinned
toolchain from `rust-toolchain.toml` (rustup selects it) and Python 3:

```powershell
python tools/release/release_record.py run --target windows_x86_64 --all
```

The record lands at `tools/release/records/windows_x86_64__<commit>__<stamp>.json`;
`python tools/release/release_record.py check <file>` re-validates it.
`--expect-skip GATE|TEST|REASON` must declare every test the run skips;
an unlisted skip keeps the closure open. If a gate set is run in pieces,
repeat `run --target windows_x86_64 --gate <NAME>` per gate; the row is
recorded only when every named gate passed on that host.

Cross-reference for the Linux x86-64 row's per-leg detail and the known
fixture-migration failure families (expected to replay on Windows):
`wiki/drafts/rc_native_matrix_linux_x86_64.md`.

## Cross-target coverage already witnessed on Linux x86-64

Useful coverage, not a row substitute:

- The windows_x86_64 backend emits PE artifacts from Linux: the
  dependency-free benchmark subject `samples/cli/arithmetic/wrapping_square_sum`
  compiles and publishes on `windows_x86_64` (witnessed at `f2f39039da`,
  ~23.8s compile, 1,024-byte code section, per the benchmark row notes in
  `TASKS.md`).
- The `omega-native-differential-test` crate exercises the windows_x86_64
  pipeline cross-target on any host; its host-agnostic legs run on Linux
  today but its PE execution legs remain unobserved by definition here.

Re-checked at `c93cceb9ca` (Linux x86-64): the row remains **open** — no
Windows x86-64 runner exists, and the host-free legs above cannot be
re-executed at this revision because the workspace is red before the test
crate builds: `external-roots` fails `cargo check --lib` with
`E0432 unresolved import effects::ComponentEraJournalRoster` in
`program_local/program_local_roots/epoch_cohorts.rs:9` (in-flight
ENTRY-CONTENT-ROOTS lane; not this row's repair).

Re-checked at `12579822068` (Linux x86-64): the row remains **open** — no
Windows x86-64 runner exists. The `external-roots` blocker above is
repaired (`cargo check -p external-roots --lib` is green), but the
cross-compile witness has moved: `tools/benchmark/benchmark.py measure
--root samples/cli/arithmetic/wrapping_square_sum/main.omg --target
windows_x86_64 --no-run` now fails after `omega.lock` settlement, ~24s
into the compile leg, with `native artifact semantic entry contract
failed: settlement did not bind its retained semantic contract:
SemanticContract`. The same subject+flow on `linux_x86_64` compiles and
publishes green (median 33.8s, 8,192-byte code section), so the moved
failure is specific to the windows_x86_64 realization path — a
native-emission-side regression relative to the `f2f39039da` witness, for
the semantic-entry-contract owner lane (not this row's repair; the row's
open condition is unchanged).

## Substrate integrity note for the row's owner

`tools/release/release_record.py` `command_run` does not currently check
that the running host can satisfy the requested runner before marking the
row `recorded`: `run --target windows_x86_64` executed on a Linux host
would write a record claiming the row, and `check` would accept it (host
fields are recorded but not validated against the runner). Until that
guard lands — owned by the RC-RELEASE-RECORD-SUBSTRATE lane (claim fence
at the time of writing) — a windows_x86_64 record's `host.os`/`host.machine`
fields are the only evidence that execution was direct, and reviewers must
read them explicitly.
