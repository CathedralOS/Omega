# Slice Field View

Compile-scope probe: records holding slice views, state-level
`requires`, and `&mut`-to-`&` reborrowing. No standard-library
dependency.

- `Sample { readings: backing[1..3] }` — slice field initialized from
  an exclusive subrange of a local array.
- `machine read(...) requires 1 < values.len;` — a state restates the
  callee guard's bound as its own requires clause.
- `Counts::clone(&mut merged)` — `&mut` reborrow weakens to the
  callee's shared loan.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/slice_field_view/main.omg
mbx run -p omega -- update --project samples/cli/probes/slice_field_view
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
