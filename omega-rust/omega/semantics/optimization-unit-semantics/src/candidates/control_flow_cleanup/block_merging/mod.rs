//! Optimizer module role: stage group. Block-merge validation.

mod adjacent;
mod non_adjacent;

pub use adjacent::validate_adjacent_block_merge_candidate;
pub use non_adjacent::validate_non_adjacent_block_merge_candidate;
