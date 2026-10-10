# Mixed Case Ledger

Depend-free compile-scope probe: mixed-shape case data and let-position
record patterns — the first sample with common fields beside payload
cases, and the first `let { ... as ... }` destructure.

- `Event` names common fields (`consumed`, `bonus`) beside payload cases:
  `Event::Treasure { gold, consumed }` sets one common field explicitly;
  `bonus` keeps its unnamed-field default zero — verified from the
  `check_default` state's parameter.
- `Point` is gated (each field 0..=100); `let { x, y as vertical, z as _ }`
  destructures exhaustively with `as` renames and a waived field — a
  record has no cases, so the pattern is total by law.

The machine stores a Treasure event, checks the default common field,
destructures a 30/40/5 point, and accepts on 30 + 40 == 70.
Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/mixed_case_ledger/main.omg
mbx run -p omega -- update --project samples/cli/probes/mixed_case_ledger
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
