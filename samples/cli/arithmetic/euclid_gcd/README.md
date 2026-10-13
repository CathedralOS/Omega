# Euclid's greatest common divisor

The state-machine loop computes `gcd(48, 36)`, prints its result
message, waits for input (or EOF), and exits with code **12**. The hosted entry
uses a provisioned `Binding<Console>` with intrinsic binding validity.

`Main` contains only that binding. The static entry and states take their
context explicitly; the loop carries the two operands as state parameters.
The remainder and pause buffer are local values. No scratch field is needed
to swap the operands: the next transition receives `(right, remainder)`.

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

Before this static-parameter migration, the ordinary CLI route was verified
on macOS ARM64: package update, explicit
review decisions, resume, native compilation, and execution produce the exact
stdout above, empty stderr, and exit 12 with both EOF and Enter. On Linux
x86-64 the former compiler-library regression (filtered to this sample)
compiles, publishes, and runs natively to exit 12 with the documented stdout
under EOF input; its test-owned acceptance does not cover the ordinary CLI
review route. On Linux x86-64 `omega update --project . --target
linux_x86_64` rejects inside the `omega-language-std` package candidate check
with `routed service field Filesystem::host has no exact Fused
selected-provider-plan join` before review decisions — the shared std gate
already recorded for this project at `f600f8400b7`, re-witnessed at
`b53c7ea26032`. The ordinary Linux CLI route remains open in
[SAMPLE-CORPUS](../../../../TASKS.md).

The migrated source is verified on Windows x86-64 at `b80e3d12bdd` with an
optimized dev compiler after local package review. EOF and Enter produce all
138 stdout bytes, empty program stderr, and exit 12. The published image emits
the prompt before input and waits for Enter. `omega run --both` also reports
native and Terminal exit 12; its comparison checks exits. The former
`samples_compile` Rust test target no longer exists.
