//! Typed trees to checked trees.
//!
//! Start at `checking.rs` for specialization, validation, and plan construction;
//! it also owns the open-index normalization and static machine-call
//! specialization steps that package orchestration reuses on typed snapshots.
//! `execution::selected_execution` owns rebuilding plans after provider
//! settlement; `execution::finalize_execution` owns initial plan completion.
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
mod execution {
    #[path = "control_cleanup.rs"]
    pub(crate) mod terminal_cleanup;
    #[path = "debug_metadata.rs"]
    pub(crate) mod terminal_debug;
    #[path = "scalar/mod.rs"]
    pub(crate) mod terminal_scalar;
    #[path = "unit/mod.rs"]
    pub(crate) mod terminal_unit;
    #[cfg(test)]
    pub(crate) fn exact_two_field_record_projection_for_test(
        program: &typed_trees::TypedTrees,
        root_type: typed_trees::types::TypeReferenceHandle,
        moved_field: symbols::SymbolHandle,
        target_type: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(String, String, String, String)> {
        terminal_unit::exact_two_field_record_projection(
            program,
            root_type,
            moved_field,
            target_type,
        )
    }
    pub(crate) mod execution_plans;
    pub(crate) mod finalize_execution;
    pub(crate) mod selected_execution;
}
mod facts;
mod labels;
mod lookup;
mod monomorphization;
mod operators;
mod package_review;
mod product_pruning;
mod values;

pub use checking::{
    SelectedBoundaryFamilySpecialization, SelectedGenericOperatorProviderSpecialization,
    lower_package_typed_trees_with_selected_generic_operator_providers,
    lower_preliminary_typed_trees, lower_typed_trees,
    lower_typed_trees_with_selected_generic_operator_providers, normalize_open_index_identities,
    specialize_static_machine_calls,
};
pub use execution::selected_execution::{
    SelectedIeeeFloatFmaUnitApplication, SelectedOperatorApplication,
    rebuild_checked_terminal_plans_with_selected_execution, refresh_settled_state_write_frames,
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
    recompute_machine_specialization_commitment, refresh_closed_domain_instance_identities,
};

mod semantic;
mod semantic_calls;
mod semantic_places;

pub use checking::lower_typed_trees as lower_typed_program;

mod proof;
pub use proof::{
    CheckedContractEntailmentAssumptionDischargeRecheckError,
    recheck_contract_entailment_assumption_discharge,
};

mod borrow;
mod flow;

#[cfg(test)]
mod tests;
