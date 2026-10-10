# Div Result Loop

Compile-scope probe: explicit Result-style error handling over a
state loop. No standard-library dependency.

- `DivResult { Ok(value), Err(code) }` — the sum-type idiom real
  apps use for recoverable failure; the error is data, not a trap.
- `main.i` counter walks `aa`/`bb` element-wise; load into `a`/`b`
  fields first — the `b != 0` arm then discharges `a / b` (facts
  live on the loaded field, not the re-read index).
- Match destructures `Ok { value }` into the handler state.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/div_result_loop/main.omg
mbx run -p omega -- update --project samples/cli/probes/div_result_loop
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
