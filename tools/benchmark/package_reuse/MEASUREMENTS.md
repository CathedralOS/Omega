# Linux x86-64 measurements

The release compiler built the original public filesystem program with three fresh caches. All three executions returned 0, emitted nothing on either stream and left the working directory empty. The separate explicit-input experiment reused one checked supplier after a root-source edit.

## Host and method

The base checkout was clean at `e6477bc08c36684b78cca182ba91bd012b4c5a4d`. The benchmark runner revision was `e86e06fb2ea1e33e8325bd5317e534594322b5b6`. The same-entry experiment revision was `1fa25da7f79fb15b81143a0f2c9975291e4f7298`. The qualified build tool was mbx 1.29.2-omega.3. The base release compiler SHA-256 was `f1f2c14a870a4957b671499bc3ba0d90b5fc88f7a108ef060f59abb557f4c987`. Rebuilding on the clean base reproduced the same compiler bytes used for the integration test.

The host ran Linux 6.18.57 on an Intel Xeon Platinum 8573C, with four CPUs available and a 16 GiB memory limit. Each build started a new compiler process. No prebuilt dependency artifact, prior output, lockfile or compiler working state supplied a cold trial. The OS page cache was uncontrolled. The clock covered process launch through compiler exit. Artifact hashing and native execution occurred afterward.

The default command sandbox presented the filesystem root as UID 65534. The resolver refused it at startup. The measured commands used the same non-root account in the cloud runner's normal filesystem context, where the root belongs to UID 0 and the private cache belongs to the compiling user. The resolver and native admission code ran as shipped.

## Reproduction

Build the runner on the candidate and retain its executable. Use the same registered worktree for the clean detached baseline. Start the runner from the baseline repository root and give it a new results directory.

```sh
# Candidate checkout
mbx build --locked -p xtask
cp target/debug/xtask /tmp/omega-public-values-xtask-linux
mbx build --locked --release -p omega
mbx test --locked --release -p omega --test prechecked_dependency -- --nocapture

# Clean baseline checkout
mbx build --locked --release -p omega
/tmp/omega-public-values-xtask-linux measure-public-values "$PWD/target/release/omega" /workspace/scratch/omega-package-reuse-e647-linux
```

The runner invoked `omega build --target linux_x86_64 --build-dir OUTPUT ENTRY`. It set `OMEGA_LIBRARY_ROOT` to the checkout's bundled library and `XDG_CACHE_HOME` to the trial's private cache. The cold entry was `source-rust/application/tests/fixtures/filesystem/run/public_values/build.omg`. Its SHA-256 was `dc4bda0f879ed834eb3f9ff081d6a0b00e483fd86bebe8c3e0c3f29e5d8ca7fd`. The original main source SHA-256 was `85df47708700d685cf77bae0fed481298d00ab8808885de9ae63e2e78cab9fa8`.

## CLI results

| CLI trial | Compiler wall time (s) | Resolver files before → after | Native exit |
| --- | ---: | ---: | ---: |
| cold-1 | 214.205620 | 0 → 0 | 0 |
| cold-2 | 173.394406 | 0 → 0 | 0 |
| cold-3 | 200.254007 | 0 → 0 | 0 |
| prepare | 189.918032 | 0 → 0 | 0 |
| edited-1 | 204.352655 | 0 → 0 | 0 |
| edited-2 | 216.547034 | 0 → 0 | 0 |
| edited-3 | 226.198876 | 0 → 0 | 0 |
| profile | 219.003721 | 0 → 0 | 0 |

The three original trials had a median of 200.254007 s. The three source-edit trials had a median of 216.547034 s. All eight executables were 524,288-byte x86-64 ELF64 images. The original, preparation and profiling images all had SHA-256 `377cf86f898520ee6e68d736d2b816c3d2c7480391801e8c561d7ea28c449cec`. Each source edit produced a distinct executable. All programs passed their native cleanup oracle. The compiler issued the same two unreachable-branch warnings on the original trials.

The copied library contained 221 tracked files, including 219 Omega source files, of which 118 belonged to the system package. The preparation build left the shared resolver directory with zero files. Retaining that directory across source edits supplied no checked dependency. The explicit-input test below measures that case. [Per-trial JSON](linux-results.json)

## Where cold work went

The stock profiler reported 10.421 s in project opening, including 6.873 s resolving sources, 2.863 s handing off dependencies and 0.665 s executing the root Build. Its dependency source judgment row was 6.006 s. These nested measurements overlap. [Compiler phase output](linux-phases.txt)

A separate fresh CLI build traced `open`, `openat` and `openat2` with `strace -f -yy -T -s 400`. The following reproduces the trace from the repository root using fresh private directories. Count successful returned file descriptors whose resolved pathname ends in `.omg`.

```sh
repo="$PWD"
trace_dir="$(mktemp -d)"
mkdir -m 700 "$trace_dir/cache" "$trace_dir/cwd"
(cd "$trace_dir/cwd" && XDG_CACHE_HOME="$trace_dir/cache" OMEGA_LIBRARY_ROOT="$repo/source/library" \
  strace -f -yy -T -s 400 -e trace=open,openat,openat2 -o "$trace_dir/open.trace" \
  "$repo/target/release/omega" build --target linux_x86_64 --build-dir "$trace_dir/product" \
  "$repo/source-rust/application/tests/fixtures/filesystem/run/public_values/build.omg" </dev/null)
sed -nE 's/.*= [0-9]+<([^>]*\.omg)>[[:space:]]+<[0-9.]+>$/\1/p' "$trace_dir/open.trace" | sort | uniq -c
```

There were 454 opens for 217 distinct source paths. The bundled library accounted for 452 opens across 215 paths. Each of the 118 system source files was opened twice, and the compiler opened `core/process_exit.omg` 30 times. The traced file-opening syscalls took 0.168 s in aggregate under instrumentation. That number excludes reads, directory traversal, hashing and parsing. The traced ELF had the original hash and passed the native run oracle.

Retaining the same verified source capture within an invocation could remove duplicate opens on a cold build. A packed source distribution would still need to preserve exact package revision, source bytes, filesystem metadata and grants. The traces do not establish a speedup for packing. The independent trace of the same base measured 115.398 s across ten lowered-form checking calls and 2.265 s in one native emission. Send the repeated-check evidence to the checker owner before pursuing independent parallel native emission. [Independent Linux phase trace](https://github.com/CathedralOS/Omega/blob/3911be9e78e4efe7ed186dc1804c6999807c958c/wiki/drafts/measurements/public_values_cold_baseline.md)

## Explicit supplier experiment

Each of three fresh test processes compiled and launched four native Linux applications. The first application requested the `signals` leaf from the repository's `independent_scalar_results` fixture. It compiled the provider normally and paid for an additional export. The producer wrote 22,977 bytes of Psi, Omega's portable compiled program, and retained its locally issued manifest and exact byte digest. The same host then opened a fresh Product for an edited application. It supplied the declared artifact once and enforced the receiving Product's checks. The executable and supplier library were byte-identical to both an ordinary source build through the same embedding API and an independent release CLI build. All twelve applications returned 0 with empty output streams.

| Build and launch (seconds) | Trial 1 | Trial 2 | Trial 3 | Median |
| --- | ---: | ---: | ---: | ---: |
| First build including the additional export | 4.261151 | 3.963879 | 4.225105 | 4.225105 |
| Additional export within the first build | 0.252234 | 0.221674 | 0.450811 | 0.252234 |
| Edited root using the checked dependency | 3.628077 | 4.164714 | 3.656839 | 3.656839 |
| Edited root building the dependency through the same embedding API | 4.674391 | 4.247351 | 3.798230 | 4.247351 |
| Edited root through the ordinary CLI | 3.456679 | 3.759934 | 3.641753 | 3.641753 |

The explicit supplier saved one source publication per edited build. Every trial ran the reused path before the same-entry source control. Both timers include a fresh project open, native compilation and application execution. The reused path also measures the host's package-match and byte-acceptance work. The CLI control includes separate process startup. These three observations describe this leaf and the fixed order.

The unprivileged leaf has no accepted semantic bindings and uses an exact local revision with no registry version selection. Changed supplier source, changed local revision with identical imported declarations, a changed grant and a different provider all failed the input-equality predicate. A different target and altered saved bytes were refused by the supplier interface. The normal CLI refused a missing import and stale package acceptance and left the lockfile byte-identical after the stale check.

The test retained compiler-issued provider evidence in its embedding host. A production cross-process package interface needs an explicit carrier and receiving-host authorization for the same evidence. For the fused filesystem dependency, a prior checked output or implicit Build plan would violate the cold trial conditions. The storage specification allows identical work to share an in-memory result within the current invocation. Coordinate changes to package planning, provider carriers or the native object contract with the corresponding owners. [Storage rule](../../../wiki/spec/build/compiler_storage.md)

To reproduce the three trials, compile the test with `mbx test --locked --release --no-run -p omega --test prechecked_dependency`, then invoke the reported test binary with `--nocapture` three times. Give each process a new absolute `XDG_CACHE_HOME`, set `OMEGA_LIBRARY_ROOT` to the checkout's absolute `source/library` path, and close stdin. The JSON records the manifest identity and timings from each independent scratch project.
