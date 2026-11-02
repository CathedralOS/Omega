# Measure input propositions

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. A measure application must
form its body with the exact supplied subject before using its rank. The ordinary
identity helper publishes its admitted interval through `requires` and its value
through `ensures`; its private acyclic body supplies progress. The measure's
parameter remains a bare scalar.

The [positive](main.omg) admits zero through five, counts from both five and zero,
and traps unless both results are zero. Reversing the recursive branch returns
five on the first call and fails the oracle. Its Build file binds all four hosted
targets; actual execution evidence remains host-specific.

The [negative](../../../fail/termination/constrained_measure_parameter_domain/main.omg)
changes the helper's admitted floor to one while `walk` still admits zero.
The view cannot form for that subject, so checking must refuse the rank range.
The positive distinguishes that obligation from missing support for calls inside
a measure. Neither program adds a caller guard to repair the view's input.

## Historical declarations

Before migration commit `818f643f12d`, the negative declared
`Countdown::Remaining.value: u64 [1..=5]` and `walk.remaining: u64 [0..=5]`.
The former now belongs to `bounded_rank`'s ordinary input contract; the latter
retains `walk`'s `requires remaining <= 5` and the unsigned floor. The view still
computes the identity, its rank range is still `0..=5`, and its recursive edge
still subtracts one. The migration-only `OneToFive` domain has no independent
nominal customer and is removed.

[Scalar propositions](../../../../../wiki/spec/language/domains.md#declaration-and-membership),
[ranking](../../../../../wiki/spec/language/termination.md#ranking), and ordinary
[call formation](../../../../../wiki/spec/language/state_contracts.md) govern the
source. This reconciles these declarations; it does not close the complete
historical audit, remaining predicate propagation, customer qualification, or
suffix-parser retirement.
