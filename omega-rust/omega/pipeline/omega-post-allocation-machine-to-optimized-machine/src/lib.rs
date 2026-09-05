#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Catalog-driven post-allocation optimization components.
//!
//! [`rules`] owns the single enable/order catalog and the rule implementations.
//! [`execution`] is the executable catalog consumer, while target leaves retain
//! pipeline custody for their independently validated symbolic plans.

mod aarch64_cbnz;
mod aarch64_movn;
mod aarch64_same_view_copy;
mod error;
mod execution;
pub mod rules;
mod source;

pub use rules::aarch64::compare_zero_branch_nonzero::*;
pub use rules::aarch64::elide_same_view_copy_before_compare_i64_left_operand::*;
pub use rules::aarch64::elide_same_view_copy_before_compare_i64_right_operand::*;
pub use rules::aarch64::elide_same_view_copy_before_compare_zero::*;
pub use rules::aarch64::elide_same_view_copy_before_return::*;
pub use rules::aarch64::materialize_i64_movn::*;
pub use rules::aarch64::same_view_copy_elision::*;
pub use rules::x86_64::materialize_i64_mov_r32_imm32::*;
pub use rules::x86_64::materialize_i64_mov_r64_imm32_sign_extended::*;
pub use rules::x86_64::materialize_i64_xor_zero::*;
pub use rules::{
    ORDERED_POST_ALLOCATION_MACHINE_RULES, POST_ALLOCATION_MACHINE_RULE_CATALOG,
    PostAllocationMachineRuleCatalogEntry, PostAllocationMachineRuleCatalogError,
    PostAllocationMachineRuleCatalogPayload, PostAllocationMachineRuleKind,
    require_post_allocation_machine_rule, selected_post_allocation_machine_rule,
};

use omega_selected_instructions_to_register_homes::AllocationSource;
use source::replay_machine_source;
mod model;
mod x86_mov_r32_imm32;
mod x86_mov_r64_imm32_sign_extended;
mod x86_xor_zero;

pub use aarch64_cbnz::*;
pub use aarch64_movn::*;
pub use aarch64_same_view_copy::*;
pub use error::OptimizedPostAllocationMachineOptimizationError;
pub use execution::*;
pub use model::*;
pub use x86_mov_r32_imm32::*;
pub use x86_mov_r64_imm32_sign_extended::*;
pub use x86_xor_zero::*;

use omega_register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
