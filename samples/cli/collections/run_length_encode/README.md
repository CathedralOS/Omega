# Run-length encode

Scans the fixed 20-byte record `AAAABBBCCDDDDDEEEEEE` and prints one
`run: <value> x<count>` line per run boundary (4 A, 3 B, 2 C, 5 D, 6 E), then
reads Enter and exits with the number of runs: 5. Exercises adjacent-element
comparison, emit-on-boundary control, and per-run line assembly.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/run-length-encode-route samples/cli/collections/run_length_encode/main.omg &&
  ./build/run-length-encode-route/omega-program
```

On Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/run-length-encode-route samples/cli/collections/run_length_encode/main.omg
if ($LASTEXITCODE -ne 0) { throw 'run_length_encode compilation failed' }
& ./build/run-length-encode-route/omega-program.exe
if ($LASTEXITCODE -ne 5) { throw 'run_length_encode returned a wrong exit' }
```

Use Cargo when `mbx` is unavailable. Compilation must succeed before executing
an image. Package review remains part of this outer command; do not manufacture
acceptance to advance the sample.

The faster compiler-library probe on macOS is:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=run_length_encode cargo nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='run_length_encode'`, run the
same nextest command without its inline environment assignment, then
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`.
This harness bypasses the CLI's package review and closes stdin rather than
pressing Enter. It checks exit 5 and output containing `run: C x2`; it is not
evidence that the full interactive command works.
