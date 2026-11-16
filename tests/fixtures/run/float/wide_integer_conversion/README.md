Named I64/U64 conversions from f32/f64 use the destination's ordinary,
Trapping or Saturating result. Ordinary calls require finite input whose
truncated value fits the integer carrier. Trapping calls trap when that
condition fails. Saturating calls clamp to the integer bounds and map NaN
to zero. Every normal result truncates toward zero; qualified results keep
their policy until explicitly removed.

This witness serves **LOWERED-FORM-CHECKER**. Its rules are
[destination-owned conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion),
[result-domain overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads)
and [guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The [positive](main.omg) has twelve parameterized helpers and a
[Build](build.omg) for all four hosted targets. Its runtime checks distinguish
fraction truncation, the signed minimum, large exact unsigned values above
the signed maximum, NaN, infinities and saturation to the actual u64 maximum.
The program traps on a wrong result. Safe Trapping helpers explicitly promise
crashes none and prove the conversion's normal-input conditions.

The [reject twin](../../../fail/float/wide_integer_conversion/main.omg)
removes only signed32_plain's upper bound. The ordinary converter still
requires that bound; the other two conditions and the runtime program stay
unchanged.

At fixed base 5c768555ccc, the frozen pair on a7c9bf84c5e refused all twelve
named calls because their names resolved to no declaration. The actual
optimized producer was b6c3ae8cfbf, reused after checking that the scalar
conversion paths were unchanged. These I64/U64 owners were empty data
namespaces. The repair makes them modules and preserves each existing
conversion contract as a boundary machine; the authoritative Core roster
records the actual declaration form.

Focused publication, invalid-input trap controls and optimized execution are
pending. Other hosts have not been executed. I8/I16/U8/U16 conversion
namespaces remain separate TASKS acceptance.
