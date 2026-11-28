# Public filesystem values on Linux x86-64

The fastest of five empty-cache builds took 89.616 s. The median was 90.758 s. The median edited build took 91.894 s. None met the 1.000 s target.

A release compiler built from `e6477bc08c36684b78cca182ba91bd012b4c5a4d` compiled the original fixture from source on the cloud Xeon. In the independent Linux execution control, both the native executable and Psi returned 0 with empty standard output and an empty working directory after execution. The native executable's SHA-256 was `377cf86f898520ee6e68d736d2b816c3d2c7480391801e8c561d7ea28c449cec`.

## Five independent Linux clients per condition

| Uninstrumented metric | Condition | Min | Median | Mean | Max |
|---|---|---:|---:|---:|---:|
| Elapsed (s) | Cold | 89.616 | 90.758 | 92.818 | 100.687 |
| Elapsed (s) | Edited | 89.539 | 91.894 | 91.374 | 92.920 |
| Peak RSS (MiB) | Cold | 1,212.363 | 1,222.352 | 1,223.569 | 1,238.250 |
| Peak RSS (MiB) | Edited | 1,226.770 | 1,228.082 | 1,231.623 | 1,242.258 |
| User CPU (s) | Cold | 185.335 | 191.483 | 194.110 | 213.601 |
| User CPU (s) | Edited | 184.725 | 192.252 | 190.606 | 192.997 |
| System CPU (s) | Cold | 6.625 | 7.061 | 7.434 | 8.578 |
| System CPU (s) | Edited | 5.865 | 6.777 | 6.787 | 8.016 |

Each row below is a new client. Instrumented pairs have their own original and edited builds. Every build produced a 524,288-byte ELF, every native launch exited 0, both streams were empty and all scratch directories were empty afterward.

| Client | Cold (s) | Peak MiB | Edited (s) | Peak MiB |
|---|---:|---:|---:|---:|
| wall-1 | 89.658 | 1238.25 | 92.602 | 1228.08 |
| wall-2 | 89.616 | 1212.36 | 89.539 | 1226.77 |
| wall-3 | 90.758 | 1218.60 | 92.920 | 1227.09 |
| wall-4 | 93.371 | 1222.35 | 89.916 | 1233.91 |
| wall-5 | 100.687 | 1226.28 | 91.894 | 1242.26 |
| instrumented-1 | 158.007 | 1229.79 | 134.934 | 1214.56 |
| instrumented-2 | 93.274 | 1222.48 | 89.226 | 1203.19 |
| instrumented-3 | 89.620 | 1227.96 | 89.721 | 1200.74 |
| instrumented-4 | 92.569 | 1230.50 | 99.440 | 1208.66 |
| instrumented-5 | 92.052 | 1219.21 | 96.043 | 1216.34 |

## Stock phase timings

Times are medians over five separate instrumented invocations per condition, in seconds. Open subphases and dependency Build rows nest. The interval after target preparation includes checking, lowering, native compilation, publication and diagnostic output.

| CLI interval | Original cold | Edited |
|---|---:|---:|
| Open / prepare: ProjectInput -> LocalPreparation | 0.0026 | 0.0021 |
| Open / acquire root: LocalPreparation -> CapturedRoot | 0.0007 | 0.0005 |
| Open / execute Build: CapturedRoot -> BuildStage | 0.2997 | 0.2962 |
| Open / resolve sources: BuildStage -> ProjectSources | 3.1848 | 3.1342 |
| Open / dependency handoff: ProjectSources -> PlannedProducts | 1.4056 | 1.4238 |
| Open / dispose session: PlannedProducts -> ProjectProducts | 0.0003 | 0.0004 |
| Build / Source acquisition: SourcePaths -> SourceFiles | 0.0008 | 0.0010 |
| Build / Source ingress: SourceFiles -> SyntaxTrees | 0.0794 | 0.0707 |
| Build / build-source-judgment: TypedTrees -> AcceptedTerminalSource | 0.0794 | 0.0852 |
| Dependency Builds / Source acquisition: SourcePaths -> SourceFiles | 0.0011 | 0.0010 |
| Dependency Builds / Source ingress: SourceFiles -> SyntaxTrees | 0.0669 | 0.0603 |
| Dependency Builds / build-source-judgment: TypedTrees -> AcceptedTerminalSource | 2.7368 | 2.5988 |
| timeline / open_products | 4.9365 | 4.8875 |
| timeline / prepare_targets | 0.0067 | 0.0069 |
| timeline / teardown | 0.0379 | 0.0388 |
| end of prepare_targets to first teardown | 87.5459 | 91.0703 |
| total elapsed | 92.5153 | 96.0050 |

## Host and artifact identity

The compiler checkout was pinned at `e6477bc08c36684b78cca182ba91bd012b4c5a4d`, fetched main on acquisition. The cloud baseline launcher built the compiler once with `MBX_SCHEDULER_CPUS=3 mbx build --release -p omega`. Compiler construction took 21m 16s outside the measurements. The toolchain used mbx `1.29.2-omega.3` and rustc `1.100.0-nightly (a69a63265 2026-09-03)`. The CLI reported `0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`, so the retained checkout and compiler hash identify the measured binary.

- Host: Debian 13 on Linux 6.18.57, KVM, Intel Xeon Platinum 8573C. The process ran at nice 19 on logical CPUs 2–4, with a four-CPU cgroup quota and a 16 GiB memory limit. Scheduling and OS page-cache state could vary across samples.
- Release compiler SHA-256: `513e72140bbcfb6db5ba1a83af1a5b9e6aa904255c0a6d5de274cb5982c78935`.
- Original `build.omg` and `main.omg` SHA-256: `dc4bda0f879ed834eb3f9ff081d6a0b00e483fd86bebe8c3e0c3f29e5d8ca7fd` and `85df47708700d685cf77bae0fed481298d00ab8808885de9ae63e2e78cab9fa8`.
- Edited `main.omg` SHA-256: `7a21175cea9238543db25c4acecaf118065f17f59192152ede047089f029c43b`. Edited ELF SHA-256: `61f58b680afa3dbbd044d3f37856a7992afdeae6ee857e4e02a83385babcc4de`.
- Each primed per-user cache contained seven directories and zero regular files. The edited trial retained the client's existing project state, output directory and dependencies.
- Full logs, per-process commands, raw CPU samples summarized by function, interval records and reproduction scripts are retained in the cloud workspace at `/workspace/omega-measurement/public-values-linux-evidence.tar.gz`, SHA-256 `2c07513e031a3ba06d517ffdd072a71758d4f50456428ba6731ee2da548df7f2`. The initial projected filesystem reported the root as unrelated UID 65534 and the compiler refused resolver custody. All reported builds used the ordinary cloud host namespace, where the unmodified custody check succeeded.

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

## Combined candidate

The temporary integration candidate is [bd7e9ed177d1e8ea61b980f48644c31c619f6832](https://github.com/CathedralOS/Omega/commit/bd7e9ed177d1e8ea61b980f48644c31c619f6832). Its parents are schema head `bcf570afbce15767e19319cf2f07a0a8ac390c2b` and fixed-array head `880b67dce36f0834dcce799b02ab2168c6c878be`. Both descend from the shared base. The array head reports acceptance of both disjoint-write controls and refusal of both captured-write controls. The compiler source changes combined cleanly. The whole library tree and fixture match the base.

The comparison runs the staged Linux Build entry point from the baseline recipe. Five fresh baseline/combined pairs alternate which compiler goes first. Both cold builds in a pair precede both filename-edited builds. A sixth pair collects stock phase timings and separate cold clients provide GDB spans. The temporary candidate carries only these two source changes. #6743 recorded zero prebuilt dependency hits in this CLI workload. Its three-trial 3.657 s versus 4.247 s edited medians belong to a separate embedding experiment with explicit checked supplier inputs. [Dependency experiment](https://github.com/CathedralOS/Omega/pull/6743)
