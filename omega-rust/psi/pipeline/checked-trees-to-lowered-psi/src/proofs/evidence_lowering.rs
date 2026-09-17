//! Proposition and evidence artifact lowering.
//!
//! This file installs the artifacts and names checked evidence identities.
//! `guarded_call_evidence.rs` lowers payloadless guarded-call evidence and
//! outcome-specific ensures, `evidence_terms.rs` lowers the proposition
//! vocabulary, evidence terms, interfaces and contract lanes,
//! `proof_output_calls.rs` lowers proof-output calls and runtime calls and
//! `producer_provenance.rs` lowers evidence producer provenance.

mod evidence_terms;
mod guarded_call_evidence;
mod producer_provenance;
mod proof_output_calls;

use super::{
    CheckedTrees, EvidenceRoute, LoweredPsi, LoweringError, ObligationEvidence, PrimitiveJudgment,
    Proposition, unsupported,
};
use crate::proofs::evidence_lowering::evidence_terms::{
    lower_evidence_contract_lanes, lower_evidence_term_ids, lower_evidence_terms,
    lower_proposition_vocabulary,
};
use crate::proofs::evidence_lowering::guarded_call_evidence::{
    exact_payloadless_return_guard, lower_and_install_payloadless_guarded_call_evidence,
    lower_outcome_specific_ensures,
};
use crate::proofs::evidence_lowering::producer_provenance::{
    hex_bytes, lower_evidence_producer_provenance,
};
use crate::proofs::evidence_lowering::proof_output_calls::lower_proof_output_calls;

pub(crate) fn lower_and_install_evidence_artifacts(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(machine)
    {
        return lower_and_install_payloadless_guarded_call_evidence(checked, plan, lowered);
    }
    let evidence_term_ids = lower_evidence_term_ids(checked, machine)?;
    let (declarations, applications, declaration_ids) =
        lower_proposition_vocabulary(checked, &evidence_term_ids.term_ids)?;
    let evidence_terms = lower_evidence_terms(
        checked,
        machine,
        &declaration_ids,
        &applications,
        evidence_term_ids.term_ids,
    )?;
    let outcome_specific_ensures = lower_outcome_specific_ensures(
        checked,
        machine,
        lowered.semantic_module.entry,
        &lowered.semantic_module,
        &evidence_terms.term_ids,
        &evidence_terms.declarations,
    )?;
    let evidence_contract_lanes = lower_evidence_contract_lanes(
        checked,
        machine,
        lowered.semantic_module.entry,
        &evidence_terms.term_ids,
    )?;
    let proof_output_calls = lower_proof_output_calls(
        checked,
        machine,
        lowered.semantic_module.entry,
        &lowered.semantic_module,
        &evidence_terms.term_ids,
        &declarations,
        &applications,
    )?;
    let evidence_producers =
        lower_evidence_producer_provenance(checked, machine, &evidence_terms.term_ids)?;

    lowered.proof_bundle.evidence_producers = evidence_producers;
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
    lowered.semantic_module.proof_output_calls = proof_output_calls;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "selected terminal machine is absent while installing guarded guarantees",
        ))?;
    entry.contract.outcome_specific_ensures = outcome_specific_ensures;
    for row in &entry.contract.outcome_specific_ensures {
        if row.evidence.is_none()
            && row.proposition == Proposition::Truth
            && exact_payloadless_return_guard(entry) == Some(row.guard)
        {
            lowered.proof_bundle.evidence.push(ObligationEvidence {
                obligation: row.obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            });
        }
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    Ok(())
}

pub(crate) fn checked_evidence_requirement_identity(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    Ok(exact_trait_requirement(checked, declaring_trait, requirement)?.overload_identity)
}

/// The canonical family tuple one closed conformance table row carries for
/// its requirement; dispatch rows copy the tuple from the row they select.
///
/// A Terminal table or dispatch row names `(declaring trait, complete
/// requirement overload, canonical value tuple)`. The local dynamic surface
/// admits only requirements without local generic binders
/// (`DynamicSignatureIneligibility::RequirementLocalGenerics`) and the static
/// requirement dispatch carries no tuple coordinate, so the only tuple this
/// lowering can retain is the empty one. A requirement that declares binders
/// has no Terminal tuple producer yet and rejects here rather than lowering
/// as an unbound row that merely looks nongeneric.
pub(crate) fn checked_requirement_family_tuple(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<Vec<String>, LoweringError> {
    if exact_trait_requirement(checked, declaring_trait, requirement)?
        .declares_local_generic_binders
    {
        return unsupported(
            "conformance requirement declares requirement-local generic binders without a \
             family tuple producer",
        );
    }
    Ok(Vec::new())
}

/// The exact typed requirement one `(declaring trait, requirement)` symbol
/// pair names, projected to the facts evidence and dynamic rows consume.
struct ExactTraitRequirement {
    /// Canonical normalized overload identity; never empty.
    overload_identity: String,
    /// Whether the signature declares requirement-local generic binders.
    declares_local_generic_binders: bool,
}

fn exact_trait_requirement(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<ExactTraitRequirement, LoweringError> {
    let mut matches = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == declaring_trait)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let (definition, signature) = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact trait requirement",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous trait requirement");
    }
    let overload_identity = checked
        .typed
        .normalized_trait_requirement_overload_identity(definition, signature)
        .identity();
    if overload_identity.is_empty() {
        return unsupported("evidence producer row has an empty requirement identity");
    }
    Ok(ExactTraitRequirement {
        overload_identity,
        declares_local_generic_binders: !checked
            .typed
            .state_signature_type_parameters(signature)
            .is_empty(),
    })
}

pub(crate) fn checked_evidence_machine_identity(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let machine = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact realization machine",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous realization machine");
    }
    let mut identity = checked
        .typed
        .normalized_machine_overload_identity(machine)
        .ok_or(LoweringError::Unsupported(
            "evidence producer realization has no callable identity",
        ))?
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer realization has an empty machine identity");
    }
    let mut specializations = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == machine.symbol);
    if let Some(specialization) = specializations.next() {
        if specializations.next().is_some() {
            return unsupported("evidence machine has ambiguous generic application identity");
        }
        let replayed_commitment = validation::recompute_checked_machine_specialization_commitment(
            checked,
            machine.symbol,
        )
        .map_err(|_| {
            LoweringError::Unsupported(
                "evidence machine specialization commitment could not be replayed",
            )
        })?;
        if specialization.commitment.is_zero()
            || specialization.commitment.as_bytes() != replayed_commitment
        {
            return unsupported("evidence machine specialization commitment does not replay");
        }
        identity = format!(
            "specialized-machine|callable={}:{}|application={}",
            identity.len(),
            identity,
            hex_bytes(&specialization.commitment.as_bytes()),
        );
    }
    Ok(identity)
}
