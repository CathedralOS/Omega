//! Typed trees to checked trees.
//!
//! Start at `checking.rs` for specialization, validation, and plan construction;
//! it also owns the open-index normalization and static machine-call
//! specialization steps that package orchestration reuses on typed snapshots.
//! `selected_execution.rs` owns rebuilding plans after provider settlement.
//! This root preserves the crate API and wires the checking subsystems.

mod authored_selections;
mod call_acknowledgements;
mod capabilities;
mod checking;
mod checks;
mod conformance_application_lifetimes;
mod conformance_applications;
mod context;
mod contract_occurrences;
mod execution_plans;
mod facts;
mod field_domain;
mod labels;
mod lookup;
mod monomorphization;
mod operators;
mod product_pruning;
mod selected_execution;
mod validation;
mod values;

use checked_trees::{CheckFacts, CheckedSemanticDependencies};
use typed_trees::TypedTrees;

pub use checking::{
    SelectedGenericOperatorProviderSpecialization,
    lower_package_typed_trees_with_selected_generic_operator_providers,
    lower_preliminary_typed_trees, lower_typed_trees,
    lower_typed_trees_with_selected_generic_operator_providers, normalize_open_index_identities,
    specialize_static_machine_calls,
};
pub use selected_execution::{
    SelectedIeeeFloatFmaUnitApplication, SelectedOperatorApplication,
    rebuild_checked_terminal_plans_with_selected_execution,
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
/// one normalized `min`, `max`, or `sqrt` builtin call. This compiler-private
/// seam lets selected execution reject drift without trusting the checked fact
/// it is validating.
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

#[cfg(test)]
pub(crate) use checking::lower_typed_trees_for_crash_fact_inspection;

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
/// `lower_typed_trees` and never see build.omg.
pub use ::validation::{data_requires_establishment, validate_asm_discharge};
pub use conformance_applications::close_conformance_application;
pub use monomorphization::{
    generic_machine_template_commitment, generic_machine_template_report_fingerprint,
    recompute_machine_specialization_commitment, refresh_closed_domain_instance_identities,
};

mod semantic;
mod semantic_calls;
mod semantic_places;

#[cfg(test)]
pub(crate) use semantic::build_semantic_facts;
pub use semantic::lower_typed_program;
pub(crate) use semantic::{
    CallSite, call_site_argument_expressions, call_target_parameters, call_target_type_parameters,
    find_call_site, find_state, find_state_in_machine,
};
pub(crate) use semantic_calls::call_site_evidence_arguments;

mod proof;
pub use proof::{
    CheckedContractEntailmentAssumptionDischargeRecheckError,
    recheck_contract_entailment_assumption_discharge,
};
mod qualification_evidence;
mod review_sources;

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
    review_sources::derive_checked_body_call_source_spans(program, facts, machine_symbol)
}

#[cfg(test)]
pub(crate) use borrow::build_borrow_facts;
#[cfg(test)]
pub(crate) use flow::{build_domain_facts, build_flow_facts};
#[cfg(test)]
pub(crate) use operators::build_operator_facts;
#[cfg(test)]
pub(crate) use proof::build_proof_facts;
pub(crate) use proof::contract_target_from_state_symbol;
#[cfg(test)]
pub(crate) use values::build_value_facts;
mod borrow;
mod flow;

#[cfg(test)]
mod tests;
