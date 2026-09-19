# Generic counters

Two instances of `Counter<T>` retain independent fields and method selections:
the integer counter records twice, and the Boolean counter records once. The
program exits **16** without printing. Its Console field must be an established
`Service<Console>` supplied by the selected entry. Service validity is intrinsic;
no separate `Bound` qualification is required.

Run from the repository root on macOS ARM64:

```sh
mbx run -p omega -- --target macos_arm64 --build-dir build/generic-counters samples/cli/basics/generic_counters/main.omg &&
  ./build/generic-counters/omega-program
```

On Windows PowerShell:

```powershell
mbx run -p omega -- --target windows_x86_64 --build-dir build/generic-counters samples/cli/basics/generic_counters/main.omg
if ($LASTEXITCODE -ne 0) { throw 'Compilation failed; do not run an old executable' }
& ./build/generic-counters/omega-program.exe
if ($LASTEXITCODE -ne 16) { throw 'Expected exit 16' }
```

Ordinary package review remains required; these commands do not grant acceptance.
The compiler-library regression supplies explicit test-owned acceptance and
checks publication and native exit independently of the CLI's project review:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=generic_counters mbx nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell, set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='generic_counters'` before
the same test command and remove it afterward with
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`. Use Cargo if `mbx` is unavailable.
