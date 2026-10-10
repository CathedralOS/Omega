# Deferred Init Gated

Depend-free compile-scope probe: deferred locals and gated nested records
— the first sample declaring a primitive local without an initializer.

- `let mut total: i32 in Wrapping;` binds storage first and assigns
  later; the local keeps its own storage through assignment and
  observation, then takes a compound `+=` step.
- `Session` is a gated record (`retries <= 3`) nested inside `Registry`;
  `Registry {}` zero-fills it, and the zero value itself satisfies the
  gate — the `where` fact constrains the scalar field without hiding the
  record from establishment.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/deferred_init_gated/main.omg
mbx run -p omega -- update --project samples/cli/probes/deferred_init_gated
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
