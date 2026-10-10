# match bound patterns

Compile-scope probe. Imports `omega::language::core::float_operations`.

- Runtime values as match patterns: `match pair.left { first -> 7,
  second -> 9, _ -> 11 }` — parameters used AS patterns compare the
  subject against bound values, on field projections and computed
  f32/f64 subjects.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/match_bound_patterns/main.omg
mbx run -p omega -- update --project samples/cli/probes/match_bound_patterns
```
