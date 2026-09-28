#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Independent meaning and rewrite validation for the optimization-unit representation.
//! This crate neither lowers Terminal Psi nor sequences optimization passes or publication.
//! Candidate producers do not participate in acceptance. The caller retains
//! Terminal admission and supplies any independently admitted cycle roster;
//! structural success alone grants no execution or publication authority.
//!
//! The exports below are the whole public surface. Whole-unit acceptance
//! starts at `unit_validation.rs`: `validate_psi_optimization_unit` is the
//! entry, and the file shows the acceptance order -- identity and fact
//! indexes, catalogs with every function validated in place, retained affine
//! authority, final authorities -- with each invariant family owned by a
//! named module beside it. Rewrite acceptance starts at `candidates/mod.rs`:
//! `validate_psi_rewrite_candidate` routes each typed candidate to the exact
//! validator of the pass family that proposed it, and those family validators
//! are exported by name for callers that hold one family's candidate.
//! `current_ownership` reconstructs the ownership frontiers of the current
//! revision and `current_value_ranges` validates its value-range facts;
//! `error` is the failure vocabulary.

mod candidates;
mod current_ownership;
mod current_value_ranges;
mod error;
mod unit_validation;

pub use error::OptimizationUnitValidationError;

// Whole-unit acceptance.
pub use unit_validation::{
    validate_psi_optimization_unit, validate_psi_optimization_unit_with_admitted_cycle_machines,
};

// Rewrite-candidate acceptance: the typed dispatch and the result it returns.
pub use candidates::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

// The exact validators the dispatch routes to, by producing pass family.
pub use candidates::case_membership_specialization::validate_case_membership_specialization_candidate;
pub use candidates::control_flow_cleanup::{
    validate_adjacent_block_merge_candidate, validate_constant_conditional_candidate,
    validate_linear_empty_block_candidate, validate_non_adjacent_block_merge_candidate,
    validate_path_qualified_empty_block_candidate, validate_shared_jump_fusion_candidate,
    validate_unreachable_private_machines_candidate,
};
pub use candidates::copy_propagation::validate_redundant_block_parameter_candidate;
pub use candidates::dead_scalar_elimination::validate_dead_scalar_node_candidate;
pub use candidates::field_value_specialization::validate_field_value_specialization_candidate;
pub use candidates::global_value_numbering::{
    validate_dominating_scalar_common_subexpression_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
    validate_total_scalar_identity_candidate,
};
pub use candidates::proof_check_elision::{
    validate_proof_certified_exact_integer_self_subtract_candidate,
    validate_proof_certified_integer_remainder_by_one_candidate,
    validate_proof_certified_integer_self_divide_candidate,
    validate_proof_certified_integer_self_remainder_candidate,
    validate_proof_certified_scalar_identity_candidate,
    validate_proof_certified_signed_integer_remainder_by_negative_one_candidate,
};
pub use candidates::sparse_conditional_constant_propagation::{
    validate_boolean_evaluation_candidate, validate_integer_evaluation_candidate,
};
pub use candidates::state_specialization::validate_state_argument_specialization_candidate;

// The closed scalar observation boundary scalar rewrites are checked against.
pub use candidates::observation::{
    ClosedScalarObservationBoundary, reconstruct_closed_scalar_node_boundary,
};

// Current-revision value-range facts.
pub use current_value_ranges::{
    validate_current_value_range_fact, validate_current_value_range_fact_at,
};

#[cfg(test)]
mod tests;
