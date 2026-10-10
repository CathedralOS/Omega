# Invariant Recursion

Compile-scope probe: measured tail-recursion with a multi-clause
requires roster. No standard-library dependency.

- `requires n <= 100, acc <= 5050, 2*acc + n*(n+1) <= 10100;` —
  comma-separated clauses; the nonlinear invariant relates BOTH
  parameters (independent bounds would admit `triangle(100, 5050)`
  whose next step violates the bound).
- `terminates by n` measures the descent.
- One-arm `transition n == 0 { true -> acc }` then a bare
  `triangle(n - 1, acc + n)` tail call as the body tail.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/invariant_recursion/main.omg
mbx run -p omega -- update --project samples/cli/probes/invariant_recursion
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
