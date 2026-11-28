# Public filesystem values on Linux x86-64

A release compiler built from `e6477bc08c36684b78cca182ba91bd012b4c5a4d` compiled the original fixture from source on the cloud Xeon. In the independent Linux execution control, both the native executable and Psi returned 0 with empty standard output and an empty working directory after execution. The native executable's SHA-256 was `377cf86f898520ee6e68d736d2b816c3d2c7480391801e8c561d7ea28c449cec`.

## Reproduction on Linux x86-64

Run from a detached checkout of the candidate with the revision's qualified mbx and Python 3.10 or later. The compiler build is outside every sample. Give each invocation an unused absolute output path on a private filesystem. Use the approved mbx CPU budget for the host.

```sh
mbx build --release -p omega
mbx reserve --cpus 3 -- python wiki/drafts/measurements/public_values_cold_baseline/measure.py target/release/omega /absolute/path/to/new-series
mbx reserve --cpus 3 -- python wiki/drafts/measurements/public_values_cold_baseline/measure.py target/release/omega /absolute/path/to/new-controls --verify-only
python wiki/drafts/measurements/public_values_cold_baseline/summarize.py /absolute/path/to/new-series
```

Each client starts from a fresh `git archive` of the measured revision's tracked `source/library` tree and the two original fixture files. This preserves the path dependency's seven-level relative locator. The harness checks both original source hashes. At the baseline, the archive contains 221 library files and two client files. It starts with an empty per-user cache, temporary directory and runtime working directory, without build outputs, a lockfile or prebuilt dependency artifacts. It sets `OMEGA_LIBRARY_ROOT` to the freshly staged library, sets `XDG_CACHE_HOME` to the new cache and clears inherited Omega overrides. Staging happens before timing and warms filesystem pages. The operating system's page cache is uncontrolled.

The timer starts immediately before launching `omega build --target linux_x86_64 <original-build.omg>` and ends after the compiler exits. It includes target selection, project open, package checks, code generation, publication and process teardown. Reading and hashing the ELF, fixture preparation, source editing and execution are outside the timer. The compiler process peak RSS, user CPU time and system CPU time come from Linux `wait4`.

After each original build, the harness runs the published ELF. It then replaces all five occurrences of `portable_source.tmp` in that client's `main.omg` with `portable_edited.tmp` and launches a fresh compiler process in the same client. Dependency sources, resolver cache and build directory persist. The two filenames have the same byte length. The edited ELF must have a different hash, exit 0, produce empty streams and leave the working directory empty. The tracked fixture stays unchanged.

Five independent pairs measure ordinary wall time. Five additional pairs collect the compiler's stock `--timings` output. Native launches and their checks occur outside every build timing. Phase rows can nest, and work on parallel threads can overlap in wall time.
