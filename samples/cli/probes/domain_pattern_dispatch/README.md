# Domain Pattern Dispatch

Compile-scope probe: domain-pattern transition arms, stacked domain
refinement, and a scalar interval domain on a record field. No
standard-library dependency.

- `Player::Alive`/`Player::Dead` refine `Player::Valid` — a domain
  `requires` may itself be a domain membership (`in Player::Valid`).
- `transition main.player { Player::Alive -> ... }` — arms test domain
  membership in order; `_` closes the dispatch.
- `domain i32::Ten` bounds a scalar by interval (`0 <= v && v <= 10`);
  `level: i32 in Ten` is a record field whose literal construction
  proves the range (`Gauge { level: 7 }`; 11 is the refused negative).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domain_pattern_dispatch/main.omg
mbx run -p omega -- update --project samples/cli/probes/domain_pattern_dispatch
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
