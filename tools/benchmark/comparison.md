# Rust compiler and runtime comparisons

`omega-bench` answers one question per run: how does this fixed workload behave
under these two executables? The plan fixes the commands, input closure,
measurement boundary, and correctness oracle before either producer runs.

Build once, then invoke the retained executable directly. Keep builds and
competing measurements outside the timed experiment.

```text
python tools/build_slot.py -- mbx build -p omega-bench
target/debug/omega-bench compare plan.json build/performance/my-comparison
```

Windows executables have the `.exe` suffix. The output directory must be new:
remeasurement never overwrites a previous run. A run can use the same executable
in both slots for an A/A instrument check. This validates the capture path; a
single A/A comparison does not calibrate every workload or establish noise limits.

## Compiler comparison

The checked-in [length plan](plans/compiler-length.windows.json) uses the
`length/local` fact and two retained Windows compiler paths. Put identified
producers at those paths, or edit the plan to name your own. On another host,
change the executable suffix and target. Paths in the plan resolve as follows:

- `working_directory` is relative to the plan file.
- Executables, `inputs`, and compiler `source` are relative to that directory.
- The result directory is relative to the invoking shell's directory.

`measurement.library_root` selects the same explicit toolchain library for both
compilers through `OMEGA_LIBRARY_ROOT`. That directory is automatically included
in the hashed input closure. A retained compiler's embedded development-library
path cannot silently select a different checkout's library.

Compiler mode checks each binary's `--build-profile` and records its actual
`--version` and SHA-256. It requires an optimized producer and the measuring
host's native target. It neither builds compilers nor accepts package decisions.
Prepare any required review/lock state beforehand through the existing workflow.
Include those lock inputs and every consumed library/dependency path in `inputs`.
The working checkout's revision is not treated as the producer's revision.

Each sample compiles into its own empty output directory with `--timings`,
requires successful publication inside that directory, hashes the native image,
and executes it against the exact authored exit/stdout/stderr oracle. Native
verification has its own observation and does not enter the compile timer.
`require_identical_artifacts: true` additionally requires all warmup and measured
images to have identical bytes. Set it to false for intentional code-generation
changes; the native result oracle remains mandatory.

Extra command arguments and environment overlays are retained in the plan.
For example, optimizer-disable arguments can be supplied independently to the
two compilers. Compiler command arguments support `--offline` and repeatable
`--disable-optimization <ExactName>`; other options are rejected so an extra
source path or subcommand cannot silently replace the measured product.
Runtime commands accept arbitrary structured argument vectors.
Full compiler logs retain any optimizer selection receipt.
The existing Python workflow still owns validation of exact optimizer-selection
records for promotion; the Rust runner does not infer enabled rules from source.

Fresh output directories do **not** mean cold operating-system caches. Declared
inputs and executables are hashed before and after each sample, outside timing,
which itself warms the filesystem cache. These are repeated fresh-output builds.

## Runtime process latency

Use `measurement.kind: "process"` for a native program's complete launch-to-exit
latency. Both command specifications take an executable, an argument vector, and
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

The exact byte oracle must consume the computed result. A clean exit from a
program that discards its accumulator cannot establish arithmetic throughput.
Arguments/stdin should supply representative runtime inputs; confirm the emitted
code still performs the intended work. The harness cannot establish that from a
checksum alone. Empty stdin selects EOF, and nonempty stdin is transferred as
authored bytes. Line endings in output comparisons are exact.

The process timer includes startup, output capture, waiting, and process-tree
cleanup. This is command latency, **not isolated kernel execution time**. For
short kernels, batch useful work within the child and check its final result,
or use the explicit timed-region protocol below.

## Runtime timed regions

`measurement.kind: "reported_work"` accepts one JSON object on stdout, empty
stderr, and exit 0. The child reports its own timed region:

```json
{"schema":"omega-timed-work/1","elapsed_ns":5000000,"work_units":1000000,"checksum":"1234"}
```

The plan fixes the expected work count and checksum:

```json
{"kind":"reported_work","stdin":"","work_units":1000000,"checksum":"1234"}
```

The primary metric is `reported_work_ns_per_unit`. The enclosing process wall
time is retained separately. Zero time, zero work, unexpected work count,
incorrect checksum, unknown schema/fields, or a region longer than the enclosing
process observation refuses the sample. Setup and verification should sit outside
the child's timer; the checksum must depend on the timed computation.
Child-reported time relies on that instrumentation being correct.

The opt-in `benchmark-probe` fixture demonstrates this protocol with a
runtime-sized wrapping-square sum, runtime seed, consumed accumulator, and Rust
`black_box`. It is an instrument test, not an Omega generated-code benchmark:

```text
python tools/build_slot.py -- mbx build -p omega-bench --features fixtures
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
deviations, paired absolute/percentage changes, and a deterministic 4,000-resample
paired percentile bootstrap interval for the median difference. Resampling keeps
the pair intact. Fewer than six pairs are explicitly labelled insufficient;
six pairs are not a promise of adequate statistical power. An interval crossing
zero is inconclusive. Even a separated interval describes this run, not every
host or workload. Autocorrelation and drift can violate its assumptions.

A timeout, incorrect output, compiler error, changed declared input/executable,
or output overflow fails the run and leaves its journal. It does not produce a
successful report. A crash or external interruption leaves a journal without a
`complete` event; consumers must require that event and its report digest before
treating the report as completed evidence. No slow valid samples are discarded.

Input paths may be individual files or recursively hashed directories. The
result directory must be outside declared input directories. The plan, executable
bytes, and declared file set are checked around each sample. These checks detect
persistent changes, not transient edits restored between observations. An
isolated worktree and immutable retained producers remain the working practice.
Only declared inputs are covered; this is not automatic dependency discovery.

Environment overlays are recorded. Other environment values are inherited and
are not dumped, to avoid recording credentials. Host OS, architecture, available
parallelism, and the Windows processor identifier are recorded. The runner does
not isolate hardware contention, pin CPU frequency/affinity, or infer cache state.
Record relevant host conditions alongside the result. Memory is explicitly
unavailable until the shared bounded runner exposes a correctly defined metric;
hardware counters and allocation profiling are not yet integrated.

Processes use the existing `bounded-process` owner: a wall timeout, 1 MiB per
captured stream, process-tree cleanup, 8 GiB process/aggregate memory ceilings,
16 GiB address-space ceiling where supported, 1 GiB file-size ceiling, 1,024
open-file ceiling, and 64-process ceiling. Platform enforcement follows that
library's contracts. These limits and capture overhead are part of the recorded
measurement environment.

## Focused checks

```text
python tools/build_slot.py -- mbx nextest run -p omega-bench --features fixtures --test-threads 1
python tools/build_slot.py -- mbx clippy -p omega-bench --all-targets --features fixtures --no-deps -- -D warnings
cargo fmt -p omega-bench -- --check
```

The process fixtures test known output/checksum, compiler/native transport,
pairing, warmup exclusion, overwrite refusal, input mutation, failed compilation,
wrong exit/output, timeout, overflow, and incomplete-oracle rejection. The fake
compiler fixture is labelled as such; a real compiler comparison is separate
integration evidence.
