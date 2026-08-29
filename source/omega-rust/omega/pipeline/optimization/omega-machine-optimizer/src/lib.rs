#![forbid(unsafe_code)]

//! Target-aware machine analysis, planning, and explicitly named rules.
//!
//! Start here to choose the rung you need: [`analyses`] computes immutable
//! facts, [`planning`] joins facts with physical homes, and [`rules`] contains
//! opt-in symbolic transformations grouped by target and exact rule name.

pub mod analyses;
pub mod planning;
pub mod rules;

pub use analyses::pre_allocation_effects::*;
pub use planning::post_allocation::*;
pub use rules::aarch64::compare_zero_branch_nonzero::*;
pub use rules::aarch64::materialize_i64_movn::*;
