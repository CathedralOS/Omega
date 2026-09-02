#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Target-aware machine analysis, planning, and explicitly named rules.
//!
//! Start here to choose the rung you need: [`analyses`] computes immutable
//! facts, [`costs`] describes non-authoritative target costs, [`planning`]
//! joins facts with physical homes, and [`rules`] contains opt-in symbolic
//! transformations grouped by target and exact rule name.

pub mod analyses;
pub mod costs;
pub mod planning;
pub mod rules;

pub use analyses::pre_allocation_effects::*;
pub use costs::*;
pub use planning::post_allocation::*;
pub use rules::aarch64::compare_zero_branch_nonzero::*;
pub use rules::aarch64::elide_same_view_copy_before_compare_i64_left_operand::*;
pub use rules::aarch64::elide_same_view_copy_before_compare_zero::*;
pub use rules::aarch64::elide_same_view_copy_before_return::*;
pub use rules::aarch64::materialize_i64_movn::*;
pub use rules::x86_64::materialize_i64_mov_r32_imm32::*;
pub use rules::x86_64::materialize_i64_mov_r64_imm32_sign_extended::*;
pub use rules::x86_64::materialize_i64_xor_zero::*;
pub use rules::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleCatalogError,
    PostAllocationMachineRuleCatalogPayload, PostAllocationMachineRuleKind,
    require_post_allocation_machine_rule, selected_post_allocation_machine_rule,
};
