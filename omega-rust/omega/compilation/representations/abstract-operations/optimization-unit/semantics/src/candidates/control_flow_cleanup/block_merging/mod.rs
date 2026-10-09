//! Optimizer module role: stage group. Block-merge validation.

mod adjacent;
mod non_adjacent;

pub(crate) use adjacent::validate_adjacent_block_merge_candidate;
pub(crate) use non_adjacent::validate_non_adjacent_block_merge_candidate;
