# Benchmark harness

`benchmark.py` measures the reference compiler on a chosen subject and
writes one versioned JSON record per (subject, target, exact rule
selection) row under `records/`. It exists so optimizer work can cite
measured compile-time, peak-memory, code-size, and runtime numbers
instead of adjectives, and so a row's provenance survives the machine
that produced it.

Requirements: Python 3.9+ and a built `omega` binary
(`cargo build -p omega`). The script is standard library only and runs
identically under PowerShell on Windows and sh on macOS/Linux; no
PowerShell or POSIX-only prerequisite is needed for the workflow
itself. Individual metric legs degrade to an explicit `unavailable`
status rather than disappearing when a host cannot provide them (see
Peak memory below).

## Measuring

```text
python3 tools/benchmark/benchmark.py measure \
    --root samples/cli/arithmetic/prime_counter/main.omg \
    --target linux_x86_64 --expected-exit 8 \
    --omega target/debug/omega
```

PowerShell uses the same invocation with `python` instead of `python3`.

Each compile sample runs `omega --timings --build-dir <fresh tempdir>
--target <target> [--accept-admissions] [--disable-optimization
<ExactName>]... <root>` from the repository root, then the published
native executable is copied out and run `--run-samples` times with
stdin from the null device. Subjects that wait on console input (the
samples' `press Enter` idiom) receive EOF immediately.

`--enable <ExactName>` augments the enabled-selection key for subjects
whose `build.omg` cannot express it; normally the enabled set is read
from the subject's sibling `build.omg` `optimizations.enable(...)`
calls, because the CLI can only disable. `--no-run` marks the runtime
leg `skipped` for cross targets the host cannot execute. New target
profiles need no code change — pass the profile name to `--target`.

## Package-review preparation

Compiling a project that `depend()`s on packages requires the
package-review gate to be settled first, otherwise every compile exits
with 'package acceptance is missing or current requirements need
review'. Settlement is interactive by design: `omega update` renders a
restartable review document under the subject's `build/package-manager/`
directory, a human edits each `pending` decision to `accept` or
`reject`, and `omega update --resume` publishes `omega.lock`.

The harness automates the local-benchmark version of that ceremony:

```text
python3 tools/benchmark/benchmark.py prepare \
    --root samples/cli/arithmetic/prime_counter/main.omg \
    --target linux_x86_64 --omega target/debug/omega
```

`prepare` runs `omega update`, rewrites every `pending` decision token
to `accept` in the generated `review-<target>.txt`, and resumes, which
publishes `omega.lock` beside the subject root. This records project
acceptance of the subject's own dependency decisions; it is not a
source audit. `measure` performs the same settlement automatically
when `omega.lock` is absent (disable with `--no-prepare`).

`omega.lock` and any `omega.admissions` are host-local generated state:
the lock embeds absolute checkout paths, so never commit either file.
`measure` removes an `omega.admissions` it created; `omega.lock` is
left in place because re-settling it costs a full `update` pass.
Delete both before any operation that requires a clean tree (for
example `tools/landing.py`).

## Record schema `omega-benchmark-record/1`

Each file in `records/` is one JSON object:

| Field | Meaning |
| --- | --- |
| `schema` | The literal string `omega-benchmark-record/1`. Bumping it requires updating this section and `SCHEMA` in `benchmark.py` together; the guard test fails on drift. |
| `recorded_utc` | UTC timestamp of the measurement run. |
| `subject` | `name`, `root` (repository-relative .omg path), and `source_revision` (full lowercase commit the subject and compiler were taken from). |
| `key` | The row identity: `target` (a target profile name like `linux_x86_64`) and `selection`, a sorted-unique `enabled`/`disabled` pair of exact rule names as `Optimization::` identifiers. The empty selection serializes as `enabled: []`, `disabled: []` and lands in the `default` filename slot. |
| `host` | `os`, `machine`, `cpu`, `logical_cpus`, `python`, `rustc`, `omega_binary`, and `omega_profile` of the measuring host. Numbers from different hosts are different rows' context, never silently mixed. |
| `metrics.compile_time_ms` | `measured`; per-sample wall-clock milliseconds plus `median_ms`, `min_ms`, and the last sample's `--timings` stage table. |
| `metrics.peak_memory_bytes` | `measured` when the platform reports per-child RSS; `compile_max_rss` and `run_max_rss` in bytes. `unavailable` with a reason on hosts without `os.wait4` (Windows). |
| `metrics.code_size_bytes` | `measured`; published executable size per sample plus `stable` (whether every sample produced the identical size). |
| `metrics.runtime_ms` | `measured` with per-sample wall-clock milliseconds, observed `exit_codes`, `exit_code_expected`, and `exit_code_match`; `skipped` for `--no-run` (e.g. a cross target with no host runtime); `unavailable` if no artifact was produced. Each non-measured status carries a `reason`. |
| `notes` | Free-form strings supplied through `--note`. |

Filenames are `<subject>__<target>__<selection>.json` where the
selection slot is `default` for an empty selection or `sel-<hash>`
otherwise, so rerunning a row overwrites its own record rather than
accumulating duplicates.

Before a row can be measured the subject must be prepared once per
target (see Package-review preparation); `measure` does this itself
unless `--no-prepare` is passed.

## Checking records

```text
python3 tools/benchmark/benchmark.py validate tools/benchmark/records/*.json
```

`validate` applies the schema above structurally; `tools/tests/test_benchmark.py`
additionally checks that every committed record passes it and that this
README still names the current schema version.

## Host coverage

The committed rows record which host produced them. Targets without a
matching host (for example `windows_x86_64` or `macos_arm64` rows
authored on a Linux machine, and the QEMU-dependent `uefi_x86_64` leg)
are reported as unavailable in the task evidence rather than implied.
When those hosts are reachable, run the same `measure` command there to
publish their rows.
