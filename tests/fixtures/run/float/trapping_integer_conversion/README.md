Named `I32`/`U32` conversions from `f32` and `f64` support `Trapping`
results. Valid inputs truncate toward zero. NaN, infinity and values whose
truncated result is out of range trap. The result keeps `Trapping` until an
explicit cast removes it.

This pair covers the named Trapping conversion requirement in
**LOWERED-FORM-CHECKER**. The rules are in
[numeric values](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion)
and [guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The [positive](main.omg) passes values through four helpers. Each
promises `crashes none` and requires the conversion's Trap guard to be false.
The runtime checks distinguish toward-zero conversion from rounding, exercise
signed and unsigned endpoints and both zero signs, and include unsigned inputs
between -1 and 0. The program traps if a result is wrong. Its
[Build](build.omg) binds all four hosted targets.

The [reject twin](../../../fail/float/trapping_integer_conversion/main.omg)
changes only `signed32`'s input guarantee to `requires true`, keeping
`crashes none`. The checker rejects its uncovered Trap route. Explicit
`requires true` prevents private input inference from narrowing that contract.

The compiler preserves the normal result guarantee and complete guarded crash
contract. Artifact checks reject a missing guard, the wrong crash cause and an
unconditional replacement. Proof checking uses the existing kernel judgment for
false IEEE comparisons with known inputs, retaining the actual value equations.
A focused proof test checks those dependencies and refuses the NaN
counterexample; this repair introduces no arithmetic rule.

Runtime controls permit the live Trap route. They convert NaN and finite
overflow to i32, and negative input and infinity to u32. They
assert Trap at the actual named boundary in Psi and the native trap signal.
Wrong normal completion exits successfully, so saturation or an unchecked
conversion cannot satisfy the trap check. These controls create no additional
tracked Omega programs.

The optimized compiler from `b6c3ae8cfbf` ran the original positive in native
and Psi with exit 0 and empty output on `macos_arm64`; the twin rejected only
the uncovered Trap route. Three focused tests and five-crate strict all-target
Clippy passed. The final test assertion merely pins that observed diagnostic.
All four source files match the recorded baseline, which refused the four calls.
Other hosts were not executed. Unqualified conversions and other integer
destinations remain outside this acceptance.
