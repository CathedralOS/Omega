# machine param schema

Compile-scope probe. No standard-library dependency.

- machine_param_schema: `apply<machine Schema, machine Unused>`
  with `requires machine X<const Number: u64>(v) -> u64 reaches
  Console` const+reach bounds; `Schema<2>(v)` calls;
  const-generic machines passed by name.
- ensures_fact_list: multi-clause `ensures` rosters
  (`count > 0; count <= slots.len`) and `requires` quoting
  member expressions like `counter.slots.len`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/machine_param_schema/main.omg
mbx run -p omega -- update --project samples/cli/probes/machine_param_schema
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
