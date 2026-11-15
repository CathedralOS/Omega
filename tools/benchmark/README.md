# omega-bench

`omega-bench` measures how fast the Omega compiler and the programs it builds
run, and keeps every observation so a result can be checked later. Each
subcommand answers one question: is this compiler or program faster than that
one (`compare`, `suite`), does a retained run still hold up (`validate`, `show`,
`check`, `history`), where does the time go (`profile`), how fast is this
commit on this host (`progress`), and does compiling one more machine cost more
in a bigger program (`scaling`).

Build it with `mbx`, under the [host build budget](../docs/build_slot.md) when the
machine is shared, and run the retained executable from the checkout root:

```text
mbx build -p omega-bench
target/debug/omega-bench --help
```

Windows executables have the `.exe` suffix. The plans under [`plans/`](plans/)
are Windows-only examples: they name `.exe` paths and the `windows_x86_64`
target. On another host, copy one and change the executable suffix and target.

| Command | Answers |
| --- | --- |
| `compare <plan.json> <new-dir>` | How does one fixed workload behave under two executables? |
| `suite <suite.json> <new-dir>` | `compare` for each plan in a list, each failure kept separate. |
| `validate <run-dir>` | Is this retained run complete and self-consistent? |
| `show <run-dir>` | A Markdown table of a validated run. |
| `check <run-dir> <percent>` | Does the run establish a regression beyond this budget? |
| `history <old-run> <new-run>` | How do the candidates of two separate runs compare? |
| `profile <plan.json> <baseline\|candidate> <wpr\|perf-stat\|perf-record\|selected-bodies> <new-dir>` | Where does one checked execution spend its time? |
| `progress [options]` | How fast is this commit's release compiler on this host? |
| `scaling --omega <omega> [options]` | Does a bigger program make each machine cost more to check? |
| `dominance-summary <graph-run-dir>` | Validate and summarize retained pairs from the private checker graph driver. |
| `proof-plan <source\|prepare\|prove> <baseline> <candidate> <main.omg> <new-plan.json> [options]` | Prepare a portable comparison of the proof benchmark's source route or timed replay. |
| `proof-square <new-directory> [--identity-build]` | Reconstruct the pinned square-sum workload and verify its source hashes. |
| `selected-body-inputs <baseline-omega> <new-directory>` | Retain the original compiler and two unchanged Build continuation workloads. |
| `selected-body-plan <profile-driver> <copied-main.omg> <target[,target]> <new-plan.json>` | Create its ordinary checked capture plan with explicit executable and targets. |
| `selected-body-source-plan <baseline-omega> <copied-main.omg> <target[,target]> <new-plan.json>` | Retain the original CLI control and freeze its output oracle for an ordinary comparison. |

Any command may start with `--address-space-gib N` to change the Linux
address-space cap (default 64 GiB) every measured process runs under; records
name the value used.

## Proof preparation inputs

The proof preparation helpers are part of this Rust executable on every
platform and need no PowerShell. Run them from the checkout root and pass the
real baseline and candidate executable paths, with `.exe` on Windows.
`proof-plan` writes an ordinary plan for `compare`, which still handles sample
ordering, closing stdin, resource capture and correctness checks.

In `source` mode, pass `--target windows_x86_64`, `linux_x86_64`, `linux_arm64`
or `macos_arm64`. Its oracle requires the route probe's single accepted part
and accepted final verdict, without warnings. It measures the
source-to-product route, not Build execution or native publication. `prepare` and `prove` instead
require `--repeats N --work-units N --checksum VALUE`, matching the replay's
reported work and checksum. The replay executable receives the chosen mode as
an explicit argument. Both modes default to one warmup and eight pairs;
`--pairs N` changes the pair count. Existing plans are never overwritten.

`proof-square` reads commit `9b4898865ecd5789bea4def6c2da078192a4ab82`
through Git and adds the exact literal qualifications used by the retained
measurements. It checks both resulting source hashes before creating the new
directory. `--identity-build` also removes the six pinned optional
optimization selections, keeping the arithmetic and checksum. A shallow
checkout missing the pinned commit must fetch it first. See the
[proof preparation report](../../wiki/drafts/measurements/proof_preparation_2026_11_14.md)
for the measured boundary, source distributions and reproduction commands.

## Selected-body copy attribution

The `selected-bodies` profile backend runs the kept opt-in capture driver
through the same runner, oracle and resource capture as other profiles. Its
private log and typed snapshots record real continuation copies, and the
profile record hashes every snapshot. The ordinary compiler is not
instrumented.
See [the experiment instructions](selected-bodies/README.md) for the fixed source,
portable input preparation, capture patch, replay, and measurement limits.

## Paired comparisons

A plan fixes the commands, inputs, what is timed and the expected output
(the oracle) before either executable runs. `compare` alternates the two executables
in AB/BA pairs, checks every sample against the oracle, and writes the result to
a new directory:

```text
target/debug/omega-bench compare plan.json build/performance/my-comparison
```

The output directory must be new: remeasurement never overwrites a previous
run. Never build or measure anything else during the timed experiment. A plan can
name the same executable twice for an A/A check of the tool; that tests the capture path, but one A/A comparison does not calibrate
every workload or set noise limits.

Paths in a plan resolve as follows:

- `working_directory` is relative to the plan file.
- Executables, `inputs`, and compiler `source` are relative to that directory.
- The result directory is relative to the invoking shell's directory.

A plan's `measurement.kind` selects one of three things to time.

### Compiler mode

`kind: "compiler"` times one compiler process per sample, from launch to exit,
the same way process mode times a native program. The
[length plan](plans/compiler-length.windows.json) compiles the `length/local`
fact with two retained compilers at `build/performance/baseline/omega.exe` and
`build/performance/candidate/omega.exe`; put the producers you want to compare
at those paths, or edit the plan to name your own.

Compiler mode records each executable's `--version` output and SHA-256. `omega`
does not report its optimization level, so the plan author must pick release
compilers: a debug compiler measures debug-compiler speed. Compiler mode
requires the measuring host's native target, never builds compilers and never
accepts package decisions. The working checkout's revision is not taken as
the compiler's revision.

`measurement.library_root` selects the same toolchain library for both
compilers through `OMEGA_LIBRARY_ROOT`, and that directory is hashed with the
inputs, so a kept compiler's built-in development-library path cannot
silently pick up another checkout's library.

A project that depends on packages needs an accepted lock for each compiler:
settle it beforehand with `omega packages update`, accepting the pending
decisions and running `omega packages update --resume` (see the
[packages guide](../../wiki/language_guide/packages.md#install-and-update)).
Locks belong to one compiler, so for two different compiler builds prepare two
private copies of the same project and review each with its own compiler. Set optional `measurement.candidate_source` to the second entry;
baseline uses `source`. Both entry files must have identical bytes and appear in
`inputs`, along with both accepted locks and every consumed library and
dependency path.

Each sample compiles into its own empty build directory with `--timings`,
requires exactly one `published native output to <path>` line naming a file
inside that directory, hashes the native image, and runs it against the exact
authored exit/stdout/stderr oracle. The native check is measured separately
and is not in the compile timer. `--timings` prints one human-readable
`<ms> ms  <phase>` line per compiler phase on stderr; the runner keeps those as
extra `stage_ms:<phase>` metrics, not as the primary measurement. Phase
scopes are inclusive and may overlap, so do not sum nested rows.
`require_identical_artifacts: true` also requires every warmup and measured
image to have identical bytes. Set it to false when code generation changes on
purpose; the native result oracle is still required.

The plan keeps extra arguments and environment overlays. Compiler
arguments accept `--offline` and repeatable
`--disable-optimization <ExactName>`, so each compiler can disable its own
optimizations; other options are rejected so an extra source path or subcommand
cannot silently change what is measured.
Full compiler logs are retained with each sample. The runner does not infer
enabled optimization rules from source.

Fresh output directories do **not** mean cold operating-system caches. Declared
inputs and executables are hashed before and after each sample, outside the
timer, and that hashing warms the filesystem cache. These are repeated fresh-output builds.

### Runtime process latency

`kind: "process"` times a native program's complete launch-to-exit latency.
Both command specifications take an executable, an argument vector, and
optional environment overlays. There is no shell interpolation.

```json
{
  "schema": "omega-performance-plan/1",
  "name": "checked native workload",
  "working_directory": ".",
  "inputs": ["input.txt"],
  "baseline": {"program": "before.exe", "arguments": ["input.txt"]},
  "candidate": {"program": "after.exe", "arguments": ["input.txt"]},
  "warmups": 2,
  "pairs": 12,
  "timeout_ms": 60000,
  "measurement": {
    "kind": "process",
    "stdin": "",
    "expected": {"exit_code": 0, "stdout": "checksum: 1234\n", "stderr": ""}
  }
}
```

The expected output must depend on the computed result: a clean exit from a
program that throws away its accumulator says nothing about arithmetic speed.
Arguments and stdin should supply typical runtime inputs; confirm the
generated code still does the intended work, because the harness cannot
tell that from a checksum alone. Empty stdin means EOF, nonempty stdin is
sent byte for byte, and output comparisons match line endings exactly.

The process timer includes startup, output capture, waiting, and process-tree
cleanup. This is command latency, **not isolated kernel execution time**. For
short kernels, batch useful work inside the child and check its final result,
or use a timed region.

### Runtime timed regions

`kind: "reported_work"` accepts one JSON object on stdout, empty stderr, and
exit 0. The child reports its own timed region:

```json
{"schema":"omega-timed-work/1","elapsed_ns":5000000,"work_units":1000000,"checksum":"1234"}
```

The plan fixes the expected work count and checksum:

```json
{"kind":"reported_work","stdin":"","work_units":1000000,"checksum":"1234"}
```

The primary metric is `reported_work_ns_per_unit`; the enclosing process wall
time is retained separately. The runner refuses a sample with zero time, zero work, an unexpected work
count, a wrong checksum, an unknown schema or field, or a region longer than
its process. Setup and verification belong outside the child's timer,
and the checksum must depend on the timed computation. Child-reported time is
only as good as the child's timing code.

The opt-in `benchmark-probe` fixture shows this protocol with a
wrapping-square sum sized and seeded at run time, a used accumulator, and Rust
`black_box`. It tests the tool, not Omega-generated code:

```text
mbx build -p omega-bench --features fixtures
target/debug/omega-bench compare tools/benchmark/plans/probe.windows.json build/performance/probe-aa
```

## Records and interpretation

`journal.jsonl` records a start event, identities, every sample, and completion
or failure, flushed to disk after each event. Each sample retains:

- Warmup/measured role, pair number, and actual baseline/candidate execution order.
- Primary metric, executed arguments, process duration, exit, and stream hashes.
- Complete stdout/stderr in its own directory, including failed oracle output.
- Every compiler stage table, native image hash, byte count, and native observation.

Warmups are checked and kept but left out of the comparison. Measured rounds
alternate AB then BA. `report.json` summarizes medians, ranges, median absolute
deviations, paired absolute and percentage changes, and a deterministic
4,000-resample paired percentile bootstrap interval for the median difference
and percent change. Means and nearest-rank p95/p99 describe the samples;
a small run does not estimate tails. Resampling keeps each pair together.
Fewer than six pairs are labelled insufficient; six pairs do not guarantee
enough statistical power. An interval crossing zero is inconclusive. Even a
separated interval describes this run, not every host or workload,
and autocorrelation and drift can break its assumptions.

A timeout, incorrect output, compiler error, changed declared input or
executable, or output overflow fails the run and leaves its journal; it does
not produce a successful report. A crash or outside interruption leaves a
journal without a `complete` event, so readers must require that event and its
report digest before trusting the report. Slow valid samples are never
discarded.

Input paths may be individual files or recursively hashed directories, and the
result directory must be outside declared input directories. The plan,
executable bytes, and declared file set are checked around each sample. These
catch lasting changes, not edits undone between observations, so still use an
isolated worktree and unchanging kept compilers. Only declared
inputs are covered; dependencies are not discovered automatically.

Environment overlays are recorded; other inherited values are not, so
credentials stay out. The journal records host OS,
architecture, available parallelism, the Windows processor identifier, a hashed
host identifier and a whitelist of inherited performance settings. The runner
does not shield against other load, pin CPU frequency or affinity, or infer
cache state; record relevant host conditions with the result.

Processes use the shared `bounded-process` runner: a wall timeout, 1 MiB per
captured stream, process-tree cleanup, 8 GiB process and aggregate memory
ceilings, the Linux address-space ceiling, a 1 GiB file-size ceiling, a 1,024
open-file ceiling, and a 64-process ceiling. Each platform enforces these as that
library documents. The limits and capture overhead are part of the recorded
measurement environment.

### CPU, resident memory and faults

Every captured process turns on the bounded runner's resource observations.
Each metric names its provider, scope, units and measured/unavailable status. A measured zero is kept; an unsupported metric never
becomes zero.

| Host | Provider and scope | Observations |
| --- | --- | --- |
| Windows | `GetProcessTimes`, `GetProcessMemoryInfo`, `QueryProcessCycleTime`; primary process only | User/system CPU ns, peak working set bytes, total page faults, cycles |
| Linux | `wait4` for the exact child, with reaped descendant accounting defined by the OS | User/system CPU ns, peak RSS (KiB converted to bytes), minor/major faults |
| macOS | `wait4`, same accounting scope | User/system CPU ns, peak RSS bytes, minor/major faults |

Resident memory is not allocator-requested memory or Windows committed memory.
Windows total faults are not labelled major faults. CPU cycles are not
converted to time with an assumed frequency. Compiler and native-check
resources stay separate. Stage times, phase counts and artifact bytes are
compared as extra metrics. [Provider research](research.md) cites the
operating-system documentation for each provider.

## Suites, validation and regression checks

A suite keeps each workload's sample count and failure separate:

```json
{"schema":"omega-performance-suite/1","plans":["compute.json","memory.json"]}
```

```text
target/debug/omega-bench suite suite.json build/performance/suite
target/debug/omega-bench validate build/performance/suite/case-00
target/debug/omega-bench show build/performance/suite/case-00
target/debug/omega-bench check build/performance/suite/case-00 5
target/debug/omega-bench history build/performance/old-run build/performance/new-run
```

Suite paths resolve relative to the suite file. Each case gets a fresh
directory; `suite.json` and `summary.md` retain successful cases even when
another fails. The [runtime suite](plans/runtime-suite.windows.json) runs the
`omega-bench-workload` reference kernels (compute, indexed memory, indirect
calls and allocation) with runtime units and seed and checked checksums. They
are Rust kernels that test the tool, not Omega-generated code; for product
measurements, replace the commands with matching application binaries.

`validate` requires a completed `/2` journal, the report digest, the ordered
sample set, raw stream digests and oracles, compiler identities, and retained
native artifact bytes, and recomputes the summary from those samples. `show`
prints a Markdown table only after validation passes. This checks consistency;
it is no signature against someone rewriting the whole run.
Keep the whole run directory for validation; a copied report is not enough.

`check` applies the paired percentage interval to a finite, nonnegative budget.
Exit 0 means the interval is within budget; 3 means it establishes a regression
beyond budget; 2 means inconclusive or fewer than six pairs; 1 means invalid
evidence or arguments. Treat 2 separately in automation. Extra metrics are
reported but never replace the primary metric, and no timing samples are
trimmed.

`history` compares candidate observations from two separately completed runs.
It requires the same identified host, inherited measurement settings, workload,
arguments, oracle, and declared input bytes, and uses independent resampling:
separate runs are never treated as interleaved pairs. Host drift and correlation over time
still limit it; use a fresh paired comparison to decide on a regression.

## Profiles

A profile runs one plan's baseline or candidate separately under an external
profiler and checks its output. Its duration never enters a timing comparison:

```text
target/debug/omega-bench profile compiler-plan.json candidate wpr build/performance/cpu
target/debug/omega-bench profile compiler-plan.json candidate perf-stat build/performance/counters
target/debug/omega-bench profile compiler-plan.json candidate perf-record build/performance/stacks
```

`wpr` requires Windows, the installed CPU profile and permitted tracing. The
tool uses a unique named instance, saves `cpu.etl`, and cancels only its own
instance if stopping fails. It never cancels the default session, elevates
permissions or changes system settings. `perf-stat` and `perf-record` require
Linux and `/usr/bin/perf` or `/bin/perf` with permitted events. `perf-stat`
retains cycles, instructions, branches, misses and faults with runtime and
multiplexing metadata; refused events remain unavailable. `perf-record` saves
DWARF call stacks in `perf.data`. Open the retained traces in WPA or
`perf report`. Raw artifacts and digests accompany `profile.json`. When the profiler
is not usable (exit 2, status `unavailable`) or the run or extraction fails
(status `failed`), the status says why. A forcibly interrupted run can leave
`started`, which means incomplete.

## Progress records

`omega-bench progress` records how fast one commit's release compiler is on
this host, so a dashboard can chart speed across `main`'s history. To count how
many programs compile, use `cargo xtask sweep`. Run it from the checkout root:

```text
target/debug/omega-bench progress --results <dir>
```

It builds `omega` with `mbx build --release -p omega` (`--omega <path>`
measures an existing release binary instead), then runs each workload of the
frozen set `omega-progress-set/1` with stdin closed: one warmup, `--aa-runs`
(default 3) alternating A/A pairs of the same binary, then `--runs` (default 5)
measured runs. Each check gets a fresh `--build-dir`; the native workload's
`build/` is removed before every run. Untimed, the two std-dependent projects
first get a fresh package review for this compiler with every pending decision
accepted, recorded under `preparations`. `--only <workload>` selects one
workload.

| Workload | Command |
| --- | --- |
| `one_line_check` | `omega --check` on [`workloads/one_line`](workloads/one_line/main.omg) |
| `addr_algebra_check` | `omega --check` on `tests/fixtures/pass/arithmetic/addr_algebra` |
| `medium_sample_check` | `omega --check` on `samples/cli/arithmetic/prime_counter` |
| `heavy_proof_check` | `omega --check` on `tests/fixtures/pass/proofs/runtime_inductive_gauss_sum_exit` |
| `native_run_both` | `omega run --both` on `samples/cli/arithmetic/wrapping_square_sum` |

Changing a workload means bumping `BENCHMARK_SET` in `src/progress.rs`;
records from different sets cannot be compared.

One JSON file per run, `<commit12>__<target>__<utc>.json`, schema
`omega-progress-record/1`, followed by a one-line summary on stdout:

| Field | Meaning |
| --- | --- |
| `key` | `commit`, `dirty`, `source_trees` (`omega-rust`, `source` tree hashes, the compiler's real identity when the commit only touches tools), `host` (hostname), `target`, `build_profile` (`release`), `compiler_version` (`omega --version`), `recorded_utc`. |
| `host_detail`, `compiler`, `settings` | OS/arch/logical CPUs; binary path, SHA-256 and release-build wall time; warmup, A/A and run counts plus `address_space_bytes`, the Linux address-space cap every measured process ran under. |
| `rows[]` | One per process: `workload`, `phase` (`warmup`, `aa_a`, `aa_b`, `measured`), `index`, `arguments`, `exit_code`, `ok`, `wall_s`, `user_cpu_s`, `sys_cpu_s`, `peak_rss_bytes` (job/process-tree peak; `null` where the platform gives none), `instructions_u` (user-space instructions from `perf stat -e instructions:u` on Linux when `/usr/bin/perf` or `/bin/perf` exists, otherwise `null`; perf then sits in the timed process tree), `load_average_start` and `load_average_end` (1/5/15-minute load averages around the run; `null` on Windows), `stderr_tail` on failure. |
| `summary[]` | Per workload over measured rows: `median_wall_s`, `median_cpu_s` (user+sys), `median_peak_rss_bytes`, `measured_range_pct` ((max-min)/median), `aa_spread_pct` (absolute difference of the A and B wall medians over their mean), `median_instructions_u`, `aa_instructions_spread_pct` (the same over instructions), the workload's first `load_average_start` and last `load_average_end`, and `all_ok`. |

An A/A spread larger than the change you are charting means the host was too
noisy to show it. When other workers share the VM, wall and CPU time swing by
seconds while user-space instruction counts move under one percent, so chart
`instructions_u` on Linux and use load averages to flag noisy runs.
The command exits 1 after writing the record when any run fails.

## Scaling check

Most slowdowns found so far came from a check that walks the whole program for
each question, so each machine costs more in a bigger program.
`omega-bench scaling` checks the one-line program plus N uncalled machines for
each N in `--sizes` and fails when the cost per added unit in the last interval
exceeds the first by more than `--max-ratio` (default 1.3). A linear compiler
stays near 1.

```text
target/debug/omega-bench scaling --omega target/release/omega
target/debug/omega-bench scaling --omega target/release/omega --shape citations --sizes 0,100,200,400
```

`--shape machines` (the default, sizes `0,1000,2000`) adds uncalled machines
and catches checks that scan the whole program per machine. `--shape citations`
(default sizes `0,8,16`) puts N calls of one local lemma in a single state and
catches checks that replay a state from its first statement per call. On main
`06af8fee3fe` (omega-linux, release):

| Shape | Cost per added unit, first → last interval | Ratio |
| --- | --- | ---: |
| machines, 0/1000/2000 | 14.3 M → 15.6 M instructions | 1.09 |
| citations, 0/100/200/400 | 49 M → 174 M → 454 M instructions | 9.24 (fails) |

Cost is `instructions:u` from `perf stat` on Linux and CPU time elsewhere, so
run it on `omega-linux`. A change that removes a whole-program walk shows its
ratio before and after; a change that adds one fails here.

To find the responsible walk, profile the largest size with samply on Linux
and read the heaviest frames under the checking workers:

```text
samply record --save-only -o scaling.json.gz -- target/release/omega --check <dir>/main.omg < /dev/null
```

## Checking the tool itself

The process fixtures sit behind the `fixtures` feature and no CI job runs
them, so run them when you change the tool:

```text
mbx test -p omega-bench --features fixtures
mbx clippy -p omega-bench --all-targets --features fixtures -- -D warnings
```

They cover known output and checksums, compiler and native transport, pairing,
warmup exclusion, overwrite refusal, input mutation, failed compilation, wrong
exit or output, timeout, overflow, and incomplete-oracle rejection. The fake
compiler fixture is labelled as fake; comparing real compilers is a separate
integration test.
