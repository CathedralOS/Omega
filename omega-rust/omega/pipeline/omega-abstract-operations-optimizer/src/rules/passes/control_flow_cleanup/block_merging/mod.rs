//! Optimizer module role: stage group. Adjacent and non-adjacent unique-predecessor block-merge rules.
//!
//! Each rule leaf owns its exact eligibility and proposal traversal. Accounting
//! remains separate because adjacent roster compaction and non-adjacent effect
//! shifts have materially different provenance obligations. Shared ownership
//! custody sits at the parent control-flow-cleanup level beside its other
//! consumer, shared-jump fusion.

mod adjacent;
mod adjacent_accounting;
mod non_adjacent;
mod non_adjacent_accounting;
mod substitutions;

pub use adjacent::AdjacentBlockMergeRule;
pub use non_adjacent::NonAdjacentBlockMergeRule;
