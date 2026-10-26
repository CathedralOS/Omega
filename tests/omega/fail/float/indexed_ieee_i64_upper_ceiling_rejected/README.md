# An indexed IEEE value must stay below the i64 ceiling

The true saved guard establishes a non-NaN, nonnegative completed array element,
but its inclusive upper bound still admits exactly 2^63. That value cannot
truncate into `i64`. The unqualified selected `I64::from_f64` conversion must
reject even when the normal-operation proof owner can retain the exact index
and completed float. The format's representable upper endpoint is not the
largest integer of the destination carrier.

This pins the signed 64-bit representation boundary in
[numeric values](../../../../../wiki/spec/language/numeric_values.md), preserving
finite input and representable truncation as independent obligations. The
pre-repair 218 producer rejects for the intended finite/truncation admission
reason; the repaired shared producer must retain that refusal.
