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
pub use rules::x86_64::materialize_i64_xor_zero::*;
pub use rules::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, PostAllocationMachineRuleCatalogError,
    require_post_allocation_machine_rule, selected_post_allocation_machine_rule,
};
