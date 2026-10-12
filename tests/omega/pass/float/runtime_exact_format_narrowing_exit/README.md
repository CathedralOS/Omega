# Exact float-format narrowing

`omega run --both tests/omega/pass/float/runtime_exact_format_narrowing_exit/main.omg`
exits 0 after narrowing a selected call's positive binary64 singleton and a
parameter's negative singleton to binary32 without changing either value.
Ordinary `requires` and `ensures` establish the bounds; a demanded output
predicate cannot authorize lossy conversion of an unproved input.

A binary64 parameter equal to `(0.1f32 as f64)` also narrows unchanged. Its
contract retains the landed binary32 value rather than rereading the decimal
spelling as binary64 0.1.

This differs from `F32::from_f64`, which explicitly requests nearest-even
rounding. Both native and Terminal execution observe the authored result on
macOS ARM64. Other target bindings still need matching-host qualification.

## Historical scalar positions

Seven declarations at `1d445c0646f` retain their values and carriers:

| Position | Original suffix | Canonical fact |
|---|---|---|
| `sample` result | `f64[3.75..=3.75]` | `ensures result == 3.75` |
| `narrowed` result | `f32[3.75..=3.75]` | `ensures result == 3.75f32` |
| `narrow_parameter.value` | `f64[-3.75..=-3.75]` | `requires value == -3.75` |
| `narrow_parameter` result | `f32[-3.75..=-3.75]` | `ensures result == -3.75f32` |
| `retained_binary32.value` | `f64[0.1f32..=0.1f32]` | `requires value == (0.1f32 as f64)` |
| `main.value`, `main.negative` | corresponding binary32 singleton suffixes | bare locals retain the producers' guarantees |

The paired `fail/float/exact_format_conversion_requires_denotation` at the
same revision has two parameter suffixes (`lossy.value` at binary64 0.1 and
`exact_ends_are_not_enough.value` from one through two), plus the singleton
result and cast target in `receiving_type_is_not_proof`. The parameters now
use `requires`; the demanded binary32 3.75 is an `ensures`, with a bare Exact
cast. All seven original invalid coercions still reject. No nominal interval
domain or implicit rounding replaces those obligations.
