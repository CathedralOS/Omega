# Authored wire-plan round trip

`CompactBinary::plan` uses its explicit mutable context to construct the wire
plan at build time. The program encodes counter 300, delta -2 and flag true,
checks the exact bytes `00 00 AC 02 01 03 02 01`, the eight-byte extent and
decoded values, then exits 70. The byte oracle is independent of the decoder:
an encoder and decoder agreeing on the same wrong framing must still fail.

Prepare and review the local package graph on the execution host (shown for
macOS ARM64; use `linux_x86_64` for Linux x86-64):

```sh
omega update --project tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit --target macos_arm64
# Review the fixture's build/package-manager/review-macos_arm64.txt decisions.
omega update --project tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit --resume
omega run --both tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit/main.omg
```

The native and checked-interpreter results must both be 70. Package acceptance
belongs to this checkout and does not constitute a dependency audit.
