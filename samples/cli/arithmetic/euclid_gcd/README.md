# Euclid's greatest common divisor

The original state-machine loop computes `gcd(48, 36)`, prints its result
message, waits for input (or EOF), and exits with code **12**. The hosted entry
uses a provisioned `Service<Console>` with intrinsic binding validity.

The complete expected stdout is:

```text
euclid_gcd: Euclid's algorithm on (48, 36) via the remainder loop
result: the GCD, 12 for (48, 36) (the exit code)
[press Enter to close]
```

Ordinary project review starts from the repository root in the checkout being
used for compilation:

```text
mbx run --release -p omega -- update --project samples/cli/arithmetic/euclid_gcd --target macos_arm64
```

Inspect the reported capability review, decide each required row, then run:

```text
mbx run --release -p omega -- update --resume --project samples/cli/arithmetic/euclid_gcd
mbx run --release -p omega -- --target macos_arm64 --build-dir build/euclid-gcd-route samples/cli/arithmetic/euclid_gcd/main.omg
```

Use the corresponding authored Windows or Linux target on those hosts. Use
Cargo when `mbx` is unavailable. The macOS compiler checks use
`export RUST_MIN_STACK=67108864`; PowerShell uses
`$env:RUST_MIN_STACK='67108864'`. After successful compilation, execute the
reported artifact, verify all three lines with trailing newlines and empty
stderr, and expect exit 12 for both EOF and Enter. Never run an old artifact
after compilation fails. Local-source locks bind the checkout; review again
in a different checkout rather than copying its lock.

Native completion remains open in [SAMPLE-CORPUS](../../../../TASKS.md).
The checked entry retains the complete body and exact service call frames;
the next observed native failure is `Terminal proposal must retain every
integer comparison occurrence exactly once`. `CRASH-CONTRACT` owns the
selected-comparison association and Terminal product validation boundary.

Run the native sample regression from the repository root:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=euclid_gcd cargo nextest run -p compiler \
  --test samples_compile --no-fail-fast \
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell, set `$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'euclid_gcd'`, then run
the same Cargo command on one line; remove the filter afterward with
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`. Use `mbx` instead of `cargo`
when available. The harness supplies EOF and checks the documented exit and
output substring on the current host; it does not establish runtime behavior
on other hosts or replace ordinary CLI package review/acceptance.
