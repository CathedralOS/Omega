The compiler refused named I64/U64 conversions from f32/f64 even when a
helper's input contract guaranteed a valid value. Their namespaces are now
modules, and the proof checker uses the stated comparisons to exclude their
trap conditions. NaN comparisons remain distinct from positive ordering.

This witness serves **LOWERED-FORM-CHECKER** under
[destination-owned conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion),
[result-domain overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads)
and [guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The [positive](main.omg) calls twelve parameterized helpers, covering ordinary,
Trapping and Saturating results. Its [Build](build.omg) binds all four hosted
targets. Runtime checks distinguish truncation toward zero, the signed minimum,
unsigned values above the signed maximum, NaN, infinities and saturation to the
actual u64 maximum. Qualified results keep their policy until explicitly
removed. Safe Trapping helpers promise crashes none and state their bounds.
The program traps on any wrong result.

The [reject twin](../../../fail/float/wide_integer_conversion/main.omg)
removes only signed32_plain's upper bound. It retains the other conditions
and the original runtime program. Exactly one diagnostic rejects the missing
requires clause 2.

The frozen pair on a7c9bf84c5e refused all twelve named calls at fixed base
5c768555ccc, using the verified applicable b6c3ae8cfbf producer. After repair,
the optimized compiler from 9ed0554f5d3 ran the unchanged positive in native
and Psi on macos_arm64: exit 0, empty output. The twin rejected only the
missing upper bound; all four source file hashes match the baseline.

Six focused tests passed, including publication/reopen, checked proof
dependencies and four derived invalid-input cases that assert Trap at the
actual Psi BoundaryCall and the native trap signal. The invalid-input runtime
check preceded the proof-only repair; its conversion contracts and execution
code are unchanged. Four-crate all-target Clippy, boundary and formatting
checks passed. Other hosts were not executed. I8/I16/U8/U16 namespaces remain
on TASKS.
