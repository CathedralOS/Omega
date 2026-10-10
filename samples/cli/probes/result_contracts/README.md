# Result Contracts

Compile-scope probe: `ensures result == ...` result contracts. No
standard-library dependency.

- `exact_sum() -> u8 ensures result == 7` — the body's `3u8 + 4u8`
  discharges the equation statically.
- `bounded(...) requires input <= 255u16 ensures result == input ||
  result < input` — guarded parameter + disjunctive result
  contract; the call passes `input % 256u16`.
- `Main::accept(5)` — a literal discharges `requires value > 0`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/result_contracts/main.omg
mbx run -p omega -- update --project samples/cli/probes/result_contracts
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
