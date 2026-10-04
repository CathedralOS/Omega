//! Replay a `[copy]` record element copied out of a borrowed view.
//!
//! The checked plan retains one element read per scalar field of the copied
//! record. Replay requires the authored initializer to be exactly
//! `collection[selector]` over the bare name the reads root at, every read to
//! share that root and selector, and the reads to name the declared record's
//! fields once each in declaration order. The view keeps its loan: like a
//! shared borrow, the copy is provenance pass-through for owned custody.
