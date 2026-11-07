# Intended-invalid integer rate initializer

This is a complete source project with an explicit Linux entry binding.
The initializer `TransitionRate { thousandths: true }` contradicts its
`i64` field type. There is no implicit boolean-to-integer assignment conversion.
See [numeric values](../../../../../../wiki/spec/language/numeric_values.md),
which distinguishes explicit conversions from assignments.

Check `invalid.omg` separately. Its explicit entry binding is independent of the filename.
The filename keeps this intentional rejection out of normal sample refresh discovery. Require a diagnostic naming the boolean/integer
initializer mismatch, and exit 1. Any package-review refusal, unsupported route,
parsing error or unrelated diagnostic is not a pass.
