# Linear Receipt Drop

Depend-free compile-scope probe: linear data and explicit early disposal —
the first sample declaring `[linear]` data with an authored owner-attached
drop hook.

- `data Receipt [linear]` must reach a checked cleanup edge instead of
  falling out of scope — `drop(receipt)` consumes it explicitly.
- `Receipt::drop` is the owner-attached hook the compiler selects at that
  edge; it is never an authored call target. `observe(&receipt)` takes a
  shared borrow before disposal.

The machine issues a receipt at 70, observes it through a shared borrow,
drops it, and checks the observed code. Compile-checked only (no std
dependency — `omega::language::core::drop` imports cleanly without the
standard library):

```sh
mbx run -p omega -- --check samples/cli/probes/linear_receipt_drop/main.omg
mbx run -p omega -- update --project samples/cli/probes/linear_receipt_drop
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
