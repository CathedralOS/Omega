# Tuple Matrix Dispatch

Compile-scope probe: multi-subject case exhaustiveness as a
CARTESIAN proof. No standard-library dependency.

- `transition (main.direction, main.result, approved)` — tuple
  subject; the arms cover every (Direction, Outcome, bool) triple
  without a `_` arm.
- `domain Direction::Horizontal requires direction in Left | Right`
  — a pure case-union domain refines the tuple's first member; the
  remaining (Up, Hit/Miss, true/false) matrix spells out.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/tuple_matrix_dispatch/main.omg
mbx run -p omega -- update --project samples/cli/probes/tuple_matrix_dispatch
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
