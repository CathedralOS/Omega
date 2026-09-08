# Numeric semantics implementation

[Numeric values](../../../../wiki/spec/language/numeric_values.md) owns the
language contract. This crate provides shared executable arithmetic, not
target instruction selection or permission to approximate an operation.

[float_semantics.rs](src/float_semantics.rs) decodes landed binary32/binary64
operands into exact rational/special-value meanings and implements rounding,
arithmetic, comparisons, classification, square root, conversion, directed
operations, fused/unfused arithmetic, and policy adapters. Its format records
match [core records](../../../../source/library/core/float_format.omg).
[bignum.rs](src/bignum.rs) supplies exact arithmetic/rounding machinery;
host `f64` is not the semantic implementation of a landed `f32` operation.

`apply_trapping_policy` checks only the semantic result.
`apply_saturating_policy` clamps infinity only for finite operands; division
uses its separate adapter to preserve signed-zero divisor behavior.
Float-to-integer saturation has its own clamp/NaN-to-zero law. Reusing the
arithmetic saturation adapter for conversion would change semantics.

[float_projection.rs](src/float_projection.rs) owns recognized-core projection
descriptors; semantic `FloatMeaning` comparison does not expose NaN payloads.
Source/Terminal projection support and pending denotation work live
[beside validation](../../semantics/validation/numeric_proofs.md).

Target providers consume these meanings through selected plans. Their
[realization note](../../../omega/compiler/compiler/float_realization.md)
distinguishes instruction custody, image replay, and native execution evidence.
Future formats require complete executable support, not just another record
accepted by a currently binary32/binary64-specific adapter.
