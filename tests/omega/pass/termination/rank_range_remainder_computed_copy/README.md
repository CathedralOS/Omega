# Remainder endpoint through a computed operand copy

From the repository root:

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/rank_range_remainder_computed_copy/main.omg
```

The command exits zero at source checking. The exclusive ceiling `cap % 5 + 6`
keeps a truncating-remainder atom in the endpoint, and `remaining <= 5` fits
under it because the operand is unsigned. The backedge passes `cap - 0`: not a
bare forward, so the exact endpoint identity is re-proved relationally — the
remainder atom is re-minted under the transported operand rather than matched
by its rendered text. The rank still strictly decreases.

The [negative twin](../../../fail/termination/rank_range_remainder_moved_copy/main.omg)
delivers the endpoint's operand from a different parameter, changing the
endpoint's value across the edge, so source checking rejects.
