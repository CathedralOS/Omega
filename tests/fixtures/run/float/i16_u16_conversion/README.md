Named I16/U16 conversions from f32/f64 now resolve through real module
namespaces. Ordinary, Trapping and Saturating results keep their original input,
result and crash contracts. The existing general conversion pipeline handles the
16-bit results.

This witness serves the named-conversion requirement under
**LOWERED-FORM-CHECKER** and [destination-owned
conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion),
[result-domain
overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads)
and [guarded
crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).
Completing this family does not close the parent checker task.

The [positive](main.omg) calls all twelve format/policy helpers. Signed inputs
retain -32769 < value < 32768, unsigned inputs retain -1 < value < 65536, and
both exclude NaN. The -32768.75 to -32768 and -0.75 to 0 checks need truncation
toward zero. Finite saturation checks at 32768.75, -32769.75 and 65536.75 detect
a wider conversion that merely keeps its low sixteen bits. Unsigned high-bit
values, NaN, infinities and signed zero are observable. Qualified results need
explicit policy removal. A wrong result traps.

The [reject twin](../../../fail/float/i16_u16_conversion/main.omg) removes only
signed32_plain's upper bound. It retains every other condition and operation and
rejects with exactly the missing requires clause 2.

The pair was frozen at 2a2af48abe9 on fixed base 5f48e1d5d42. The verified
retained 327c38e275ad producer refused all twelve named calls. Its Core inputs
matched. The newer generic code handles open type arguments, while these calls
are concrete. Normal scalar helpers keep their storage summaries; the Trap-only
helper has no returned storage or writable parameters, and Build uses the same
host-call summaries. The baseline twin also failed on unresolved calls, so that
observation did not establish enforcement of the missing upper bound.

The fresh optimized 8378f867b0e9 producer ran the unchanged positive in native
and Psi on macos_arm64: exit 0 and empty output in 15.116 seconds. The twin
rejected exactly the missing upper bound in 13.440 seconds. All four source file
hashes match the baseline.

The roster and publication/reload/reject checks passed. Four derived bad-input
cases passed in Psi and native, checking Trap at the actual named boundary and
the host trap signal. Two-crate strict all-target Clippy, boundary, formatting
and rules checks passed. The [Build](build.omg) binds all four hosted targets;
other hosts were not executed.
