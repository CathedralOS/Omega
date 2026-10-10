# Synthesized Equatable

Depend-free compile-scope probe: trait declarations and named
implementations — the first sample declaring a `trait`, satisfying it with
an EMPTY named implementation (which receives a synthesized structural
`equals` row), and overriding synthesis with an authored body.

- `PointEquality satisfies Equatable<Point> { }` synthesizes
  field-by-field equality, so `==`/`!=` on Point is legal and
  `PointEquality::equals` selects the same synthesized row.
- `OverrideEquality satisfies Equatable<Token>` carries an authored
  `equals` that always returns false, so `==` on two identical Tokens
  reports unequal — the authored member replaces synthesis.

The machine verifies both faces: equal points compare true, differing ones
false (operator and callable), and identical tokens compare false through
the override. Compile-checked only (no std dependency, no ProcessExit
provider):

```sh
mbx run -p omega -- --check samples/cli/probes/synthesized_equatable/main.omg
mbx run -p omega -- update --project samples/cli/probes/synthesized_equatable
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
