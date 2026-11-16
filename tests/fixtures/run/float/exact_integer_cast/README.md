# Exact float-to-integer casts

An `f32` or `f64` value can be cast to `i32` with `as` when its input contract
proves that conversion preserves its exact meaning. This **LOWERED-FORM-CHECKER**
pair passes 42 and −17 through ordinary parameters and checks both signed results
in [the program](main.omg). Both Build files bind all four hosted targets.

[The reject twin](../../../fail/float/exact_integer_cast/main.omg) changes only
the two entry guarantees: finite intervals replace the two exact alternatives.
The callers and runtime checks stay identical. The intervals satisfy the named
converter's finite/range requirements but permit fractional values, so the
checker must reject both casts for missing exactness.

The cast establishes a source-only eligibility invariant before invoking Core's
converter. Eligibility requires a finite integral value in the destination's
exact integer range and excludes negative zero. The named converter may truncate
fractions; its success does not establish cast safety. Its full entry contract
and integer-result relation remain in the published artifact.

The checker uses a [provisional trusted conversion rule](../../../../../wiki/spec/proofs/classicality.md)
with independently checked premises. IEEE equality to a finite nonzero literal
can transport these goals; IEEE equality to zero cannot erase a zero sign.
The independent miniature arithmetic model and forged-premise controls exercise
the kernel separately from this executable pair.

Runtime evidence covers only the executed host. This pair does not establish
other transport places, integer-to-float casts or every named conversion family.
