# Rule 30

Evolves a 31-cell elementary cellular automaton for 16 generations from a
single center seed, with the whole row packed into one 64-bit word: each
generation is the single bit-parallel step
`cells = ((cells << 1) ^ (cells | (cells >> 1))) % 2147483648` -- bit `i`
holds cell `i`, and the modulo clips the step back into the 31-bit row
(out-of-range neighbours read 0). Renders each generation as a `'#'`/space
line, then reads Enter and exits 0. Exercises word-wide bit ops under a
defined-overflow domain, variable shift amounts, and byte-level output.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/rule-30-route samples/cli/simulation/rule_30/main.omg &&
  ./build/rule-30-route/omega-program
```

On Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/rule-30-route samples/cli/simulation/rule_30/main.omg
if ($LASTEXITCODE -ne 0) { throw 'rule_30 compilation failed' }
& ./build/rule-30-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'rule_30 returned a nonzero exit' }
```

Use Cargo when `mbx` is unavailable. Compilation must succeed before executing
an image. Package review remains part of this outer command; do not manufacture
acceptance to advance the sample.

The faster compiler-library probe on macOS is:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=rule_30 cargo nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='rule_30'`, run the
same nextest command without its inline environment assignment, then
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`.
This harness bypasses the CLI's package review and closes stdin rather than
pressing Enter. It checks exit 0 and output containing `#`; it is not evidence
that the full interactive command works.
