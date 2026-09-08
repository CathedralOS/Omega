# Positive batch-size field countdown

The declared measure projects the exact `remaining` field. Every recursive edge
subtracts a positive batch size, guarded by enough remaining work. Ordinary
Exact arithmetic and constructor checking establish the next field's range;
ranking separately establishes strict descent.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_rank_step/main.omg
```

The [zero-step control](../../../fail/termination/measure_field_rank_zero_step/main.omg)
rejects a batch range that includes zero. This exercises source checking, not
native custom-measure certificates or named-state transport.
