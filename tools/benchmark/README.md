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
| `profile <plan.json> <baseline\|candidate> <wpr\|perf-stat\|perf-record> <new-dir>` | Where does one checked execution spend its time? |
| `progress [options]` | How fast is this commit's release compiler on this host? |
| `scaling --omega <omega> [options]` | Does a bigger program make each machine cost more to check? |

Any command may start with `--address-space-gib N` to change the Linux
address-space cap (default 64 GiB) every measured process runs under; records
name the value used.

## Paired comparisons

A plan fixes the commands, input closure, measurement boundary and correctness
oracle before either executable runs. `compare` alternates the two executables
in AB/BA pairs, checks every sample against the oracle, and writes the result to
a new directory:

```text
target/debug/omega-bench compare plan.json build/performance/my-comparison
```

The output directory must be new: remeasurement never overwrites a previous
run. Keep builds and competing measurements outside the timed experiment. A run
can name the same executable in both slots for an A/A instrument check; that
validates the capture path, but a single A/A comparison does not calibrate
every workload or establish noise limits.

Paths in a plan resolve as follows:

- `working_directory` is relative to the plan file.
- Executables, `inputs`, and compiler `source` are relative to that directory.
- The result directory is relative to the invoking shell's directory.

A plan's `measurement.kind` selects one of three boundaries.

### Compiler mode

`kind: "compiler"` times one compiler process per sample, from launch to exit,
the same way process mode times a native program. The
[length plan](plans/compiler-length.windows.json) compiles the `length/local`
fact with two retained compilers at `build/performance/baseline/omega.exe` and
`build/performance/candidate/omega.exe`; put the producers you want to compare
at those paths, or edit the plan to name your own.

Compiler mode records each executable's `--version` output and SHA-256. `omega`
reports no optimization level, so choosing release-built producers is the plan
author's job: a debug compiler measures debug-compiler speed. Compiler mode
requires the measuring host's native target, never builds compilers and never
accepts package decisions. The working checkout's revision is not treated as
the producer's revision.

`measurement.library_root` selects the same toolchain library for both
compilers through `OMEGA_LIBRARY_ROOT`, and that directory joins the hashed
input closure, so a retained compiler's embedded development-library path
cannot silently select a different checkout's library.

A project that depends on packages needs an accepted lock for each compiler:
settle it beforehand with `omega packages update`, accepting the pending
decisions and running `omega packages update --resume` (see the
[packages guide](../../wiki/language_guide/packages.md#install-and-update)).
Locks are compiler-specific, so for two different compiler builds prepare two
private copies of the same authored project and review each with its own
producer. Set optional `measurement.candidate_source` to the second entry;
baseline uses `source`. Both entry files must have identical bytes and appear in
`inputs`, along with both accepted locks and every consumed library and
dependency path.

Each sample compiles into its own empty build directory with `--timings`,
requires exactly one `published native output to <path>` line naming a file
inside that directory, hashes the native image, and runs it against the exact
authored exit/stdout/stderr oracle. Native verification is observed separately
and does not enter the compile timer. `--timings` prints one human-readable
`<ms> ms  <phase>` line per compiler phase on stderr; the runner keeps those as
additional `stage_ms:<phase>` metrics, not as the primary measurement. Phase
scopes are inclusive and may overlap, so do not sum nested rows.
`require_identical_artifacts: true` additionally requires all warmup and
measured images to have identical bytes. Set it to false for intentional
code-generation changes; the native result oracle remains mandatory.

Extra command arguments and environment overlays are retained in the plan.
Compiler command arguments accept `--offline` and repeatable
`--disable-optimization <ExactName>`, so optimizer-disable arguments can be
supplied independently to the two compilers; other options are rejected so an
extra source path or subcommand cannot silently replace the measured product.
Full compiler logs are retained with each sample. The runner does not infer
enabled optimization rules from source.

Fresh output directories do **not** mean cold operating-system caches. Declared
inputs and executables are hashed before and after each sample, outside timing,
which itself warms the filesystem cache. These are repeated fresh-output builds.

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

The exact byte oracle must consume the computed result: a clean exit from a
program that discards its accumulator cannot establish arithmetic throughput.
Arguments and stdin should supply representative runtime inputs; confirm the
emitted code still performs the intended work, because the harness cannot
establish that from a checksum alone. Empty stdin selects EOF, nonempty stdin is
transferred as authored bytes, and line endings in output comparisons are exact.

The process timer includes startup, output capture, waiting, and process-tree
cleanup. This is command latency, **not isolated kernel execution time**. For
short kernels, batch useful work within the child and check its final result,
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
time is retained separately. Zero time, zero work, an unexpected work count, an
incorrect checksum, unknown schema or fields, or a region longer than the
enclosing process observation refuses the sample. Setup and verification
should sit outside the child's timer, and the checksum must depend on the timed
computation. Child-reported time relies on that instrumentation being correct.

The opt-in `benchmark-probe` fixture demonstrates this protocol with a
runtime-sized wrapping-square sum, runtime seed, consumed accumulator, and Rust
`black_box`. It is an instrument test, not an Omega generated-code benchmark:

```text
mbx build -p omega-bench --features fixtures
target/debug/omega-bench compare tools/benchmark/plans/probe.windows.json build/performance/probe-aa
```

## Records and interpretation

`journal.jsonl` records a start event, identities, every sample, and completion
or failure. It is flushed to disk after each event. Each sample retains:

- Warmup/measured role, pair number, and actual baseline/candidate execution order.
- Primary metric, executed arguments, process duration, exit, and stream hashes.
- Complete stdout/stderr in its own directory, including failed oracle output.
- Every compiler stage table, native image hash, byte count, and native observation.

Warmups are checked and retained but excluded from comparison. Measured rounds
alternate AB then BA. `report.json` summarizes medians, ranges, median absolute
deviations, paired absolute and percentage changes, and a deterministic
4,000-resample paired percentile bootstrap interval for the median difference
and percent change. Means and nearest-rank observed p95/p99 describe the actual
samples; a small run does not estimate population tails. Resampling keeps the
pair intact. Fewer than six pairs are explicitly labelled insufficient; six
pairs are not a promise of adequate statistical power. An interval crossing
zero is inconclusive. Even a separated interval describes this run, not every
host or workload, and autocorrelation and drift can violate its assumptions.

A timeout, incorrect output, compiler error, changed declared input or
executable, or output overflow fails the run and leaves its journal; it does
not produce a successful report. A crash or external interruption leaves a
journal without a `complete` event, so consumers must require that event and
its report digest before treating the report as completed evidence. No slow
valid samples are discarded.

Input paths may be individual files or recursively hashed directories, and the
result directory must be outside declared input directories. The plan,
executable bytes, and declared file set are checked around each sample. These
checks detect persistent changes, not transient edits restored between
observations; an isolated worktree and immutable retained producers remain the
working practice. Only declared inputs are covered; this is not automatic
dependency discovery.

Environment overlays are recorded. Other environment values are inherited and
are not dumped, to avoid recording credentials. The journal records host OS,
architecture, available parallelism, the Windows processor identifier, a hashed
host identifier and a whitelist of inherited performance settings. The runner
does not isolate hardware contention, pin CPU frequency or affinity, or infer
cache state; record relevant host conditions alongside the result.

Processes use the shared `bounded-process` runner: a wall timeout, 1 MiB per
captured stream, process-tree cleanup, 8 GiB process and aggregate memory
ceilings, the Linux address-space ceiling, a 1 GiB file-size ceiling, a 1,024
open-file ceiling, and a 64-process ceiling. Platform enforcement follows that
library's contracts. These limits and capture overhead are part of the recorded
measurement environment.

### CPU, resident memory and faults

Every captured process opts into the bounded runner's native observations.
Resource metrics include their provider, scope, units and measured/unavailable
status. A measured zero is retained; an unsupported metric never becomes zero.

| Host | Provider and scope | Observations |
| --- | --- | --- |
| Windows | `GetProcessTimes`, `GetProcessMemoryInfo`, `QueryProcessCycleTime`; primary process only | User/system CPU ns, peak working set bytes, total page faults, cycles |
| Linux | `wait4` for the exact child, with reaped descendant accounting defined by the OS | User/system CPU ns, peak RSS (KiB converted to bytes), minor/major faults |
| macOS | `wait4`, same accounting scope | User/system CPU ns, peak RSS bytes, minor/major faults |

Resident memory is not allocator-requested memory or Windows committed memory.
Windows total faults are not labelled major faults. CPU cycles are not
converted to time using an assumed frequency. Compiler and native verification
resources remain separate. Stage times, phase counts and artifact bytes are
compared as additional metrics. [Provider research](research.md) records the
operating-system contract sources.

## Suites, validation and regression checks

The suite schema keeps each workload's denominator and failure separate:

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
are Rust reference kernels for the instrument, not evidence of Omega
generated-code throughput; replace the commands with matched application
binaries for product measurements.

`validate` requires a completed `/2` journal, the report digest, the ordered
sample set, raw stream digests and oracles, compiler identities, and retained
native artifact bytes, and recomputes the summary from those samples. `show`
emits a Markdown table only after validation. This is consistency validation,
not a signature or protection from an adversary rewriting the entire evidence
set. Keep the whole run directory for validation; copying just a report is
insufficient.

`check` applies the paired percentage interval to a finite, nonnegative budget.
Exit 0 means the interval is within budget; 3 means it establishes a regression
beyond budget; 2 means inconclusive or fewer than six pairs; 1 means invalid
evidence or arguments. Treat 2 separately in automation. Additional metrics are
reported, not substituted for the primary gate, and no timing samples are
trimmed.

`history` compares candidate observations from two separately completed runs.
It requires the same identified host, inherited measurement settings, workload,
arguments, oracle, and declared input bytes, and uses independent resampling:
separate runs are never treated as interleaved pairs. Host drift and temporal
correlation remain limitations; a fresh paired comparison is the regression
decision tool.

## Profiles

A profile is a separate, correctness-checked execution of one plan's baseline
or candidate under an external profiler. Its duration never enters an
uninstrumented timing comparison:

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
`perf report`. Raw artifacts and digests accompany `profile.json`. Capability
refusal (exit 2, status `unavailable`) and execution or extraction failure
(status `failed`) are explicit statuses with reasons. A forcibly interrupted
process can leave `started`; that is incomplete.

## Progress records

`omega-bench progress` records how fast one commit's release compiler is on
this host, so a dashboard can chart speed over `main`'s history. It does not
count how many programs compile; `cargo xtask sweep` does that. Run it from the
checkout root:

```text
target/debug/omega-bench progress --results <dir>
```

It builds `omega` with `mbx build --release -p omega` (`--omega <path>`
measures an existing release binary instead), then runs each workload of the
frozen set `omega-progress-set/1` with stdin closed: one warmup, `--aa-runs`
(default 3) alternating A/A pairs of the same binary, then `--runs` (default 5)
measured runs. Each check gets a fresh `--build-dir`; the native workload's
`build/` is removed before every run. The two std-dependent projects first get
a fresh package review for this compiler, every pending decision accepted,
untimed, and recorded under `preparations`. `--only <workload>` selects one
workload.

| Workload | Command |
| --- | --- |
| `one_line_check` | `omega --check` on [`workloads/one_line`](workloads/one_line/main.omg) |
| `addr_algebra_check` | `omega --check` on `tests/fixtures/pass/arithmetic/addr_algebra` |
| `medium_sample_check` | `omega --check` on `samples/cli/arithmetic/prime_counter` |
| `heavy_proof_check` | `omega --check` on `tests/fixtures/pass/proofs/runtime_inductive_gauss_sum_exit` |
| `native_run_both` | `omega run --both` on `samples/cli/arithmetic/wrapping_square_sum` |

Changing a workload means bumping `BENCHMARK_SET` in `src/progress.rs`;
records of different sets are not comparable.

One JSON file per run, `<commit12>__<target>__<utc>.json`, schema
`omega-progress-record/1`, followed by a one-line summary on stdout:

| Field | Meaning |
| --- | --- |
| `key` | `commit`, `dirty`, `source_trees` (`omega-rust`, `source` tree hashes, the compiler's real identity when the commit only touches tools), `host` (hostname), `target`, `build_profile` (`release`), `compiler_version` (`omega --version`), `recorded_utc`. |
| `host_detail`, `compiler`, `settings` | OS/arch/logical CPUs; binary path, SHA-256 and release-build wall time; warmup, A/A and run counts plus `address_space_bytes`, the Linux address-space cap every measured process ran under. |
| `rows[]` | One per process: `workload`, `phase` (`warmup`, `aa_a`, `aa_b`, `measured`), `index`, `arguments`, `exit_code`, `ok`, `wall_s`, `user_cpu_s`, `sys_cpu_s`, `peak_rss_bytes` (job/process-tree peak; `null` where the platform gives none), `instructions_u` (user-space instructions from `perf stat -e instructions:u` on Linux when `/usr/bin/perf` or `/bin/perf` exists, otherwise `null`; perf then sits in the timed process tree), `load_average_start` and `load_average_end` (1/5/15-minute load averages around the run; `null` on Windows), `stderr_tail` on failure. |
| `summary[]` | Per workload over measured rows: `median_wall_s`, `median_cpu_s` (user+sys), `median_peak_rss_bytes`, `measured_range_pct` ((max-min)/median), `aa_spread_pct` (absolute difference of the A and B wall medians over their mean), `median_instructions_u`, `aa_instructions_spread_pct` (the same over instructions), the workload's first `load_average_start` and last `load_average_end`, and `all_ok`. |

An A/A spread larger than the change you are charting means the host was too
noisy to see it. When other workers share the VM, wall and CPU time swing by
seconds while user-space instruction counts stay within a fraction of a
percent, so chart `instructions_u` on Linux and use load averages to flag runs.
The command exits 1 after writing the record when any run fails.

## Scaling check

Most slowdowns found so far were a check that walks the whole program per
question, which makes each machine cost more in a bigger program.
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

To find which walk is responsible, sample the largest size with samply on
Linux and read the heaviest frames under the checking workers:

```text
samply record --save-only -o scaling.json.gz -- target/release/omega --check <dir>/main.omg < /dev/null
```

## Checking the tool itself

The process fixtures are behind the `fixtures` feature, and no CI job runs
them, so run them when you change the tool:

```text
mbx test -p omega-bench --features fixtures
mbx clippy -p omega-bench --all-targets --features fixtures -- -D warnings
```

They cover known output and checksums, compiler and native transport, pairing,
warmup exclusion, overwrite refusal, input mutation, failed compilation, wrong
exit or output, timeout, overflow, and incomplete-oracle rejection. The fake
compiler fixture is labelled as such; a real compiler comparison is separate
integration evidence.
