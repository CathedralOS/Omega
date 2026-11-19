# Exact float narrowing

An `f64` value can change to `f32` with `as` when the checker proves that the
conversion preserves its exact value. This **LOWERED-FORM-CHECKER** witness
first widens an actual `f32` parameter, then narrows that result. The widening
call guarantees representability before the narrowing cast runs.

[The program](main.omg) checks distinct finite values, both zero signs,
positive infinity and NaN through ordinary parameters and results. Its
`requires true` helpers cannot infer narrower input contracts. Division
distinguishes the zero signs and infinity from finite values; NaN remains
unordered under IEEE comparisons.

[The reject twin](../../../fail/float/exact_narrowing/main.omg) adds exactly
2^-30 to the widened value. At 1.5, this increment survives binary64 arithmetic
but lies between adjacent binary32 values. A range bound alone cannot prove
that the cast preserves the value, so the checker must reject it.

The checker cites the live widening equation when proving the source-only
round-trip equation. It checks every cited premise and keeps its assumption
and semantic-axiom dependencies. The conversion law is a provisional trusted
rule under [kernel rule admission](../../../../../wiki/spec/proofs/foundation.md#admitting-an-arithmetic-rule),
not a mathematical-core derivation. Meaning equality distinguishes zero signs
and treats NaN reflexively; it implies neither IEEE equality nor raw-bit
identity.

The Build files bind all four hosted targets. Runtime observations cover only
the host actually executed; they do not establish the other transport places
or integer/float casts.
