# case where equality

Compile-scope probe. No standard-library dependency.

- case_where_equality: `case Point(lo,hi) where lo == hi` —
  case-where equality bounds; `hi - lo` discharges under the
  destructured arm.
- domain_erasure_established: `established by Trait::m` grant
  route, trait `satisfies` + `ensures result in D`, `as` and
  let-annotation erasure, `machine + i32::Degrees::add` roster
  decl. Call args never weaken a domain implicitly.
- alias_fact_transfer: entry `requires main.player in Alive` +
  `let mut copy` alias carrying the fact into a callee's
  requires.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/facts/case_where_equality/main.omg
mbx run -p omega -- update --project samples/cli/facts/case_where_equality
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
