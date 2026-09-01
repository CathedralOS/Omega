//! Optimizer module role: stage group. AArch64-only symbolic machine transformations.

pub mod compare_zero_branch_nonzero;
pub mod elide_same_view_copy_before_return;
pub mod materialize_i64_movn;
