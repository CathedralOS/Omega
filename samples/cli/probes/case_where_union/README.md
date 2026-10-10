# Case Where Union

Depend-free compile-scope probe: per-case `where` clauses on generic enum
data — the first sample declaring `case ... where` bounds.

- `Value<T>` gates each case on the type parameter: `Integer` admits only
  `T == i32`, `Boolean` only `T == bool`. The same declaration is a
  different union per instantiation — `Boolean` is refuted on
  `Value<i32>`, so transitions on `Value<i32>` never arm that case.
- Construction is the gate: `Value::Integer { value: 7 }` at
  `Value<i32>` proves `T == i32` at build time.
- Transition destructuring binds the payload by its declared field name
  (`Value::Boolean { value }`); renaming inside the pattern is not
  spellable — rename the carrier instead.

The machine stores Integer(7) and Boolean(true) at their own
instantiations, destructures each, and accepts on 7 && true.
Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/case_where_union/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_where_union
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
