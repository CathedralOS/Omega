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
#[cfg(test)]
mod tests;

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
/// requirement overload, canonical value tuple)`. Lanes that can express only
/// one row per requirement — the dynamic-composed-Unit application surfaces —
/// call this source and still reject a requirement that declares local
/// binders rather than lowering an empty-tuple row that merely looks
/// nongeneric. Lanes that carry tuple coordinates expand a checked row
/// through [`checked_requirement_family_rows`] instead.
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

/// One lowered conformance-table row a checked requirement row contributes:
/// the canonical value tuple and the exact realization that tuple selects.
pub(crate) struct CheckedRequirementFamilyRow {
    /// Canonical const identities in the requirement's const/value binder
    /// declaration order; empty for a nongeneric requirement.
    pub family_tuple: Vec<String>,
    /// The machine the row names: the row's own provider for a nongeneric
    /// requirement, or the tuple's bare value-tuple specialization instance.
    pub realization_machine: symbols::SymbolHandle,
    /// The state at the row realization state's declaration position on the
    /// realized machine.
    pub realization_state: symbols::SymbolHandle,
}

/// The terminal table rows one checked conformance row lowers to: one row for
/// a nongeneric requirement, or one row per declared roster tuple for a finite
/// generic family. Each family row names the unique bare value-tuple
/// specialization of the row's provider template that the tuple's canonical
/// const identities select — the same coordinate
/// `dynamic_family_realization` joins on the checked side. A tuple with no
/// retained specialization, or with several, rejects the row rather than
/// lowering a partial or ambiguous family.
pub(crate) fn checked_requirement_family_rows(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    realization_machine: symbols::SymbolHandle,
    realization_state: symbols::SymbolHandle,
) -> Result<Vec<CheckedRequirementFamilyRow>, LoweringError> {
    let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
    // A requirement with no local binders keeps exactly one empty-tuple row
    // even if it carries a trivially satisfiable `where` clause.
    if !exact.declares_local_generic_binders {
        return Ok(vec![CheckedRequirementFamilyRow {
            family_tuple: Vec::new(),
            realization_machine,
            realization_state,
        }]);
    }
    let tuples = match checked.typed.finite_signature_family(exact.signature) {
        checked_trees::finite_family::FamilyProbe::Finite { tuples, .. } => tuples,
        checked_trees::finite_family::FamilyProbe::NotFinite(_) => {
            return unsupported(
                "conformance requirement declares requirement-local generic binders without a \
                 finite family roster",
            );
        }
    };
    let template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == realization_machine)
        .ok_or(LoweringError::Unsupported(
            "conformance family row has no exact realization template machine",
        ))?;
    let state_position = checked
        .typed
        .machine_states(template)
        .iter()
        .position(|state| state.symbol == realization_state)
        .ok_or(LoweringError::Unsupported(
            "conformance family row has no exact realization template state",
        ))?;
    tuples
        .iter()
        .map(|tuple| {
            let mut specializations =
                checked
                    .typed
                    .machine_specializations
                    .iter()
                    .filter(|specialization| {
                        specialization.template == realization_machine
                            && specialization.const_argument_identities.as_slice()
                                == tuple.identities.as_ref()
                            && specialization.type_argument_identities.is_empty()
                            && specialization.machine_arguments.is_empty()
                            && specialization.conformance_arguments.is_empty()
                            && specialization.inferred_conformance_arguments.is_empty()
                            && specialization.conformance_applications.is_empty()
                    });
            let Some(specialization) = specializations.next() else {
                return unsupported(
                    "conformance family tuple has no bare value-tuple specialization of the \
                     row's provider",
                );
            };
            if specializations.next().is_some() {
                return unsupported(
                    "conformance family tuple resolves to ambiguous provider specializations",
                );
            }
            let instance = checked
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
                .ok_or(LoweringError::Unsupported(
                    "conformance family specialization has no retained instance machine",
                ))?;
            let instance_state = checked
                .typed
                .machine_states(instance)
                .get(state_position)
                .ok_or(LoweringError::Unsupported(
                    "conformance family specialization lost its tuple state",
                ))?;
            Ok(CheckedRequirementFamilyRow {
                family_tuple: tuple.identities.to_vec(),
                realization_machine: instance.symbol,
                realization_state: instance_state.symbol,
            })
        })
        .collect()
}

/// The exact typed requirement one `(declaring trait, requirement)` symbol
/// pair names, projected to the facts evidence and dynamic rows consume.
struct ExactTraitRequirement<'a> {
    /// The resolved signature; `finite_signature_family` reads its `where`
    /// clause for the family roster.
    signature: &'a checked_trees::signature::StateSignature,
    /// Canonical normalized overload identity; never empty.
    overload_identity: String,
    /// Whether the signature declares requirement-local generic binders.
    declares_local_generic_binders: bool,
}

fn exact_trait_requirement<'a>(
    checked: &'a CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<ExactTraitRequirement<'a>, LoweringError> {
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
        signature,
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
