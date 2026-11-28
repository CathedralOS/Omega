# Public filesystem values cold-build baseline

## Reproduction

Use Python 3.9 or later and the revision's qualified mbx. Run the commands from a detached checkout of the candidate. The compiler build is outside every sample. Start with a new absolute output directory on a private filesystem. For Windows qualification, use a short local path that the current Windows account owns. Record the physical CPU, host operating system, mbx budget, current load and power settings with the run. On Windows the script records the CPU returned by CIM and the compiler process's peak working set.

```sh
mbx build --release -p omega
mbx reserve --cpus 3 -- python wiki/drafts/measurements/public_values_cold_baseline/measure.py target/release/omega /absolute/path/to/new-series
mbx reserve --cpus 3 -- python wiki/drafts/measurements/public_values_cold_baseline/measure.py target/release/omega /absolute/path/to/new-controls --verify-only
python wiki/drafts/measurements/public_values_cold_baseline/summarize.py /absolute/path/to/new-series
```

The example uses the cloud reservation. For a coordinated Windows run use the workstation's approved mbx budget and `target/release/omega.exe`. Check each native exit code and its output. The gate is a release-built Omega compiling the original Build to a working Windows x86-64 executable in less than 1.000 s on the physical 7950X with empty compiler and dependency caches.

Each client is a fresh `git archive` of the measured revision's 221 tracked `source/library` files and the two original fixture files. This preserves the path dependency's seven-level relative locator. The harness checks both original file hashes. It starts with an empty per-user cache, temporary directory and runtime working directory, with no build output, lockfile or prebuilt dependency. It sets `OMEGA_LIBRARY_ROOT` to the newly staged library and clears inherited Omega overrides. On Linux the resolver uses `XDG_CACHE_HOME`. On Windows it uses `LOCALAPPDATA`. Staging happens before timing and warms filesystem pages. The operating system's page cache is uncontrolled.

The wall timer starts immediately before launching the release CLI and ends after it exits. It includes target selection, project open, package checks, code generation, publication and process teardown. PE reading, hashing, admission infrastructure, copying, editing and execution are outside the timer. The Linux peak is the compiler process high-water RSS from `wait4`. The Windows harness reads the compiler process peak working set. Neither metric sums concurrent child processes.

Every edited trial retains that same client's dependency state, library, output and cache. After its original build, the harness replaces all five occurrences of `portable_source.tmp` in the client copy of `main.omg` with `portable_edited.tmp`, then launches another compiler process. It requires a different executable hash. The two source strings have the same byte length. The fixture in Git stays unchanged. Five additional pairs use the stock `--timings` option in independent clients. The reported throughput uses only the uninstrumented pairs. Phase rows can nest and worker CPU measurements can overlap on the wall clock.
