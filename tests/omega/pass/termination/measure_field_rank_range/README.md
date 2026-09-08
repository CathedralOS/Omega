# Field measure rank range

`Countdown::Remaining` projects an exact field declared in `0..=5`. Its rank
range must contain that field's enforced bounds. The guarded reconstruction
separately proves that subtracting one keeps the new field within its range.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_rank_range/main.omg
```

The [ceiling control](../../../fail/termination/measure_field_rank_ceiling/main.omg)
must reject: a descending loop may still enter with a rank above its declared
ceiling. This is source-checking coverage, not native custom-view evidence.
Named-state transport, dynamic endpoints and general projections need their
own range proofs.
