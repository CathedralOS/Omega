//! Optimizer module role: stage group. Control-flow cleanup fixture map.

mod block_merging;
mod conditional_branches;
mod empty_blocks;
mod parameters;
mod shared_jump_fusion;

pub(crate) use block_merging::{adjacent_conditional_merge_unit, non_adjacent_merge_unit};
pub(crate) use conditional_branches::{
    constant_conditional_dead_service_unit, constant_conditional_same_target_unit,
    propagated_block_parameter_unit,
};
pub(crate) use empty_blocks::{linear_empty_block_unit, path_qualified_empty_block_unit};
pub(crate) use parameters::redundant_block_parameter_unit;
pub(crate) use shared_jump_fusion::shared_terminal_unit;
