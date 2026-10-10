# Gated Data Bounds

Depend-free compile-scope probe: gated (dependent) data — the first sample
with `data ... where` field bounds, where literal construction IS the
gate.

- `Player` carries `health >= 1, health <= 100`: `Player { health: 50 }`
  constructs because the literal proves the bounds; zero health is not a
  Player.
- `Gauge` narrows `level` to 0..=3 the same way.
- `wound` mutates inside the gate under a dominating guard — the
  `health > 30` transition arms the `health - 30` assignment, so the
  stored value still satisfies the bound and the exact-arithmetic
  obligation discharges at the same time.

The machine wounds a 50-health player to 20, reads it with a 3-level
gauge, and accepts on 20 + 3 == 23. Compile-checked only (no std
dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/gated_data_bounds/main.omg
mbx run -p omega -- update --project samples/cli/probes/gated_data_bounds
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
