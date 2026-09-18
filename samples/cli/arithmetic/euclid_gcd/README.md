# Euclid's greatest common divisor

The original state-machine loop computes `gcd(48, 36)`, prints its result
message, waits for input (or EOF), and exits with code **12**. The hosted entry
uses a provisioned `Service<Console>`. The source still carries the implementation's
separate qualification; `ENTRY-CONTENT-ROOTS` owns its removal under the settled
intrinsic binding-validity contract.

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
