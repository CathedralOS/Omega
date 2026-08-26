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

use psi_checked_trees::CheckedTrees;

/// Conservative pre-check classification used by compiler-run semantic
/// evaluation. `true` means the same typed fallback used during final authored
/// selection finalization can already prove this operator intrinsic; `false`
/// leaves it unresolved.
pub fn typed_operator_is_definitely_intrinsic(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    authored_selections::typed_operator_is_definitely_intrinsic(program, expression)
}

pub fn lower_typed_trees(
    program: psi_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lowerer::lower_typed_trees(program)
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
    generic_machine_template_fingerprint, refresh_closed_domain_instance_identities,
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
mod qualification_evidence;

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
