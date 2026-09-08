# Field measure with a pinned endpoint

The invocation supplies a rank ceiling whose declared lower bound contains the
field's enforced range. Each loop edge forwards the exact ceiling unchanged;
being within the ceiling parameter's declared type does not permit replacing it.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_rank_endpoint/main.omg
```

The [replacement control](../../../fail/termination/measure_field_rank_endpoint_replaced/main.omg)
must reject even though its new ceiling remains in the declared type range.
This checks source semantics, not native custom-measure certificates.
