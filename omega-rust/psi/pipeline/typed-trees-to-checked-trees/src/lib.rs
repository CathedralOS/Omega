//! Typed trees to checked trees.
//!
//! Start at `checking.rs`: `lower_typed_trees` is the crate's one lowering
//! entrance, and a `CheckingRequest` names the package checkpoint and carries
//! the settled selections. It also owns the open-index normalization and
//! static machine-call specialization steps that package orchestration reuses
//! on typed snapshots.
//! `execution::selected_execution` owns `settle_checked_execution`, the
//! checked->checked settlement link that applies a provider settlement;
//! `execution::finalize_execution` owns initial plan completion.
//! Executable builders live under execution, separately from temporal flow.
//! Fact population lives in `facts`, flow and value analysis in
//! `flow` and `values`, and the remaining folders each own one checking
//! concern. `package_review` carries the compiler-internal rederivation seams
//! that let orchestration reject drifted retained facts. This root preserves
//! the crate API and wires the subsystems.

mod authored_selections;
mod checking;
mod checks;
mod conformance;
mod execution;
mod facts;
mod labels;
mod lookup;
mod monomorphization;
mod operators;
mod package_review;
mod product_pruning;
mod values;

pub use checking::{
    CheckingRequest, SelectedBoundaryFamilySpecialization,
    SelectedGenericOperatorProviderSpecialization, lower_typed_trees,
    normalize_open_index_identities, specialize_static_machine_calls,
};
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

mod semantic;
mod semantic_calls;
mod semantic_places;

mod proof;
pub use proof::{
    CheckedContractEntailmentAssumptionDischargeRecheckError,
    recheck_contract_entailment_assumption_discharge,
};

mod borrow;
mod flow;

#[cfg(test)]
mod tests;
