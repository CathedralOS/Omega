The compiler rejected named I64/U64 conversions from f32/f64 even when a
helper's input contract guaranteed a valid value. Their namespaces are now
modules, and the proof checker uses the stated comparisons to exclude trap
conditions. NaN complements remain distinct from positive ordering.

This witness serves **LOWERED-FORM-CHECKER** under [destination-owned
conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion),
[result-domain
overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads)
and [guarded
crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The [positive](main.omg) calls twelve parameterized helpers for ordinary,
Trapping and Saturating results. Its [Build](build.omg) binds all four hosted
targets. Runtime checks distinguish truncation toward zero, the signed minimum,
unsigned values above the signed maximum, NaN, infinities and saturation to the
actual u64 maximum. Qualified results keep their policy until explicitly
removed. Safe Trapping helpers promise crashes none and state their bounds. The
program traps on a wrong result.

The [reject twin](../../../fail/float/wide_integer_conversion/main.omg) removes
only signed32_plain's upper bound. It retains the other conditions and the
original runtime program. Exactly one diagnostic rejects the missing requires
clause 2.

The unchanged program also exposed missing source information during
publication. The compiler now retains actual intrinsic headers, checks the
operators and argument types in their source contracts, and resolves qualified
names through their declarations. Fixed mathematical vocabulary keeps its
installed identity. Numeric operations stay on their catalog-owned execution
path, and receiving checks reconstruct their complete callable identities.

The frozen pair on a7c9bf84c5e refused all twelve named calls at fixed base
5c768555ccc, using the verified applicable b6c3ae8cfbf producer. The final
optimized compiler from 58ee6c16e6c ran the unchanged positive in native and Psi
on macos_arm64: exit 0, empty output, 15.261 seconds. The twin rejected only the
missing upper bound in 13.488 seconds. All four source file hashes match the
baseline.

Three identity/receiving controls passed at 4f9c5e0415e, and the sealed
float-operator control passed at 58ee6c16e6c. Strict Clippy for the affected
identity crates, followed by the final typed-trees change, passed. Boundary,
formatting and rules checks passed.

Earlier conversion publication and proof controls remain tied to their recorded
sources. The retained four-case bad-input test passed on the recorded 7271-based
build before the proof repair, asserting Trap at the actual Psi BoundaryCall and
the native trap signal. Its numeric contracts, FloatSemantics conversion
behavior and native conversion instructions are unchanged. Other hosts were not
executed. I8/I16/U8/U16 remain on TASKS.
