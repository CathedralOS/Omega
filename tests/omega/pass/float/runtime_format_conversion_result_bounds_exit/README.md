Run `omega run --both tests/omega/pass/float/runtime_format_conversion_result_bounds_exit/main.omg`.
Native and Terminal execution both exit 0 on macOS ARM64. The other authored
target bindings still need matching-host execution.

The binary64 singleton containing 0.1 rounds to the binary32 singleton containing
0.1 through the named conversion; Exact `as` cannot request lossy rounding.
Both helper `ensures` and the bare receiving local keep those rounded bounds.
The second helper exercises a fractional value inside a strict IEEE range, whose
upper endpoint must not undergo integer predecessor arithmetic.

## Historical scalar positions

Six declarations at `42fbfe08b15` retain ordinary predicates:

| Position | Original suffix | Canonical fact |
|---|---|---|
| `sample` result | `f64[0.1..=0.1]` | `ensures result == 0.1f64` |
| `rounded` result | `f32[0.1..=0.1]` | `ensures result == 0.1f32` |
| `open_window.value`, result | `f64[0.0..1.0]` | zero-inclusive, one-exclusive `requires` and `ensures` |
| `main.value`, `main.inside` | corresponding result suffixes | bare locals retain the producers' guarantees |

The related `fail/float/format_conversion_exclusive_upper` at that revision
has two declarations: a binary64 parameter with an excluded midpoint
`1.000000059604644775390625`, and a binary32 result below one. Both retain
their strict bounds as contracts. Inputs below that midpoint can still round
to one in binary32, so the demanded upper bound rejects at the return.
The named conversion, mathematical endpoints and runtime checks are unchanged;
no interval-only domain substitutes for the bounds.
