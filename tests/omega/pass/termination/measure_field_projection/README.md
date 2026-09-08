# Direct field measure

The checked rank is the exact `u64` field of the measure's parameter. Its
receiver, field declaration, and nominal subject type must agree before a
termination summary is available to source checking or build-time admission.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_projection/main.omg
```

The [unknown receiver](../../../fail/termination/measure_unbound_projection/main.omg)
must reject during termination checking. The
[nested projection](../../../fail/termination/measure_nested_projection/main.omg)
must also reject: decrementing the outer `remaining` field does not decrease
`inner.remaining`. These fixtures test source checking, not native custom-view
certificate production.
