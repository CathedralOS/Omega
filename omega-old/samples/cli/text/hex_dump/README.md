# Hex dump

Renders a fixed 16-byte record as one canonical dump line -- offset, sixteen
lowercase hex pairs, and an ASCII gutter that maps bytes outside `32..126` to
`.` -- then reads Enter and exits 0. Exercises byte-table formatting and two
computed column writers into one UTF-8 carrier.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/hex-dump-route samples/cli/text/hex_dump/main.omg &&
  ./build/hex-dump-route/omega-program
```

On Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/hex-dump-route samples/cli/text/hex_dump/main.omg
if ($LASTEXITCODE -ne 0) { throw 'hex_dump compilation failed' }
& ./build/hex-dump-route/omega-program.exe
if ($LASTEXITCODE -ne 0) { throw 'hex_dump returned a nonzero exit' }
```

Use Cargo when `mbx` is unavailable. Compilation must succeed before executing
an image. Package review remains part of this outer command; do not manufacture
acceptance to advance the sample.

The faster compiler-library probe on macOS is:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=hex_dump cargo nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='hex_dump'`, run the
same nextest command without its inline environment assignment, then
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`.
This harness bypasses the CLI's package review and closes stdin rather than
pressing Enter. It checks exit 0 and output containing `4f 6d 65 00`; it is
not evidence that the full interactive command works.
