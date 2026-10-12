# Exact integer coercion

Run from the repository root:

```sh
omega run --both tests/omega/pass/float/runtime_exact_integer_coercion_exit/main.omg
```

Native and Terminal execution both exit 0 on macOS ARM64. Other target
bindings still need matching-host execution. The helper result's source
postcondition must reach conversion checking, and the
converted integer bound must reach its caller. A live IEEE equality proves
integrality; a binary64 interval above unit spacing contains only integers.
The named conversion deliberately truncates 3.75 to 3: its weaker guarantee
must not replace the authored Exact coercion obligation.

## Historical scalar positions

Seven declarations at `615a7f1dd29` retain ordinary contracts:

| Position | Original suffix | Canonical fact |
|---|---|---|
| `sample` result | `f64[3.0..=3.0]` | `ensures result == 3.0` |
| `fraction` result | `f64[3.75..=3.75]` | `ensures result == 3.75` |
| `exact`, `truncate` results | `i32[3..=3]` | each `ensures result == 3` |
| `large_integer.value` | `f64[4503599627370496.0..=4503599627370498.0]` | the same inclusive bounds in `requires` |
| `main.value`, `main.truncated` | `i32[3..=3]` | bare locals retain the producers' guarantees |

The paired `fail/float/exact_integer_coercion_requires_denotation` at the
same revision has three parameter suffixes: `fractional.value` at 3.75,
`integer_ends_are_not_enough.value` from zero through three, and
`overflow.value` at `1e30f64`. Their `requires` keep the same endpoints.
The fractional, negative-fraction, unknown, non-integral-interior, overflow,
NaN and stale-guard coercions still reject for their authored denotation
obligations. The guarded path and all conversion bodies remain unchanged.
