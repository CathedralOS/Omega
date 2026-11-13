# Benchmark harness

Use the Rust [`omega-bench` tooling](comparison.md) for paired compiler and
runtime experiments, suite runs, regression checks, compatible history
comparisons, and allocation, WPR, or Linux `perf` profiles. Build it with
`cargo build -p omega-bench`.

## Scaling check

Most slowdowns found so far were a check that walks the whole program per
question, which makes each machine cost more in a bigger program.
`omega-bench scaling` checks the one-line program plus N uncalled machines for
each N in `--sizes` (default `0,1000,2000`) and fails when the cost per added
machine in the last interval exceeds the first by more than `--max-ratio`
(default 1.3). A linear compiler stays near 1.

```text
target/debug/omega-bench scaling --omega target/release/omega
target/debug/omega-bench scaling --omega target/release/omega --shape citations --sizes 0,100,200,400
```

`--shape machines` (the default) adds uncalled machines and catches checks
that scan the whole program per machine. `--shape citations` puts N calls of
one local lemma in a single state and catches checks that replay a state from
its first statement per call. On main `06af8fee3fe` (omega-linux, release):

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

## Progress records

`omega-bench progress` records how fast one commit's release compiler is on
this host, and optionally how much of the corpus it completes, so a dashboard
can chart both over `main`'s history. Run it from the checkout root:

```text
cargo build -p omega-bench
target/debug/omega-bench progress --results <dir>               # speed set
target/debug/omega-bench progress --results <dir> --completion  # + whole corpus
```

It builds `omega` with `cargo build --release -p omega` (`--omega <path>`
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
| `one_line_check` | `omega --check` on [`progress/one_line`](progress/one_line/main.omg) |
| `addr_algebra_check` | `omega --check` on `tests/fixtures/pass/arithmetic/addr_algebra` |
| `medium_sample_check` | `omega --check` on `samples/cli/arithmetic/prime_counter` |
| `heavy_proof_check` | `omega --check` on `tests/fixtures/pass/proofs/runtime_inductive_gauss_sum_exit` |
| `native_run_both` | `omega run --both` on `samples/cli/arithmetic/wrapping_square_sum` |

Changing a workload means bumping `BENCHMARK_SET` in `src/progress.rs`;
records of different sets are not comparable. `--completion` runs
`tools/corpus_gate.py --sweep --json`, which owns worker sharding and memory
caps, and records its counts. A full corpus takes hours.

One JSON file per run, `<commit12>__<target>__<utc>.json`, schema
`omega-progress-record/1`, followed by a one-line summary on stdout:

| Field | Meaning |
| --- | --- |
| `key` | `commit`, `dirty`, `source_trees` (`omega-rust`, `source` tree hashes, the compiler's real identity when the commit only touches tools), `host` (hostname), `target`, `build_profile` (`release`), `compiler_version` (`omega --version`), `recorded_utc`. |
| `host_detail`, `compiler`, `settings` | OS/arch/logical CPUs; binary path, SHA-256 and release-build wall time; warmup, A/A and run counts plus `address_space_bytes`, the Linux address-space cap every measured process ran under (64 GiB unless `omega-bench --address-space-gib N progress` changed it). |
| `rows[]` | One per process: `workload`, `phase` (`warmup`, `aa_a`, `aa_b`, `measured`), `index`, `arguments`, `exit_code`, `ok`, `wall_s`, `user_cpu_s`, `sys_cpu_s`, `peak_rss_bytes` (job/process-tree peak; `null` where the platform gives none), `instructions_u` (user-space instructions from `perf stat -e instructions:u` on Linux when `/usr/bin/perf` or `/bin/perf` exists, otherwise `null`; perf then sits in the timed process tree), `load_average_start` and `load_average_end` (1/5/15-minute load averages around the run; `null` on Windows), `stderr_tail` on failure. |
| `summary[]` | Per workload over measured rows: `median_wall_s`, `median_cpu_s` (user+sys), `median_peak_rss_bytes`, `measured_range_pct` ((max-min)/median), `aa_spread_pct` (absolute difference of the A and B wall medians over their mean), `median_instructions_u`, `aa_instructions_spread_pct` (the same over instructions), the workload's first `load_average_start` and last `load_average_end`, and `all_ok`. |
| `completion` | `null`, or the gate's `exit_code`, `wall_s`, `verdict`, `fixtures`, `by_status`, `by_verdict`, `categories` (status counts per `tier/category`), and `release_gates`: `RC-SOURCE-SEMANTICS` and `RC-DIAGNOSTICS` derived from off-target and wrong-reason fixtures; the native and sample gates stay `not_recorded`. |

An A/A spread larger than the change you are charting means the host was too
noisy to see it. When other workers share the VM, wall and CPU time swing by
seconds while user-space instruction counts stay within a fraction of a
percent, so chart `instructions_u` on Linux and use load averages to flag runs.
The command exits 1 after writing the record when any run fails.
