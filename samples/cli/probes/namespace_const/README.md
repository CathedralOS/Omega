# Namespace Const

Depend-free compile-scope probe: carrier namespaces and marked generic
parameters — the first sample where a lowercase top-level const supplies
the value a data carrier's machines return.

- `const seed` is a plain module constant; free `marker()` reads it —
  the carrier namespace contributes no field binding.
- `Counter::constant()` reaches the free machine through the carrier's
  name; `Counter::echo<T [copy]>` binds a parameter whose marker makes
  returning `argument` by value legal.
- Inference spells the generic call without angle args:
  `Counter::echo(17)` picks `T == i32`.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/namespace_const/main.omg
mbx run -p omega -- update --project samples/cli/probes/namespace_const
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
