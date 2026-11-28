# Public filesystem values on Linux x86-64

A release compiler built from `e6477bc08c36684b78cca182ba91bd012b4c5a4d` compiled the original fixture from source on the cloud Xeon. In the independent Linux execution control, both the native executable and Psi returned 0 with empty standard output and an empty working directory after execution. The native executable's SHA-256 was `377cf86f898520ee6e68d736d2b816c3d2c7480391801e8c561d7ea28c449cec`.

## Reproduction on Linux x86-64

Run from a detached checkout of the candidate with the revision's qualified mbx and Python 3.10 or later. The compiler build is outside every sample. Give each invocation an unused absolute output path on a private filesystem. Use the approved mbx CPU budget for the host.

```sh
evidence_rev=3911be9e78e4efe7ed186dc1804c6999807c958c
recipe=wiki/drafts/measurements/public_values_cold_baseline
bench=$(mktemp -d)
git fetch origin "$evidence_rev"
git show "$evidence_rev:$recipe/measure.py" > "$bench/measure.py"
git show "$evidence_rev:$recipe/summarize.py" > "$bench/summarize.py"
mbx build --release -p omega
mbx reserve --cpus 3 -- python "$bench/measure.py" target/release/omega /absolute/path/to/new-series
mbx reserve --cpus 3 -- python "$bench/measure.py" target/release/omega /absolute/path/to/new-controls --verify-only
python "$bench/summarize.py" /absolute/path/to/new-series
```

Each client starts from a fresh `git archive` of the measured revision's tracked `source/library` tree and the two original fixture files. This preserves the path dependency's seven-level relative locator. The harness checks both original source hashes. At the baseline, the archive contains 221 library files and two client files. It starts with an empty per-user cache, temporary directory and runtime working directory, without build outputs, a lockfile or prebuilt dependency artifacts. It sets `OMEGA_LIBRARY_ROOT` to the freshly staged library, sets `XDG_CACHE_HOME` to the new cache and clears inherited Omega overrides. Staging happens before timing and warms filesystem pages. The operating system's page cache is uncontrolled.

The timer starts immediately before launching `omega build --target linux_x86_64 <original-build.omg>` and ends after the compiler exits. It includes target selection, project open, package checks, code generation, publication and process teardown. Reading and hashing the ELF, fixture preparation, source editing and execution are outside the timer. The compiler process peak RSS, user CPU time and system CPU time come from Linux `wait4`.

After each original build, the harness runs the published ELF. It then replaces all five occurrences of `portable_source.tmp` in that client's `main.omg` with `portable_edited.tmp` and launches a fresh compiler process in the same client. Dependency sources, resolver cache and build directory persist. The two filenames have the same byte length. The edited ELF must have a different hash, exit 0, produce empty streams and leave the working directory empty. The tracked fixture stays unchanged.

Five independent pairs measure ordinary wall time. Five additional pairs collect the compiler's stock `--timings` output. Native launches and their checks occur outside every build timing. Phase rows can nest, and work on parallel threads can overlap in wall time.

## Cold phase trace

A separate Linux run with GDB measured 158.131 s inside the compiler, with its stock `--timings` enabled. The same optimized executable ran without recompilation. It emitted the Linux ELF and exited normally. A function's duration below is the union of its call intervals across threads, so recursive or concurrent calls to that function count once at each instant. Functions can run inside other rows.

| Measured scope | Calls | Wall seconds |
|---|---:|---:|
| Product preparation, including its frontend and checkers | 2 | 149.407 |
| Psi 06 judgment | 10 | 115.398 |
| Psi 05 `lower_program_with_parallel` | 299 | 7.101 |
| CLI project open and Build plan | 1 | 6.317 |
| Frontend `prepare_with_body_scope` | 4 | 3.631 |
| Native emission | 1 | 2.265 |
| Layout schema `record_argument` | 8 | 0.037 |

The final checker call lasted 58.843 s. Two earlier checker calls each exceeded 22 s. In project open, source resolution took 3.940 s, dependency handoff took 1.943 s and root Build execution took 0.428 s. The frontend and layout rows cover only the named functions. The GDB script records their precise symbols. GDB stops contribute overhead, so use the separately collected ordinary CLI measurements for throughput.

The [trace script](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline/trace_phases.py) records [entry-to-return intervals](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline/span-events.tsv). Run the [interval summary](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline/span_summary.py) on that TSV to reproduce the call counts and unions. For a new trace, use a new original client with the same environment and mbx reservation as the wall runs. Generate a symbol map with `nm -C target/release/omega`, set `BENCH_NM` to its absolute path and `BENCH_SPANS` to the desired output TSV. In the client's empty scratch directory run the following through mbx with absolute paths.

```sh
gdb -nx -batch -x /absolute/path/to/trace_phases.py --args /absolute/path/to/omega build --timings --target linux_x86_64 /absolute/path/to/public_values/build.omg
```

## CPU sampling

In a separate fresh client, GNU gprofng recorded 346.402 sampled CPU-seconds across threads. The compiler reported 118 selected source files and published a native artifact for `linux_x86_64`. The sample attributed 26.449 inclusive CPU-seconds to `Prover::prove`, 19.213 to `Replay::resolve_storage_branch`, 19.143 to `proof_admission::kernel::decide_primitive`, 17.883 to one `Replay::transfer` specialization and 17.772 to `PropositionContext::validate`. These function samples can nest. Allocation and deallocation also appeared among the top exclusive costs. The GDB phase trace and the CPU sample were separate invocations.

The [inclusive](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline/cpu-inclusive.txt) and [exclusive](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline/cpu-exclusive.txt) tables preserve the first 30 functions. To reproduce, run the harness with `--profile-only`, then run `gprofng display text -sort i.totalcpu -functions <sample.er>` and `gprofng display text -functions <sample.er>`.

## Independent schema candidate

PR #6729 measured one probe-free cold native Linux pair on a cloud Xeon Platinum 8573C with a four-CPU cgroup quota and uncontrolled OS pages. Shared base `e6477bc08c36684b78cca182ba91bd012b4c5a4d` took 83.059 s and candidate `d5ac45bd29303ecc6efdf78abf357597d7cf98f6` took 65.909 s, a 20.6% reduction within that pair. Both began with empty compiler, dependency and output caches and no prebuilt dependency. Both compiled 118 sources and emitted the same ELF as this series, which exited 0. The published recipe invokes the file directly with an explicit output directory. This series invokes the staged `omega build` subcommand. The two series keep their own baselines. [Pinned native Linux qualification](https://github.com/CathedralOS/Omega/blob/1b3a5e315b392e7d9ac1acb191e57fe12f0ede79/wiki/drafts/measurements/policy_schema_inputs_2026_11_28.md#native-linux-x86-64-qualification)

All seven full policy results matched between revisions at the same source path. Each of the four layout results and three access results was distinct in the cold compilation. On the clean candidate, a separate 99 Hz CPU profile attributed 4,040 of 14,313 samples (28.2%) to integer-bound proof search directly under control-flow join inference. This identifies a checker path to remeasure after integration. Those samples overlap the broader checker samples and measure CPU activity. [Candidate measurements and preservation checks](https://github.com/CathedralOS/Omega/pull/6729)
