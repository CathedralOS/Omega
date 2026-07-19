mod leaf;
mod mutation;
mod prelude;
mod straight_line;

pub(super) use leaf::{
    LeafBranchSelectionScratch, leaf_expansions_defer_to_local_initializer,
    push_branch_arms_end_marker, push_branch_scope_marker,
    select_runtime_leaf_branch_expansion_for_tree,
    select_runtime_leaf_branch_expansions_for_operation,
};
pub(in crate::selection) use prelude::{
    BranchPreludeSelectionScratch, select_runtime_branch_preludes_for_operation,
};
pub(in crate::selection) use straight_line::{
    StraightLineBranchSelectionScratch, select_assignment_value_call_result_local_copy,
    select_runtime_straight_line_branch_expansions_for_operation,
    select_runtime_straight_line_nested_branch_expansions_for_operation,
};
