# Slice Field View

Compile-scope probe: records holding slice views, state-level
`requires`, and `@mutable`-to-`@` reborrowing. No standard-library
dependency.

- `Sample { readings: backing[1..3] }` — slice field initialized from
  an exclusive subrange of a local array.
- `machine read(...) requires 1 < values.length;` — a state restates the
  callee guard's bound as its own requires clause.
- `Counts::clone(@mutable merged)` — `@mutable` reborrow weakens to the
  callee's shared loan.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/slice_field_view/main.omg
mbx run -p omega -- packages update --project samples/cli/probes/slice_field_view
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
