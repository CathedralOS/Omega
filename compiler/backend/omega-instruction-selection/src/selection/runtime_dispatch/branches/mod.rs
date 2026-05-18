mod leaf;
mod mutation;
mod prelude;
mod straight_line;

pub(super) use leaf::select_runtime_leaf_branch_expansions_for_operation;
pub(crate) use mutation::{
    select_runtime_resolved_mutation_write,
    select_runtime_resolved_mutation_write_in_table_with_scratch,
};
pub(super) use prelude::select_runtime_branch_preludes_for_operation;
pub(super) use straight_line::select_runtime_straight_line_branch_expansions_for_operation;
