//! Optimizer module role: stage group. GVN fixtures by expression and dominance relationship.

mod dominating_control_flow;
mod identities;
mod local;
mod phi_translated;

#[allow(unused_imports)] // Retained as part of the shared fixture API.
pub(crate) use dominating_control_flow::scalar_dominator_gvn_unit;
pub(crate) use dominating_control_flow::{
    compatible_policy_dominator_gvn_unit, diamond_dominator_gvn_unit, dominator_gvn_unit,
    proof_certified_dominator_gvn_unit, sibling_only_gvn_unit,
};
pub(crate) use identities::{
    BitwiseNeutralOperation, SaturatingNeutralOperation, WrappingNeutralOperation,
    bitwise_literal_pair_unit, bitwise_neutral_identity_unit,
    bitwise_neutral_identity_unit_with_type_and_liveness, saturating_multiply_literal_pair_unit,
    saturating_neutral_identity_unit, saturating_neutral_identity_unit_with_type_and_liveness,
    wrapping_multiply_literal_pair_unit, wrapping_neutral_identity_unit,
    wrapping_neutral_identity_unit_with_type_and_liveness,
    wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness,
};
#[allow(unused_imports)] // Retained as part of the shared fixture API.
pub(crate) use local::scalar_local_cse_unit;
pub(crate) use local::{
    compatible_policy_local_cse_unit, local_cse_unit, proof_certified_local_cse_unit,
};
pub(crate) use phi_translated::{
    PhiTranslatedRightArm, compatible_policy_phi_translated_gvn_unit, phi_translated_gvn_fixture,
    phi_translated_gvn_unit, proof_certified_phi_translated_gvn_unit,
};
