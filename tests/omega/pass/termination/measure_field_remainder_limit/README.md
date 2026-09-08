# Fixed remainder-based field limit

From the repository root:

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_remainder_limit/main.omg
```

The command exits zero at source checking. The unrestricted `u64` field may
exceed the interval evaluator's signed window, but its remainder is in `0..=4`.
Adding six gives an exclusive endpoint above every possible rank.

Formation checks every operation before the pinning query reads its inputs.
Every self-edge must carry each exact endpoint field unchanged, either by
forwarding its record or copying that field into the same nominal constructor.
Complete write frames preserve those field paths through the checked prefix;
changing an unrelated field does not change the endpoint.

The [negative twin](../../../fail/termination/measure_field_remainder_limit_changed/main.omg)
replaces the limit with zero. Its range would still contain every rank, but
membership does not prove that the invocation's endpoint remains fixed.

This is source-checking support, not a native custom-view certificate. General
algebraic replacement of quotient/remainder inputs, flow-dependent formation,
and named-state transport remain separate obligations. Operand calls hoisted
into additional states also require that transport, even without authored states.
