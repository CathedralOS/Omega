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

pub(crate) use optimization::digest_encoder;

// The entry, and the guard that pins preparation's tables as read-only here.
mod checking;
#[cfg(test)]
mod immutable_checking_input;

// The steps. `lower_typed_trees` runs them as: facts are built, flow and
// values are judged, borrows and checks are settled, execution plans are
// assembled.
mod borrow;
mod checks;
mod execution;
mod flow;
mod values;

// What the stage publishes, the helpers every step reads, and the seams the
// product coordinator reaches once a checked tree exists.
mod products;
mod published;
mod shared;

pub use ::facts::declaration_page_fork_totals;
pub use checking::{
    CheckingOutcome, CheckingRequest, RetainedCheck, lower_typed_trees, lower_typed_trees_reusing,
    reuse_retained_check,
};
pub use products::package_review::{
    derive_authored_machine_crash_buckets, derive_authored_signature_crash_buckets,
    derive_checked_contract_expression_evidence_instantiation,
    derive_checked_contract_expression_evidence_parameters, derive_checked_nominal_call_target,
    derive_checked_semantic_dependencies, infer_checked_crash_causes,
    infer_checked_machine_crash_causes, infer_checked_recursive_machine_terminations,
    infer_machine_termination_summary, late_bound_member_declaration_from_exact_owner,
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};

pub use products::product_context::project_checked_product_context;
pub use products::product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product, select_checked_tree_product,
};

/// Build inputs to Psi's instruction-authority check. The checking entrance
/// consumes these; the compiler does not run an additional validation pass.
pub use ::validation::{AsmAuthorityAdmission, data_requires_establishment};
