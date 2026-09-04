mod authored_selections;
mod call_acknowledgements;
mod capabilities;
mod checks;
mod conformance_application_lifetimes;
mod conformance_applications;
mod context;
mod contract_occurrences;
mod facts;
mod field_domain;
mod labels;
mod lookup;
mod lowerer;
mod monomorphization;
mod operators;
mod validation;
mod values;

use psi_checked_trees::{CheckFacts, CheckedSemanticDependencies, CheckedTrees};
use psi_typed_trees::TypedTrees;

/// Conservative pre-check classification used by compiler-run semantic
/// evaluation. `true` means the typed expression cannot select an authored
/// operator declaration; final checking still validates builtin semantics.
pub fn typed_operator_has_no_authored_selection(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    authored_selections::typed_operator_has_no_authored_selection(program, expression)
}

/// Conservative declaration candidates for an operator before checked
/// selection is final. Build-time authority uses this set only to prove that
/// every possible authored meaning is already within the package's admitted
/// source graph; ordinary checked lowering still chooses the exact meaning.
pub fn typed_operator_authored_selection_candidates(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Vec<psi_symbols::SymbolHandle> {
    authored_selections::typed_operator_authored_selection_candidates(program, expression)
}

/// Independently rederive the exact visible boundary requirement selected by
/// one normalized `min`, `max`, or `sqrt` builtin call. This compiler-private
/// seam lets selected execution reject drift without trusting the checked fact
/// it is validating.
pub fn resolve_checked_builtin_float_operator_requirement(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    origin: psi_checked_trees::CheckedValueOrigin,
) -> Option<psi_symbols::SymbolHandle> {
    operators::resolve_builtin_float_operator_requirement(program, expression, origin)
}

pub fn lower_typed_trees(
    program: psi_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lowerer::lower_typed_trees(program, &[], &[])
}

/// Lower a pre-settlement package checkpoint. Unresolved selections are
/// retained only for compiler-owned toolchain source; the caller must reject
/// unresolved ordinary-package selections before granting build authority.
pub fn lower_preliminary_typed_trees(
    program: psi_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lowerer::lower_preliminary_typed_trees(program)
}

/// One Omega-selected generic checked body that must be specialized for the
/// exact closed applications of its boundary-operator requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedGenericOperatorProviderSpecialization {
    pub requirement_operator: psi_symbols::SymbolHandle,
    pub realization_machine: psi_symbols::SymbolHandle,
}

/// Lower with exact selected generic operator providers supplied by the
/// orchestration owner. Psi derives applications from authored uses and uses
/// ordinary authoritative specialization; the request carries no application
/// strings, capability assertions, or provider-selection policy.
pub fn lower_typed_trees_with_selected_generic_operator_providers(
    program: psi_typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[psi_validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lowerer::lower_typed_trees(program, selected, opaque_property_receipts)
}

/// Final package-aware lowering keeps ordinary package selections strict while
/// permitting unresolved compiler-owned toolchain selections to remain TCB
/// input. The compiler must run its package declaration-authority gate over
/// the result before issuing package evidence.
pub fn lower_package_typed_trees_with_selected_generic_operator_providers(
    program: psi_typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[psi_validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lowerer::lower_package_typed_trees(program, selected, opaque_property_receipts)
}

/// Exact compiler-owned join from one authored operator use to the checked
/// machine selected to realize it. Selected execution supplies these rows only
/// after ProviderPlan settlement; ordinary checking supplies none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOperatorApplication {
    pub expression: psi_typed_trees::expression::ExpressionHandle,
    pub origin: psi_checked_trees::CheckedValueOrigin,
    pub requirement_operator: psi_symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: psi_symbols::SymbolHandle,
    pub realization_state: psi_symbols::SymbolHandle,
    pub operands: Vec<psi_typed_trees::expression::ExpressionHandle>,
}

/// One compiler-intrinsic nearest IEEE FMA selected for an attached Unit
/// local initializer. This is intentionally disjoint from checked-body
/// operator adapters: no bodyless call or fabricated realization machine is
/// introduced into checked Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIeeeFloatFmaUnitApplication {
    pub expression: psi_typed_trees::expression::ExpressionHandle,
    pub origin: psi_checked_trees::CheckedValueOrigin,
    pub requirement_operator: psi_symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    pub format: psi_core::IeeeFloatFormat,
    pub operands: Vec<psi_typed_trees::expression::ExpressionHandle>,
}

/// Rebuild the bounded Unit-effect roster with exact selected boundary-
/// operator applications available during planning. This is a compiler-
/// internal phase seam, not a public checked-IR contract.
pub fn rebuild_checked_unit_effect_plans_with_selected_operators(
    program: &mut CheckedTrees,
    applications: &[SelectedOperatorApplication],
) {
    rebuild_checked_unit_effect_plans_with_selected_execution(program, applications, &[]);
}

/// Rebuild attached Unit plans once from the complete selected execution
/// roster. Keeping adapter calls and compiler-intrinsic scalar operations in
/// one transaction prevents either settlement pass from erasing the other.
pub fn rebuild_checked_unit_effect_plans_with_selected_execution(
    program: &mut CheckedTrees,
    operator_applications: &[SelectedOperatorApplication],
    ieee_float_fma_applications: &[SelectedIeeeFloatFmaUnitApplication],
) {
    program.facts.flow.terminal_unit_effects = flow::build_checked_unit_effect_plans(
        &program.typed,
        &program.facts,
        operator_applications,
        ieee_float_fma_applications,
    );
}

/// Rebuild every checked Terminal plan whose exact shape depends on selected
/// operator execution. Unit-local scalar calls and direct structural-scalar
/// returns are one transaction so neither selected family can erase the
/// other's custody.
pub fn rebuild_checked_terminal_plans_with_selected_execution(
    program: &mut CheckedTrees,
    operator_applications: &[SelectedOperatorApplication],
    ieee_float_fma_applications: &[SelectedIeeeFloatFmaUnitApplication],
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let terminal_unit_effects = flow::build_checked_unit_effect_plans(
        &program.typed,
        &program.facts,
        operator_applications,
        ieee_float_fma_applications,
    );
    let mut diagnostics = Vec::new();
    let structural_scalar_returns = flow::build_checked_structural_scalar_return_plans(
        &program.typed,
        &program.facts,
        &terminal_unit_effects,
        operator_applications,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    program.facts.flow.terminal_unit_effects = terminal_unit_effects;
    program.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(())
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
) -> Vec<psi_checked_trees::CheckedOperatorCrashContract> {
    operators::derive_checked_operator_crash_contracts(program)
}

/// Rederive every checked machine-to-operator realization together with the
/// complete canonical contracts on both sides. Package review compares this
/// against the retained checked baseline before publishing the selected
/// operator.
pub fn derive_checked_operator_realization_contracts(
    program: &TypedTrees,
) -> Vec<psi_checked_trees::CheckedOperatorRealizationContract> {
    operators::derive_checked_operator_realization_contracts(program)
}

/// Rederive one exact compiler-owned collection view from the final typed
/// program and the checked environments that own its expression. Package
/// review uses this compiler-internal seam to reject a retained intrinsic fact
/// that no longer agrees with its receiver and call shape.
pub fn derive_checked_collection_view_intrinsic(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    authored_selections::derive_checked_collection_view_intrinsic(program, facts, expression)
}

/// Rederive one nominal call target from the final typed program and the exact
/// checked owner environments that contain the expression. Package review uses
/// this compiler-internal seam for proof-owned attached and path-qualified
/// calls whose typed node intentionally carries no direct target symbol.
pub fn derive_checked_nominal_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_symbols::SymbolHandle> {
    authored_selections::derive_checked_nominal_call_target(program, facts, expression)
}

/// Rederive the erased requires lane for one exact proof-expression call
/// target from checked contract facts. This is a compiler-internal package
/// review seam; handles remain private joins and never enter review identity.
pub fn derive_checked_contract_expression_evidence_parameters(
    facts: &CheckFacts,
    target_machine_symbol: psi_symbols::SymbolHandle,
    target_state_symbol: psi_symbols::SymbolHandle,
) -> Vec<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>> {
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
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_state_symbol: psi_symbols::SymbolHandle,
    parameter: psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>,
) -> Option<psi_checked_trees::CheckedPropositionApplication> {
    checks::contracts::instantiate_contract_expression_evidence_parameter(
        program,
        facts,
        expression,
        target_state_symbol,
        parameter,
    )
}

#[cfg(test)]
pub(crate) use lowerer::lower_typed_trees_for_crash_fact_inspection;

/// Bind exact PDI3 operation/algebra authority and refresh every enclosing
/// indexed-domain semantic ID. Orchestration calls this before typed
/// snapshots and trust receipts; checked lowering calls it before capturing
/// generic template fingerprints and again after specialization.
pub fn normalize_open_index_identities(
    program: &mut psi_typed_trees::TypedTrees,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    psi_validation::normalize_open_index_expressions(program)?;
    monomorphization::refresh_closed_domain_instance_identities(program)
        .map_err(|diagnostic| vec![diagnostic])
}

/// Validate and consume compile-time machine-symbol selections, rewriting
/// every complete generic call tuple to direct concrete calls. The ordinary
/// checked-tree path invokes this before validation; orchestration also uses
/// it on a private clone before interpreting build.omg so build-time execution
/// sees the same specialized program as runtime lowering.
pub fn specialize_static_machine_calls(
    program: &mut psi_typed_trees::TypedTrees,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    specialize_static_machine_calls_with_nominal_uses(program).map(|_| ())
}

pub(crate) fn specialize_static_machine_calls_with_nominal_uses(
    program: &mut psi_typed_trees::TypedTrees,
) -> Result<Vec<psi_validation::ValidatedNominalMachineUse>, Vec<psi_diagnostics::Diagnostic>> {
    conformance_application_lifetimes::resolve_elided_conformance_lifetimes(program)?;
    conformance_applications::validate_conformance_applications(program)?;
    let mut nominal_uses = psi_validation::validate_static_machine_selections_with_facts(program)?;
    psi_validation::validate_generic_machine_contract_entailment(program)?;
    monomorphization::monomorphize_generic_machine_value_calls_with_nominal_uses(
        program,
        &mut nominal_uses,
    )?;
    Ok(nominal_uses)
}

/// Derive the checked body-local termination summary for one typed machine.
///
/// Constant and plan positions must run before checked lowering because their
/// values refine the typed program. This exposes the checker's pure judgment
/// for those admission sites while keeping the proof implementation single-
/// sourced with the facts produced by [`lower_typed_trees`].
pub fn infer_machine_termination_summary(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: psi_symbols::SymbolHandle,
) -> Option<psi_language_semantics::TerminationGuarantee> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    Some(checks::termination::infer_machine_checked_summary(
        program, machine,
    ))
}

pub use conformance_applications::close_conformance_application;
pub use monomorphization::{
    generic_machine_template_commitment, generic_machine_template_report_fingerprint,
    recompute_machine_specialization_commitment, refresh_closed_domain_instance_identities,
};
/// The v0 asm-intrinsic discharge gate (asm requires a freestanding boundary
/// root) -- re-exported for the ORCHESTRATION layer, which owns the
/// BuildConfig fact the gate consumes; the other validations run inside
/// `lower_typed_trees` and never see build.omg.
pub use psi_validation::{data_requires_establishment, validate_asm_discharge};

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
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    machine_symbol: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_source::SourceSpan>, Vec<psi_diagnostics::Diagnostic>> {
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
