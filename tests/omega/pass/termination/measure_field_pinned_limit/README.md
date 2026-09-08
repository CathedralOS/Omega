# Countdown with a field-held limit

Check from the repository root:

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_pinned_limit/main.omg
```

The entry requirement relates two distinct fields of the same owned record.
Each recursive edge rebuilds the decreasing field and preserves the limit's
value. The selected view ranks `remaining`, not the whole record or `limit`.

The command exits zero at source checking; this fixture does not claim native
custom-view certificate support. The [negative twin](../../../fail/termination/measure_field_limit_changed/main.omg)
replaces the limit with five. Every next rank still fits, but the endpoint is
not proved equal to its invocation-fixed value, so source checking rejects.

The [invalid arrival](../../../fail/termination/measure_field_arrival_contract/main.omg)
preserves the endpoint and decreases the rank, but breaks a separate recursive
precondition. Ordinary contract checking must reject it independently of ranking.
