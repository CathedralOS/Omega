# Refined conversion targets

Run from the repository root:

```sh
omega run --both tests/omega/pass/float/runtime_refined_conversion_target_exit/main.omg
```

Native and Terminal execution exit 0 on macOS ARM64. Both Exact conversions
retain the helper's value. The old scalar target suffixes become ordinary
postconditions on the converted results; demanding those postconditions never
proves representability or unchanged source denotation. The original bodies and
authored oracle remain unchanged. Local-capture and later-state variations also
execute with exit 0 through the same custody and conversion judgments.

## Historical scalar positions

All nine positions at `f2714a1d179` keep their original values and carriers:

| Position | Original suffix | Canonical fact |
|---|---|---|
| positive `sample` result | `f64[3.0..=3.0]` | `ensures result == 3.0` |
| positive `integer` cast target | `i32[3..=3]` | bare Exact cast; `ensures result == 3` |
| positive `narrow` cast target | `f32[3.0..=3.0]` | bare Exact cast; `ensures result == 3.0f32` |
| negative `sample` result | `f64[3.0..=3.0]` | `ensures result == 3.0` |
| negative `outside` cast target | `i32[4..=4]` | bare Exact cast; false `ensures result == 4` |
| negative `float_outside` cast target | `f32[4.0..=4.0]` | bare Exact cast; false `ensures result == 4.0f32` |
| negative `excluded` cast target | `f32[0.0..3.0]` | bare Exact cast; false `ensures result >= 0.0f32 && result < 3.0f32` |
| negative `fractional` cast target | `i32[3..=3]` | bare Exact cast; `ensures result == 3` cannot authorize truncation of 3.75 |
| negative `rounded` cast target | `f32[3.0..=3.0]` | bare Exact cast; `ensures result == 3.0f32` cannot authorize rounding binary64 `3.0000000000000004` |

`fail/float/refined_conversion_target_requires_source_proof` retains every
original conversion body, with the two fractional/lossy denotation refusals.
Those source-validation errors precede postcondition checking, so
`fail/float/refined_result_wrong_or_excluded_bounds` independently checks the
three false output demands. Neither phase masks the other in the coverage.
No interval-only nominal domain substitutes for a scalar proposition.
