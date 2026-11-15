# Exact float widening

An `f32` value can change to `f64` without rounding. This fixture requires
**LOWERED-FORM-CHECKER** to preserve that value through an ordinary parameter
and result, after `widen` explicitly removes the source domain.

[The program](main.omg) checks distinct positive and negative finite values,
both zero signs, positive infinity and NaN. Its `requires true` helpers cannot
infer a narrower input contract. The infinity check distinguishes infinity
from a large finite result; division distinguishes the zero signs.

[The reject twin](../../../fail/float/exact_widening/main.omg) retains the
`Reading` domain when changing the carrier. The checker must reject that cast
because a carrier cast takes an unqualified operand. Callers and runtime
checks are identical. The Build files bind all four hosted entry points;
execution evidence is limited to the actual host named in the PR.

The governing rule is [carrier casts](../../../../../wiki/spec/language/domains.md#carrier-casts).
Narrowing with a representability proof and exact integer/float carrier casts
remain separate requirements. IEEE equality does not establish raw bit identity
or equality between proof meanings in different formats.
