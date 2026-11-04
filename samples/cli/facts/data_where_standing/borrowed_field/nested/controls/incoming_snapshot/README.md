# Keep a copied reference when its containing pointer changes

The helper copies a reference to the old scalar, replaces `holder.counter`,
and reads the copied reference again. The old read must keep its value and its
bound. The exact addition uses that read after the replacement. Runtime checks
also verify the new pointer and unchanged nonzero neighboring fields.

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

This is the incoming-storage gap in the existing nested borrowed-field fact,
under **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. The positive runs both 60-to--17
and -17-to-60 replacements and expects exit 0. One twin removes only Counter's
predicate; the identity twin instead asks the replacement to equal the old read.
Both must reject. All four hosted entry bindings are authored.

The frozen optimized baseline `cd5340c6` rejects the positive at the exact
addition: it cannot prove the upper bound of the read after pointer replacement.
Both controls currently stop at that same arithmetic failure; the identity
control has not yet reached its intended missing equality. The initial sources,
profile and raw diagnostics are retained before the compiler edit.

The first proposed repair separates logical entry-predicate values from actual
scalar reads. Native/Terminal execution and all nine transport places remain
required; this control alone does not complete the fact.
