//! Typed trees to checked trees.
//!
//! One entrance: [`lower_typed_trees`], in `checking.rs`, which takes a
//! [`CheckingRequest`] naming the package checkpoint and the settled
//! selections. Read that function to read this stage — the module order
//! below is the order it runs them in.
//!
//! Wire codec and compatibility admission runs at the entrance for every
//! request, including memo reuse; Build supplies demands, not their verdict.
//! It prepares the typed program, validates it, builds the facts a checked
//! machine publishes, analyses its flow and values, and plans its execution:
//!
//! 1. `proof` elaborates mathematical declarations and signatures.
//! 2. `lookup` resolves projected receiver calls to their exact owners.
//! 3. `authored_selections` binds what the source selected before
//!    specialization can rewrite it.
//! 4. `monomorphization` specializes generic and provider templates, then
//!    binds each specialization's contract identity.
//! 5. `operators` binds token-bound machine calls.
//! 6. `checking::program_validation` rejects a typed program that cannot be
//!    checked at all.
//! 7. `facts` and `semantic` build the published knowledge: what is known,
//!    the call coordinates it is known about, and the places it names.
//! 8. `flow`, `values` and `borrow` analyse temporal flow, value origins and
//!    borrow custody over those facts.
//! 9. `checks` runs the checking rules and records their evidence.
//! 10. `execution` completes the plans a checked machine executes.
//!
//! Beside the route, `conformance` closes trait applications, `labels` names
//! things for diagnostics and for the operator labels that facts retain and
//! later checks compare, and `product_pruning` and `package_review` are
//! orchestration seams rather than steps: the compiler calls them after a
//! checked tree exists, to prune a product and to rederive retained facts it
//! must be able to reject when they drift.

// The route, in the order `lower_typed_trees` runs it.
mod authored_selections;
mod borrow;
mod checking;
mod checks;
mod execution;
mod facts;
mod flow;
mod lookup;
mod monomorphization;
mod operators;
mod proof;
mod semantic;
mod values;
mod wire_compatibility;

// Beside the route.
mod conformance;
mod labels;

// Orchestration seams, called after a checked tree exists.
mod build_program;
mod package_review;
mod product_pruning;

pub use build_program::{project_build_program, project_selected_program};

pub use checking::{
    CheckingRequest, lower_typed_trees, normalize_open_index_identities,
    specialize_static_machine_calls,
};

pub use package_review::{
    derive_authored_machine_crash_buckets, derive_authored_signature_crash_buckets,
    derive_checked_collection_view_intrinsic,
    derive_checked_contract_expression_evidence_instantiation,
    derive_checked_contract_expression_evidence_parameters, derive_checked_nominal_call_target,
    derive_checked_operator_crash_contracts, derive_checked_operator_realization_contracts,
    derive_checked_semantic_dependencies, derive_pre_flow_operator_selections,
    derive_pre_flow_value_origins, infer_checked_crash_causes, infer_checked_machine_crash_causes,
    infer_machine_termination_summary, late_bound_member_declaration_from_exact_owner,
    typed_build_declaration_operand, typed_build_declaration_selection, typed_build_operation,
    typed_build_provider_selection, typed_operator_authored_selection_candidates,
    typed_operator_has_no_authored_selection, typed_product_provider_selection_operand,
    typed_provider_selection_expressions,
};

pub use product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product, prune_target_siblings, select_checked_tree_product,
    target_sibling_selection_occurrences,
};

/// Build inputs to Psi's instruction-authority check. The checking entrance
/// consumes these; the compiler does not run an additional validation pass.
pub use ::validation::{AsmAuthorityAdmission, data_requires_establishment};
pub use conformance::conformance_applications::close_conformance_application;
pub use monomorphization::{
    generic_machine_template_commitment, generic_machine_template_report_fingerprint,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.
