use super::ContractProjectionContext;
use crate::evidence::{
    PackageReviewContractFact, PackageReviewContractKind, PackageReviewPropositionEvidence,
};
use crate::projection::contracts::portable_parameter_position;
use crate::projection::exact_identity::{nominal_identity, trait_requirement_identity};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn checked_outcome_specific_guarantee<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    result_data: SymbolHandle,
    result_case: SymbolHandle,
    binding: Option<&psi_typed_trees::name::Identifier>,
) -> Result<&'a psi_checked_trees::OutcomeSpecificGuaranteeFact, Vec<Diagnostic>> {
    let psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol } = context.owner
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` publishes an outcome-specific guarantee without a checked machine owner",
            context.subject_kind, context.subject_name
        ))]);
    };
    let public_selector = binding.map(|binding| binding.as_str());
    let matching = compilation
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, checked)| {
            (checked.machine_symbol == machine_symbol
                && checked.fact == fact
                && checked.result_data == result_data
                && checked.result_case == result_case
                && checked.public_selector.as_deref() == public_selector)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` outcome-specific guarantee has {} exact checked carrier rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
}

pub(crate) fn checked_contract_fact<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    kind: PackageReviewContractKind,
) -> Result<&'a psi_checked_trees::ContractProofFact, Vec<Diagnostic>> {
    let checked_kind = match kind {
        PackageReviewContractKind::Requires => psi_checked_trees::ContractProofFactKind::Requires,
        PackageReviewContractKind::Ensures => psi_checked_trees::ContractProofFactKind::Ensures,
    };
    let matching = compilation
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, checked)| {
            (checked.fact == fact && checked.kind == checked_kind && checked.owner == context.owner)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract fact has {} checked owner rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
}

pub(crate) fn validate_checked_contract_evidence(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked: &psi_checked_trees::ContractProofFact,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    validate_checked_contract_evidence_components(
        compilation,
        context,
        binding,
        checked.owner,
        checked.kind,
        checked.evidence_term,
        projected,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_checked_contract_evidence_components(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked_owner: psi_checked_trees::ContractProofFactOwner,
    checked_kind: psi_checked_trees::ContractProofFactKind,
    checked_evidence_term: Option<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>>,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    let Some(binding) = binding else {
        if checked_evidence_term.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an unnamed contract with a checked evidence term",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(None);
    };
    let Some(term_handle) = checked_evidence_term else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let term = compilation.facts.proof.evidence_terms.get(term_handle);
    if term.name != binding.as_str() || term.owner != checked_owner || term.kind != checked_kind {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not match its checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    if matches!(
        projected,
        PackageReviewContractFact::PropositionParameter(_)
    ) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` uses a generic proposition endpoint without an exact checked witness interface",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewContractFact::Proposition(application) = projected else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` is not a proposition",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, term.proposition.declaration)? != application.declaration {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed proposition endpoint during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewPropositionEvidence::Witness(interface) = &application.evidence else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not expose witness evidence",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no exact checked witness interface",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness trait during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let mut checked_requirements = checked_interface
        .requirements
        .iter()
        .map(|requirement| {
            let owner = compilation
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == requirement.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact declaring trait",
                    )]
                })?;
            let signature = compilation
                .trait_machine_signatures(owner)
                .iter()
                .find(|candidate| candidate.symbol == requirement.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact overload declaration",
                    )]
                })?;
            Ok((
                nominal_identity(compilation, requirement.declaring_trait)?,
                trait_requirement_identity(compilation, owner, signature)?,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_requirements.sort();
    let mut projected_requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            (
                requirement.declaring_trait.clone(),
                requirement.requirement.clone(),
            )
        })
        .collect::<Vec<_>>();
    projected_requirements.sort();
    if checked_requirements != projected_requirements {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness requirements during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    portable_parameter_position(term.lane_position).map(Some)
}
