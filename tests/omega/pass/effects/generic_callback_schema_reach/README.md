# Closed applications of a selected generic callback

From the repository root:

```sh
cargo run -p omega -- --check tests/omega/pass/effects/generic_callback_schema_reach/main.omg
cargo nextest run -p checked-trees-to-lowered-psi --lib generic_callback_schema --no-fail-fast --no-tests fail
cargo nextest run -p terminal-codec --lib schema --no-fail-fast --no-tests fail
```

One Schema binder selects a generic machine, then calls its distinct `2` and `3`
applications; a second generic selection is unused. Publication retains both
exact callees, expected static tuples, and the unused selected contract;
source-free reload verifies those joins and interpretation returns 3. Structural
requirements keep their fixed Console contract even though neither body performs
I/O. The nested callback control selects quiet and Console contracts through the
same schema and retains each application's different row. Nominal forwarding
checks follow actual helper calls, not unrelated applications elsewhere in the
module.
Unused families require no emitted body or fabricated executable application.
The isolated identity control retains an unused Console family's contract while
its own row stays empty and source-free execution returns its input.

The decoder rejects missing application records, changed template owners,
commitments, argument kinds/order/values, and redirected same-shaped callees.
An unchanged commitment cannot authorize an independently changed callee tuple.
These relations remain ordinary producer-trusted source projections, not
authenticated original source-contract openings or full generic PCC. Generic
execution is absent: the calls execute ordinary concrete machines with ordinary
fuel accounting.
