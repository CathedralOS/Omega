//! Optimizer module role: stage group. Independent CFG rewrite validation by exact graph transformation.

mod block_merging;
mod constant_conditionals;
mod contract;
mod empty_block_threading;
mod shared_jump_fusion;
mod unreachable_private_machines;

pub use block_merging::{
    validate_adjacent_block_merge_candidate, validate_non_adjacent_block_merge_candidate,
};
pub use constant_conditionals::validate_constant_conditional_candidate;
pub use empty_block_threading::{
    validate_linear_empty_block_candidate, validate_path_qualified_empty_block_candidate,
};
pub use shared_jump_fusion::validate_shared_jump_fusion_candidate;
pub use unreachable_private_machines::validate_unreachable_private_machines_candidate;
