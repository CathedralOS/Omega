Named saturating conversions from `f32` and `f64` to `i32` and `u32` truncate
toward zero, clamp values outside the integer range, and map NaN to zero. The
returned value keeps `Saturating` until the program explicitly removes it.

This pair covers the named integer-result conversion gap in
**LOWERED-FORM-CHECKER**. Its rules are in
[numeric values](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion)
and [result-domain overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads).

The [positive](main.omg) passes runtime operands through four helpers. Its
checks distinguish truncation from rounding, signed from unsigned bounds,
finite overflow from ordinary values, and NaN from infinity. Both zero signs
produce integer zero. The program traps if any result is wrong, and its Build
binds all four hosted targets.

The [negative](../../../fail/float/saturating_integer_conversion/main.omg)
keeps the same callers and values. It removes only the first helper's explicit
`as -Saturating`, so the checker must reject returning a qualified result as a
plain integer. All four helpers declare `requires true`; private input inference
cannot narrow their input contracts.

The compiler must retain the selected Core declaration, the exact source and
result carriers, the complete integer-result contract, and the result policy.
Native checking must reject replacing saturation with truncation or erasing
that policy from retained declarations.

Other integer destinations and `Trapping` remain separate requirements. Runtime
observations apply only to the host recorded in the PR; authored bindings alone
do not establish execution on the other hosts.
