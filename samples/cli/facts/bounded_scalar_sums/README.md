# Bounded Scalar Sums

Compile-scope probe: bounded scalar carriers `u64 [a..=b]` on
parameters, results, and record fields — and the exact-arithmetic
payoff (decision-17 narrowing): declared bounds prove sums cannot
overflow, so no Wrapping domain and no `as` widening is needed. No
standard-library dependency.

- `add50(a, b) -> u64 [0..=100]` — the result bound is proven from the
  parameter bounds; the body is a plain exact `a + b`.
- `main.left + main.right` is exact for the same reason — bounded
  record fields narrow the operands to [0, 100].
- `main.result: u64 [0..=100]` — a bounded field receiving the
  call result directly.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/bounded_scalar_sums/main.omg
mbx run -p omega -- update --project samples/cli/facts/bounded_scalar_sums
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
