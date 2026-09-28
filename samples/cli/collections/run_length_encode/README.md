# Run-length encode

Scans the fixed 20-byte record `AAAABBBCCDDDDDEEEEEE` and prints one
`run: <value> x<count>` line per run boundary (4 A, 3 B, 2 C, 5 D, 6 E), then
reads Enter and exits with the number of runs: 5. Exercises adjacent-element
comparison, emit-on-boundary control, and per-run line assembly.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- run --target macos_arm64 samples/cli/collections/run_length_encode/main.omg
```

On Windows PowerShell:

```powershell
mbx run -p omega -- run --target windows_x86_64 samples/cli/collections/run_length_encode/main.omg
if ($LASTEXITCODE -ne 5) { throw 'run_length_encode returned a wrong exit' }
```

Use Cargo when `mbx` is unavailable. Compilation must succeed before executing
an image. Package review remains part of this outer command; do not manufacture
acceptance to advance the sample.

The maintained native sample gate completes local package review, closes stdin
at EOF, and checks exit 5 plus the documented `run: C x2` output. On macOS:

```sh
python3 tools/sample_native_gate.py --cli target/debug/omega --target macos_arm64 --filter collections/run_length_encode
```

In PowerShell use `python` instead of `python3`, `target/debug/omega.exe`, and
`--target windows_x86_64`. Build that CLI from the current checkout first. The
gate's local review acceptance is for measurement, not a dependency audit, and
EOF execution does not establish interactive Enter behavior.

The complete run prints these five lines in order, between its banner and
closing prompt:

```text
run: A x4
run: B x3
run: C x2
run: D x5
run: E x6
```
