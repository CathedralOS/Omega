//! Optimizer module role: stage group. Built-in Psi transformations, organized by their explicit pass identity.

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v5";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v13";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
const DEAD_PURE_SCALAR_PASS_NAME: &[u8] = b"omega.psi-pass.dead-pure-scalar-elimination.v2";
const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v12";
const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] = b"omega.psi-pass.global-value-numbering.v14";
const STATE_SPECIALIZATION_PASS_NAME: &[u8] = b"omega.psi-pass.state-specialization.v1";
const REPRESENTATION_SPECIALIZATION_PASS_NAME: &[u8] =
    b"omega.psi-pass.representation-specialization.v1";

mod support;

mod case_membership_specialization;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod field_value_specialization;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;
mod state_specialization;

pub(super) use case_membership_specialization::built_in_registrations as case_membership_specialization_rule_registrations;
pub(super) use control_flow_cleanup::built_in_registrations as control_flow_cleanup_rule_registrations;
pub(super) use copy_propagation::built_in_registrations as copy_propagation_rule_registrations;
pub(super) use dead_scalar_elimination::built_in_registrations as dead_scalar_elimination_rule_registrations;
pub(super) use field_value_specialization::built_in_registrations as field_value_specialization_rule_registrations;
pub(super) use global_value_numbering::built_in_registrations as global_value_numbering_rule_registrations;
pub(super) use proof_check_elision::built_in_registrations as proof_check_elision_rule_registrations;
pub(super) use sparse_conditional_constant_propagation::built_in_registrations as sparse_conditional_constant_propagation_rule_registrations;
pub(super) use state_specialization::built_in_registrations as state_specialization_rule_registrations;

/// The representation-specialization pass runs both registered families in
/// schedule order: case-membership folds (ordinal 0), then field-value folds
/// (ordinal 1). Both are published under the same public selection.
pub(super) fn representation_specialization_rule_registrations()
-> Vec<crate::rules::catalog::BuiltInRuleRegistration> {
    let mut registrations = case_membership_specialization_rule_registrations();
    registrations.extend(field_value_specialization_rule_registrations());
    registrations
}

#[allow(unused_imports)]
pub use case_membership_specialization::*;
pub use control_flow_cleanup::*;
pub use copy_propagation::*;
pub use dead_scalar_elimination::*;
#[allow(unused_imports)]
pub use field_value_specialization::*;
pub use global_value_numbering::*;
pub use proof_check_elision::*;
pub use sparse_conditional_constant_propagation::*;
#[allow(unused_imports)]
pub use state_specialization::*;

use support::{accepted_obligation_fact, boolean_constant, literal_integer_constant};

#[cfg(test)]
use support::node_elision_accounting;

#[cfg(test)]
pub(crate) use sparse_conditional_constant_propagation::range_comparisons::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind,
};

#[cfg(test)]
pub(crate) mod tests;
