The compiler already accepts named `I32`/`U32` conversions from `f32`/`f64`
with ordinary integer results. This pair retains that existing behavior for
**LOWERED-FORM-CHECKER**: fractional parts truncate toward zero, and the caller
must prove finite input with a representable truncated result.

The rules are in [destination-owned conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion)
and [result-domain overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads).

The [positive](main.omg) passes values through four helpers with the complete
normal input conditions from Core. Runtime checks cover signed and unsigned
fractions, endpoints and both zero signs, including unsigned inputs between
-1 and 0. The program traps if any result is wrong. Its [Build](build.omg)
binds all four hosted targets.

The [reject twin](../../../fail/float/unqualified_integer_conversion/main.omg)
removes only `signed32`'s upper bound. It keeps the other two input conditions
and `crashes none`, with an explicit contract that prevents private input
inference from narrowing the helper. The checker rejects the missing bound;
the diagnostic calls it requires clause 2. A focused public-API test checks
and publishes the positive, reopens its portable artifact, and pins exactly
that one rejection.

The verified optimized producer from `b6c3ae8cfbf` ran the positive in native
and Psi with exit 0 and empty output on `macos_arm64`; the twin rejected only
the missing upper bound. The conversion declarations, contracts and execution
code are unchanged at fixed base `8982fd50cec`. Reviewed changes to generic
calls, integer-cast proofs and byte views are outside these plain scalar
programs. The producer keeps its actual source identity; this records reuse,
not a new compiler build. The focused retention test passed on `4447c2fea52`.

Changes are limited to the regression pair, its test and documentation. Other
hosts were not executed. The other integer destination namespaces remain on
TASKS.
