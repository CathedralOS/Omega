# Computed field-held rank limit

From the repository root:

```sh
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_computed_limit/main.omg
```

The command exits zero at source checking. The field's declared `0..=5` bounds
establish that `limit + 1` is a defined Exact `u64` value; the entry requirement
places the rank below that exclusive endpoint. Every backedge separately proves
that the endpoint stays fixed and the rank decreases.

The [negative twin](../../../fail/termination/measure_field_computed_limit_overflow/main.omg)
allows any `u64` limit. A limit of `18446744073709551615` makes the addition overflow even
though the ranked field is small, so source checking rejects.

This is arithmetic formation and source ranking evidence, not a native
custom-view certificate. Flow-dependent formation and broader projection or
non-polynomial endpoint substitution remain separate support boundaries.
