# Boundary Crash Route

Depend-free compile-scope probe: boundary machines and conditional crash
routes — the first sample declaring a `boundary machine` and satisfying
it.

- `boundary machine == Comparison::equal` overloads `==` for i32 through
  the Comparison carrier and declares a conditional crash route:
  `crashes Trap !(right >= 0)` — any `==` whose right operand can't be
  proven non-negative propagates the Trap route.
- `ComparisonProvider::equal` satisfies the boundary machine — the
  provider is an ordinary machine bound by `satisfies`.
- `safe` discharges the route with `requires value >= 0`; `may_crash`
  propagates it and `wrapper` covers it by declaration.
- The entry calls both on the dormant path (right = 0 ≥ 0).

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/boundary_crash_route/main.omg
mbx run -p omega -- update --project samples/cli/probes/boundary_crash_route
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
