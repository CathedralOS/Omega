//! Optimizer module role: stage group. Verified pass-manager fixtures by semantic scenario.
//!
//! Baseline admission, compatible-policy value numbering, exact addition, and
//! division/remainder cases are the sibling-facing fixture families. Proof
//! certificates stay below this map as shared fixture construction.

mod baseline;
mod compatible_policy;
mod division_and_remainder;
mod exact_add;
mod proof_certificates;

pub(super) use baseline::verified_empty_unit;
pub(super) use compatible_policy::{
    verified_compatible_policy_cse_unit, verified_compatible_policy_phi_gvn_unit,
};
pub(super) use division_and_remainder::{
    verified_exact_remainder_by_one_unit, verified_exact_self_divide_unit,
    verified_exact_self_remainder_unit, verified_exact_signed_remainder_by_negative_one_unit,
};
pub(super) use exact_add::{verified_exact_add_unit, verified_exact_add_zero_unit};
