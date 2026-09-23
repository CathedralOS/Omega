//! Typed trees to checked trees.
//!
//! One entrance: [`lower_typed_trees`], in `checking.rs`, which takes a
//! [`CheckingRequest`] naming the package checkpoint and the settled
//! selections. Read that function to read this stage — the module order
//! below is the order it runs them in.
//!
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
//! things for diagnostics, and `product_pruning` and `package_review` are
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

// Beside the route.
mod conformance;
mod labels;

// Orchestration seams, called after a checked tree exists.
mod package_review;
mod product_pruning;

pub use checking::{
    CheckingRequest, SelectedBoundaryFamilySpecialization,
    SelectedGenericOperatorProviderSpecialization, lower_typed_trees,
    normalize_open_index_identities, specialize_static_machine_calls,
};
// The borrow certificate ledgers published inside the checked fact arenas
// replay independently for any consumer of the published record: the
// producing pass keeps the checker itself public so post-publication
// evidence is checkable, not just inspectable.
pub use checks::replay_checked_borrow_certificates;
pub use execution::selected_execution::{
    ExecutionSettlement, SelectedIeeeFloatFmaUnitApplication, SelectedOperatorApplication,
    SettledCallSite, SettledFloatIntrinsic, SettledFloatIntrinsicExecution,
    SettledOperatorAdapterCall, SettledOperatorAdapterSource, SettledRequirementCall,
    settle_checked_execution,
};

pub use package_review::{
    derive_authored_machine_crash_buckets, derive_authored_signature_crash_buckets,
    derive_checked_body_call_source_spans, derive_checked_collection_view_intrinsic,
    derive_checked_contract_expression_evidence_instantiation,
    derive_checked_contract_expression_evidence_parameters, derive_checked_nominal_call_target,
    derive_checked_operator_crash_contracts, derive_checked_operator_realization_contracts,
    derive_checked_semantic_dependencies, derive_pre_flow_operator_selections,
    derive_pre_flow_value_origins, infer_checked_crash_causes, infer_checked_machine_crash_causes,
    infer_machine_termination_summary, late_bound_member_declaration_from_exact_owner,
    resolve_checked_builtin_float_operator_requirement, typed_build_provider_selection,
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
    typed_product_provider_selection_operand, typed_provider_selection_expressions,
};

pub use product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product,
};

/// The privileged-service asm-intrinsic discharge (each instruction's
/// authority class checks against the build's supplied
/// `AsmAuthorityAdmission` evidence) -- re-exported for the ORCHESTRATION
/// layer, which owns the BuildConfig fact the gate consumes; the other
/// validations run inside `lower_typed_trees` and never see build.omg. The
/// live call site is the typed->checked settlement transition in
/// `assembled-syntax-to-checked-compilation` (`checking/phase_transitions.rs`),
/// which derives the admission from the evaluated `Build.freestanding`
/// through `TypedToCheckedSettlementInput`.
pub use ::validation::{
    AsmAuthorityAdmission, data_requires_establishment, validate_asm_discharge,
};
pub use conformance::conformance_applications::close_conformance_application;
pub use monomorphization::{
    generic_machine_template_commitment, generic_machine_template_report_fingerprint,
};

pub use proof::{
    CheckedContractEntailmentAssumptionDischargeRecheckError,
    recheck_contract_entailment_assumption_discharge,
};

#[cfg(test)]
mod tests;
