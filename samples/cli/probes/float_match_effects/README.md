# Float Match Effects

Compile-scope probe: float literals and call-pattern match arms — the
first sample where a match arm's pattern is itself a call. One core
import (`float_operations`) supplies the selected float-equality
operator meaning; no standard-library dependency.

- `bump(&mut calls, x)` is invoked once per arm until a literal compares
  equal — `calls` reaches exactly 3, proving ordered first-match
  evaluation across call-pattern arms.
- `1.0f64`/`1.0f32` literals match as patterns in both `match` subjects
  and `match` arms.
- `values[0u64]` indexes a `[f32; 1]` with a u64 index literal — the
  index spelling carries its own type suffix.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/float_match_effects/main.omg
mbx run -p omega -- update --project samples/cli/probes/float_match_effects
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
