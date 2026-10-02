# Authored wire-plan round trip

`CompactBinary::plan` uses its explicit mutable context to construct the wire
plan at build time. The unchanged program encodes counter 300, delta -2 and
flag true, checks the eight-byte extent and decoded values, then exits 70.

Prepare and review the local package graph on Linux x86-64:

```sh
omega update --project tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit --target linux_x86_64
# Review the fixture's build/package-manager/review-linux_x86_64.txt decisions.
omega update --project tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit --resume
omega run --both tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit/main.omg
```

The native and checked-interpreter results must both be 70. Package acceptance
belongs to this checkout and does not constitute a dependency audit.
