//! Built-in Psi transformations, organized by their explicit pass identity.

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v5";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v13";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
const DEAD_PURE_SCALAR_PASS_NAME: &[u8] = b"omega.psi-pass.dead-pure-scalar-elimination.v2";
const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v12";
const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] = b"omega.psi-pass.global-value-numbering.v11";

mod support;

mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;

pub(super) use control_flow_cleanup::built_in_registrations as control_flow_cleanup_rule_registrations;
pub(super) use copy_propagation::built_in_registrations as copy_propagation_rule_registrations;
pub(super) use dead_scalar_elimination::built_in_registrations as dead_scalar_elimination_rule_registrations;
pub(super) use global_value_numbering::built_in_registrations as global_value_numbering_rule_registrations;
pub(super) use proof_check_elision::built_in_registrations as proof_check_elision_rule_registrations;
pub(super) use sparse_conditional_constant_propagation::built_in_registrations as sparse_conditional_constant_propagation_rule_registrations;

pub use control_flow_cleanup::*;
pub use copy_propagation::*;
pub use dead_scalar_elimination::*;
pub use global_value_numbering::*;
pub use proof_check_elision::*;
pub use sparse_conditional_constant_propagation::*;

use global_value_numbering::local_cse_accounting;
use support::{accepted_obligation_fact, boolean_constant, literal_integer_constant};

#[cfg(test)]
use sparse_conditional_constant_propagation::range_comparisons::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind, evaluate_integer_range_comparison,
    evaluate_integer_range_pair_comparison,
};

#[cfg(test)]
pub(crate) mod tests;
