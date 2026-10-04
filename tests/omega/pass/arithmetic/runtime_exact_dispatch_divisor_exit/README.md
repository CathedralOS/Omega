# Exact division after value dispatch

Both Boolean dispatch arms produce nonzero divisors. Checking must join those
arm facts before checking division, without requiring a particular spelling of
the divisor. The native program and checked interpreter must both exit **70**:
`quotient(true)` is 14 and `quotient(false)` is 6. Exit 71 means a wrong result.

From the repository root, prepare the ordinary project package review once:

```text
omega update --project tests/omega/pass/arithmetic/runtime_exact_dispatch_divisor_exit
```

Review the compiler-reported policy changes for the bundled standard library
and the application's Console binding. Edit the reported review document's
individual decisions, then complete acceptance:

```text
omega update --resume --project tests/omega/pass/arithmetic/runtime_exact_dispatch_divisor_exit
omega run --both tests/omega/pass/arithmetic/runtime_exact_dispatch_divisor_exit/main.omg
```

`--both` reads standard input to EOF before execution; this fixture needs no
input. Use `</dev/null` on macOS/Linux or `cmd /c "omega run --both
tests/omega/pass/arithmetic/runtime_exact_dispatch_divisor_exit/main.omg <NUL"`
on Windows. A CLI exit of 70 is this fixture's success result, not a compiler
failure. The local lock records package decisions, not a waiver of arithmetic
checking; do not copy another checkout's acceptance or enable a test-only policy
to run the CLI.

For the isolated arithmetic regression, the corpus gate supplies its own test
package policy and compares both authored execution oracles:

```text
python tools/corpus_gate.py --both --filter arithmetic/runtime_exact_dispatch_divisor_exit
```

The invalid control is `fail/arithmetic/exact_dispatch_divisor_unproven`: an
unconstrained divisor inside a dispatch arm must fail checking.
