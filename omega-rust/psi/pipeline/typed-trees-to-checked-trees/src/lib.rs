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
//! concern. This root preserves the crate API and wires the subsystems.

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
mod product_pruning;
mod values;

use checked_trees::{CheckFacts, CheckedSemanticDependencies};
use typed_trees::TypedTrees;

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

/// Conservative pre-check classification used by compiler-run semantic
/// evaluation. `true` means the typed expression cannot select an authored
/// operator declaration; final checking still validates builtin semantics.
pub fn typed_operator_has_no_authored_selection(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    authored_selections::typed_operator_has_no_authored_selection(program, expression)
}

/// Exact declaration selected by one late-bound member access whose typed
/// `member_symbol` is still invalid, derived from the receiver's owner type
/// exactly as checked binding derives it. Build-time authority confines that
/// declaration's package instead of every same-spelled member in the program.
pub fn late_bound_member_declaration_from_exact_owner(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    authored_selections::exact_owner_member_declaration(program, expression)
}

/// Conservative declaration candidates for an operator before checked
/// selection is final. Build-time authority uses this set only to prove that
/// every possible authored meaning is already within the package's admitted
/// source graph; ordinary checked lowering still chooses the exact meaning.
pub fn typed_operator_authored_selection_candidates(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<symbols::SymbolHandle> {
    authored_selections::typed_operator_authored_selection_candidates(program, expression)
}

/// Independently rederive the exact visible boundary requirement selected by
/// one normalized `min`, `max`, or `sqrt` builtin call: the tokenless
/// `F32::`/`F64::` boundary operator, or the top-level `boundary requirement`
/// machine when core spells the slot that way. This compiler-private seam
/// lets selected execution reject drift without trusting the checked fact it
/// is validating.
pub fn resolve_checked_builtin_float_operator_requirement(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Option<symbols::SymbolHandle> {
    operators::resolve_builtin_float_operator_requirement(program, expression, origin)
}

/// Derive declaration-level operator selections on the current typed graph.
/// This reuses the checked value-origin traversal, but performs no flow proof
/// or whole-program validation. Consumers must reject unresolved selections;
/// ordinary checked lowering independently reconstructs these facts later.
pub fn derive_pre_flow_operator_selections(
    program: &typed_trees::TypedTrees,
) -> checked_trees::CheckedOperatorFacts {
    let values = derive_pre_flow_value_origins(program);
    operators::build_operator_facts(program, &values)
}

/// Reuse the current checked value traversal without claiming proof validity.
/// Exact expression origins also identify cast-owned constant type positions.
pub fn derive_pre_flow_value_origins(
    program: &typed_trees::TypedTrees,
) -> checked_trees::CheckedValueFacts {
    let proof_plan = ::proof::obligations::build_proof_plan(program);
    values::build_value_facts(program, &proof_plan)
}

/// Rederive the complete, canonically ordered checked semantic-dependency
/// table from the final typed program and its checked facts.
///
/// This is the compiler-internal package-review entry point. It does not trust
/// or consult the semantic-dependency table already retained in `facts`.
pub fn derive_checked_semantic_dependencies(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedSemanticDependencies {
    flow::derive_checked_semantic_dependencies(program, facts)
}

/// Rederive one complete checked crash-contract row for every root and
/// domain-homed operator declaration. This compiler-internal package-review
/// entry point retains exact typed proof-fact joins without creating a public
/// IR format.
pub fn derive_checked_operator_crash_contracts(
    program: &TypedTrees,
) -> Vec<checked_trees::CheckedOperatorCrashContract> {
    operators::derive_checked_operator_crash_contracts(program)
}

/// Rederive canonical authored crash buckets for an exact typed machine.
/// Scalar lowering metadata and private sites are deliberately outside this
/// read-only package-policy join; canonical predicate equality is unchanged.
pub fn derive_authored_machine_crash_buckets(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<checked_trees::CrashRouteBucket> {
    facts::derive_authored_machine_crash_buckets(program, machine)
}

/// Exact authored crash identity for a retained structural signature, using
/// the same canonical owner as its checked contract capsule.
pub fn derive_authored_signature_crash_buckets(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<checked_trees::CrashRouteBucket> {
    facts::derive_authored_signature_crash_buckets(program, signature)
}

/// Query conservative causes from the checker's closed local crash summary.
/// `None` means no complete summary, not a crash-free machine. This read-only
/// query exposes no private guard, site, call, or proof coordinates.
pub fn infer_checked_machine_crash_causes(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
) -> Option<Vec<checked_trees::CrashCause>> {
    facts::infer_checked_machine_crash_causes(program, facts, machine)
}

/// Query all closed local crash cause summaries with one shared analysis.
/// Missing machine keys mean unknown, not complete empty summaries. Each row
/// retains only its exact machine key and conservative canonical cause set.
pub fn infer_checked_crash_causes(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<(symbols::SymbolHandle, Vec<checked_trees::CrashCause>)> {
    facts::infer_checked_crash_causes(program, facts)
}

/// Rederive every checked machine-to-operator realization together with the
/// complete canonical contracts on both sides. Package review compares this
/// against the retained checked baseline before publishing the selected
/// operator.
pub fn derive_checked_operator_realization_contracts(
    program: &TypedTrees,
) -> Vec<checked_trees::CheckedOperatorRealizationContract> {
    operators::derive_checked_operator_realization_contracts(program)
}

/// Rederive one exact compiler-owned collection view from the final typed
/// program and the checked environments that own its expression. Package
/// review uses this compiler-internal seam to reject a retained intrinsic fact
/// that no longer agrees with its receiver and call shape.
pub fn derive_checked_collection_view_intrinsic(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    authored_selections::derive_checked_collection_view_intrinsic(program, facts, expression)
}

/// Rederive one nominal call target from the final typed program and the exact
/// checked owner environments that contain the expression. Package review uses
/// this compiler-internal seam for proof-owned attached and path-qualified
/// calls whose typed node intentionally carries no direct target symbol.
pub fn derive_checked_nominal_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    authored_selections::derive_checked_nominal_call_target(program, facts, expression)
}

/// Rederive the erased requires lane for one exact proof-expression call
/// target from checked contract facts. This is a compiler-internal package
/// review seam; handles remain private joins and never enter review identity.
pub fn derive_checked_contract_expression_evidence_parameters(
    facts: &CheckFacts,
    target_machine_symbol: symbols::SymbolHandle,
    target_state_symbol: symbols::SymbolHandle,
) -> Vec<arena::Handle<checked_trees::CheckedEvidenceTerm>> {
    checks::contracts::exact_target_evidence_parameters(
        facts,
        target_machine_symbol,
        target_state_symbol,
    )
}

/// Freshly instantiate one erased requires proposition against the current
/// typed call arguments. Package review uses this after checking to reject a
/// coordinated typed-tree edit paired with stale checked evidence custody.
pub fn derive_checked_contract_expression_evidence_instantiation(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    target_state_symbol: symbols::SymbolHandle,
    parameter: arena::Handle<checked_trees::CheckedEvidenceTerm>,
) -> Option<checked_trees::CheckedPropositionApplication> {
    checks::contracts::instantiate_contract_expression_evidence_parameter(
        program,
        facts,
        expression,
        target_state_symbol,
        parameter,
    )
}

pub use product_pruning::{
    CheckedTreeProductPruning, CheckedTreeProductPruningOutcome, CheckedTreeProductRoots,
    CheckedTreeProductRootsError, CheckedTreeProductSelection, CheckedTreeProductSelectionIdentity,
    prune_checked_tree_product,
};

/// Derive the checked body-local termination summary for one typed machine.
///
/// Constant and plan positions must run before checked lowering because their
/// values refine the typed program. This exposes the checker's pure judgment
/// for those admission sites while keeping the proof implementation single-
/// sourced with the facts produced by [`lower_typed_trees`].
pub fn infer_machine_termination_summary(
    program: &typed_trees::TypedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Option<language_semantics::TerminationGuarantee> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    Some(checks::termination::infer_machine_checked_summary(
        program, machine,
    ))
}

/// The v0 asm-intrinsic discharge gate (asm requires a freestanding boundary
/// root) -- re-exported for the ORCHESTRATION layer, which owns the
/// BuildConfig fact the gate consumes; the other validations run inside
/// `lower_typed_trees` and never see build.omg. The live call site is the
/// typed->checked settlement transition in
/// `assembled-syntax-to-checked-compilation` (`checking/phase_transitions.rs`),
/// which receives the evaluated `Build.freestanding` through
/// `TypedToCheckedSettlementInput`.
pub use ::validation::{data_requires_establishment, validate_asm_discharge};
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

/// Read the exact authored source span retained when checked call identity was
/// still joined to its typed owner. Compiler-generated calls contribute no
/// authored location.
///
/// This is a compiler-internal package-review seam, not a public IR contract.
pub fn derive_checked_body_call_source_spans(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine_symbol: symbols::SymbolHandle,
) -> Result<Vec<source::SourceSpan>, Vec<diagnostics::Diagnostic>> {
    facts::review_sources::derive_checked_body_call_source_spans(program, facts, machine_symbol)
}

mod borrow;
mod flow;

#[cfg(test)]
mod tests;
