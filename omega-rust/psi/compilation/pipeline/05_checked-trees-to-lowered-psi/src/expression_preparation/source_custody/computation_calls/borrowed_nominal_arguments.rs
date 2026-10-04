//! Borrowed aggregates retain their declared owner and exact call occurrence.
//! Fixed arrays loan the same established storage as records; their formal
//! shape retains the length and element type without a nominal declaration.
//! Records may retain supported field projections; scalar sums are observed
//! whole at their established home, including ordinary returned case values.
//! The implicit receiver precedes explicit actuals but has no row in their
//! observation roster. Validate it separately against actuals whose storage
//! remains live during the invocation; scalar evaluation has already finished.
//! A projected operand has two identities: the caller's storage root and the
//! authored endpoint captured by the call. Reconstruct both from the source;
//! an equal endpoint type does not permit substituting a sibling or another root.
