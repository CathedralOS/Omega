# Tuple Case Payloads

Compile-scope probe: tuple case patterns binding payloads from
multiple subjects. No standard-library dependency.

- `(Cell::Value { n as left_n }, Cell::Value { n as right_n })` —
  per-subject payload destructuring with `as` rename; the bindings
  stay in scope for the arm target.
- The remaining (Value,Empty / Empty,Value / Empty,Empty) matrix
  cells spell out — Cartesian-exhaustive without `_`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/tuple_case_payloads/main.omg
mbx run -p omega -- update --project samples/cli/probes/tuple_case_payloads
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
