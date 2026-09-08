# Relational field-rank range

`requires` relates the exact `remaining` field to an invocation-fixed ceiling.
The range proof checks entry membership, then substitutes each reconstructed
field and every scalar argument simultaneously. Every recursive edge must keep
the endpoints fixed, remain in range, and strictly decrease.

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_relational_range/main.omg
```

The [changed-ceiling control](../../../fail/termination/measure_field_relational_endpoint_changed/main.omg)
rejects even though the replacement ceiling contains every possible next field
value. This is source-checking coverage; native custom-view certificates and
named-state field transport remain separate work.
