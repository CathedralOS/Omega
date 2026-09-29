# Destination-owned float-to-integer conversions

From the repository root, run:

```sh
mbx run -p omega -- run tests/omega/pass/float/runtime_named_float_to_integer_conversion_exit/main.omg
```

The authored result is exit **70**. This exercises exact `f32`/`f64`
conversion to all eight integer carriers, result-selected Trapping and
Saturating overloads, and NaN saturation. Exact calls must discharge the actual
boundary requirements through independently reconstructed literal evidence.

`runtime_float_integer_conversion_boundaries_exit` checks valid negative
fractions below the signed minimum, the unsigned high range, and U64 rounding.
The `named_float_integer_{nan,lower_boundary}_conversion_traps` siblings require
the target's explicit crash, not any arbitrary process failure.

Run the matching-host native corpus gate to check the exit and trap oracles;
cross-target image generation alone does not establish execution.
