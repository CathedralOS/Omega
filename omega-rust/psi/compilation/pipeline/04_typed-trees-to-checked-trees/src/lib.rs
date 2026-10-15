//! Typed trees to checked trees.
//!
//! Start at checking.rs: lower_typed_trees consumes FrozenTypedTrees from
//! typed-trees-to-typed-trees and retained preparation judgments. Checking
//! validates source semantics, publishes CheckFacts and execution plans, and
//! never rewrites the typed representation. Invocation-owned declaration
//! authority remains fresh even when an exact-input language judgment is reused.
//!
//! Source elaboration and specialization belong to the preceding preparation
//! stage. Independent validation and proof packages own shared semantic queries;
//! facts, flow, values, borrow, checks and execution build the checked result.

// The route, in the order `lower_typed_trees` runs it.
pub(crate) use optimization::digest_encoder;
mod authored_selections;
mod borrow;
mod checking;
mod checks;
mod execution;
mod facts;
mod flow;
#[cfg(test)]
mod immutable_checking_input;
mod operators;
mod proof;
mod quotient_eligibility;
mod semantic;
mod values;

// Beside the route.
mod labels;

// Orchestration seams, called after a checked tree exists.
mod package_review;
mod product_pruning;

pub use checking::{
    CheckingOutcome, CheckingRequest, RetainedCheck, lower_typed_trees, lower_typed_trees_reusing,
    reuse_retained_check,
};
pub use package_review::{
    derive_authored_machine_crash_buckets, derive_authored_signature_crash_buckets,
    derive_checked_contract_expression_evidence_instantiation,
    derive_checked_contract_expression_evidence_parameters, derive_checked_nominal_call_target,
    derive_checked_semantic_dependencies, derive_pre_flow_value_origins,
    infer_checked_crash_causes, infer_checked_machine_crash_causes,
    infer_machine_termination_summary, late_bound_member_declaration_from_exact_owner,
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};

pub use product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product, prune_target_siblings, select_checked_tree_product,
};

/// Build inputs to Psi's instruction-authority check. The checking entrance
/// consumes these; the compiler does not run an additional validation pass.
pub use ::validation::{AsmAuthorityAdmission, data_requires_establishment};
