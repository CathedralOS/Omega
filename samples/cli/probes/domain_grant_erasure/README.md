# Domain Grants + Erasure

Compile-scope probe: four domain grant routes and two erasure routes.
No standard-library dependency.

- `Token::Issued established by TokenIssuer::issue` — provenance-locked:
  only a machine carrying `ensures result in Issued` may produce the
  qualification; `satisfies` binds the impl to the trait surface.
- `5 as i32 in Positive` — `as` grants a requires-carrying scalar
  domain when the literal proves the clause (contrast: requires
  clauses over slices/machines have no working grant — filed gap).
- `90 as i32 in Degrees` — vacuous domains grant freely.
- `x as i32` bare-target and `let plain: T = qualified` — erase
  domain (semantic + arithmetic policy) and predicate facts.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domain_grant_erasure/main.omg
mbx run -p omega -- update --project samples/cli/probes/domain_grant_erasure
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
