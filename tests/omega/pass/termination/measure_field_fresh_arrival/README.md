# Field measure with a dependency-free record arrival

`walk` seeds the `iterate` state with a fresh `Countdown` literal whose fields
do not mention the ranked record at all. `pending` still occupies the ranked
slot — it is the only `Countdown` formal — so the `Countdown::Remaining` view
reads `pending.remaining` there. The arrival's produced rank (`3`) is extracted
from the literal and proved inside `0..=5`; the recursive arm still owes strict
descent on every cyclic edge.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_fresh_arrival/main.omg
```

The [stalled control](../../../fail/termination/measure_field_fresh_arrival_stalled/main.omg)
replaces the recursive literal with a constant reset above the live rank and
must reject.
