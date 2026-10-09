//! Derived-metadata restamping for a transformed function.
//!
//! Optimizer applications mutate operations and node custody; every field a
//! unit derives from operation shape must then be rebuilt or the unit no
//! longer validates. This restamp is the producer-side derivation: the
//! independent validator recomputes each field from the operation list
//! itself and compares, so a transform that drifts from the contract fails
//! validation rather than silently carrying stale metadata.
