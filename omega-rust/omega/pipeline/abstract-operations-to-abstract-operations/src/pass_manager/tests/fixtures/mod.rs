//! Optimizer module role: stage group. Verified pass-manager fixtures by semantic scenario.
//!
//! Baseline admission, compatible-policy value numbering, exact addition, and
//! division/remainder cases are the sibling-facing fixture families. Proof
//! certificates stay below this map as shared fixture construction.

mod admission;
mod baseline;
mod compatible_policy;
mod control_flow;
mod cyclic;
mod division_and_remainder;
mod exact_add;
mod proof_certificates;
mod scalars;
mod state_specialization;

pub(super) use baseline::verified_empty_unit;
pub(super) use compatible_policy::{
    verified_compatible_policy_cse_unit, verified_compatible_policy_phi_gvn_unit,
};
pub(super) use control_flow::{
    verified_conditional_distinct_returns_unit, verified_linear_empty_block_unit,
    verified_merge_parameter_unit,
};
pub(super) use cyclic::verified_unranked_cycle_unit;
pub(super) use division_and_remainder::{
    verified_exact_remainder_by_one_unit, verified_exact_self_divide_unit,
    verified_exact_self_remainder_unit, verified_exact_signed_remainder_by_negative_one_unit,
};
pub(super) use exact_add::{verified_exact_add_unit, verified_exact_add_zero_unit};
pub(super) use scalars::{
    verified_dead_literals_unit, verified_half_dead_literals_unit, verified_parameter_add_unit,
};
pub(super) use state_specialization::{
    verified_dispatch_all_constant_unit, verified_dispatch_specialization_unit,
};
