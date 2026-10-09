//! The path-derived relocation window, now through the shared derivation
//! and audit: `crossed_window` enumerates every acyclic path between the
//! run's block and the destination's — in the direction this family's
//! own dominance bounds established — and returns the crossed positions
//! and crossed edges; `admit_run_relocation` proves that window
//! independent once. What stays local is what makes this family the
//! general admission: the sink/hoist once-per-traversal bounds
//! (dominance, re-entry, continuations), the member's pure-work bound,
//! and the skipped-edge dead-path audit a sink owes.
