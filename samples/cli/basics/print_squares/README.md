# Print squares

Print the squares 1 through 9 as three-digit decimal text, ending in `081`,
then read Enter and exit 0. The program retains its nested state cycles,
runtime-indexed UTF-8 buffer writes, and ordinary Console calls.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/print-squares-route samples/cli/basics/print_squares/main.omg &&
  ./build/print-squares-route/omega-program
```

On Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/print-squares-route samples/cli/basics/print_squares/main.omg
if ($LASTEXITCODE -ne 0) { throw 'print_squares compilation failed' }
& ./build/print-squares-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'print_squares returned a nonzero exit' }
```

Use Cargo when `mbx` is unavailable. Compilation must succeed before executing
an image. Package review remains part of this outer command; do not manufacture
acceptance to advance the sample. The current first failure and remaining
producer/native dependencies belong in [SAMPLE-CORPUS](../../../../TASKS.md).

The faster compiler-library probe on macOS is:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=print_squares cargo nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='print_squares'`, run the
same nextest command without its inline environment assignment, then
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`.
This harness bypasses the CLI's package review and closes stdin rather than
pressing Enter. It checks exit 0 and output containing `081`; it is not evidence
that the full interactive command works.
