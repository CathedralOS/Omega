Named I8/U8 conversions from f32/f64 now resolve through module namespaces.
Their ordinary, Trapping and Saturating contracts are unchanged, and the
existing conversion pipeline handles the byte results.

This witness serves **LOWERED-FORM-CHECKER** under [destination-owned
conversion](../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion),
[result-domain
overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads)
and [guarded
crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes).

The [positive](main.omg) calls all twelve format/policy helpers. Ordinary and
safe Trapping calls preserve the exact bounds: signed inputs satisfy -129 <
value < 128, unsigned inputs satisfy -1 < value < 256, and both exclude NaN. The
checks include -128.75 to -128 and -0.75 to 0, which need truncation toward
zero. Finite saturation checks include 128.75 to 127, -129.75 to -128 and 256.75
to 255, so keeping the low byte of a wider result cannot pass. Unsigned high-bit
values, NaN, infinities and signed zero are observable too. Qualified results
require explicit policy removal. A wrong result traps.

The [reject twin](../../../fail/float/byte_integer_conversion/main.omg) removes
only signed32_plain's upper bound. It retains every other condition and
operation, and rejects with exactly the missing requires clause 2.

The pair was frozen at 8d9404c53ed on fixed base bb15ce5e91e. The verified
applicable optimized 58ee6c16e6c producer refused all twelve named calls; its
compiler and Core inputs matched the fixed base. After repair, the optimized
327c38e275ad producer ran the unchanged positive in native and Psi on
macos_arm64: exit 0 and empty output in 15.137 seconds. The twin rejected in
13.357 seconds. All four source file hashes match the baseline.

The roster and publication/reload/reject tests passed. Four derived bad-input
cases passed in Psi and native, checking Trap at the actual named boundary and
the host trap signal. Two-crate strict all-target Clippy, boundary, formatting
and rules checks passed. The [Build](build.omg) binds all four hosted targets;
other hosts were not executed. I16/U16 remain on TASKS.
