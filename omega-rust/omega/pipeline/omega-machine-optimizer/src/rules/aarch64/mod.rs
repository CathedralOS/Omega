//! Optimizer module role: stage group. AArch64-only symbolic machine transformations.

pub mod compare_zero_branch_nonzero;
pub mod elide_same_view_copy_before_compare_i64_left_operand;
pub mod elide_same_view_copy_before_compare_i64_right_operand;
pub mod elide_same_view_copy_before_compare_zero;
pub mod elide_same_view_copy_before_return;
pub mod materialize_i64_movn;
pub mod same_view_copy_elision;

mod same_view_copy_before_compare;
