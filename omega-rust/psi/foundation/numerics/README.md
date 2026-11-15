# Numeric semantics implementation

[Numeric values](../../../../wiki/spec/language/numeric_values.md) defines the
language contract. This crate provides shared executable arithmetic, not
target instruction selection or permission to approximate an operation.

[float_semantics.rs](src/float_semantics.rs) decodes landed binary32/binary64
operands into exact rational or special-value meanings and implements rounding,
arithmetic, comparisons, classification, square root, conversion, directed
operations, fused/unfused arithmetic, and policy adapters. Its format records
match the [core records](../../../../source/library/core/float_format.omg).
[bignum.rs](src/bignum.rs) supplies exact arithmetic and rounding; host `f64`
does not define the meaning of a landed `f32` operation.

`apply_trapping_policy` checks only the semantic result.
`apply_saturating_policy` clamps infinity only for finite operands; division
has its own adapter to keep signed-zero divisor behavior. Float-to-integer
saturation has its own clamp and NaN-to-zero rule. Reusing
the arithmetic saturation adapter for conversion would change semantics.

[lib.rs](src/lib.rs) (`float_projection`) owns recognized-core projection
descriptors; semantic `FloatMeaning` comparison does not expose NaN
payloads.

Target providers use these meanings through selected plans. Their
[realization note](../../../../wiki/drafts/reference/float_realization.md)
separates instruction custody, image replay, and native execution evidence.
A future format needs complete executable support, not just another record
that a binary32/binary64-specific adapter accepts.
