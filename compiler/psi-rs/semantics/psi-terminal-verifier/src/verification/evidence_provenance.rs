//! Exact proof-artifact producer custody, separate from proposition identity.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::EvidenceIdentity;
use psi_terminal::TerminalModule;

use super::{EvidenceProducerRealization, ProofBundle, VerificationError};

pub(super) fn validate_evidence_producer_provenance(
    module: &TerminalModule,
    proof_bundle: &ProofBundle,
) -> Result<(), VerificationError> {
    use psi_terminal::EvidenceContractLaneKind;

    let terms = module
        .evidence_terms
        .iter()
        .map(|term| (term.id, term))
        .collect::<BTreeMap<_, _>>();
    let required = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| lane.kind == EvidenceContractLaneKind::Requires)
        .map(|lane| (lane.machine, lane.term))
        .collect::<BTreeSet<_>>();
    let mut unmatched_ensures = BTreeMap::<_, usize>::new();
    let package_outputs = module
        .proof_output_calls
        .iter()
        .flat_map(|invocation| invocation.outputs.iter().filter_map(|output| output.output))
        .collect::<BTreeSet<_>>();
    for lane in &module.evidence_contract_lanes {
        if lane.kind == EvidenceContractLaneKind::Ensures
            && !required.contains(&(lane.machine, lane.term))
            && !package_outputs.contains(&lane.term)
        {
            *unmatched_ensures.entry(lane.term).or_default() += 1;
        }
    }
    for invocation in &module.proof_output_calls {
        for output in &invocation.outputs {
            if let Some(callee_output) = output.callee_output {
                unmatched_ensures.entry(callee_output).or_insert(1);
            }
        }
    }

    let mut previous_id = None;
    let mut previous_term = None;
    let mut produced_terms = BTreeSet::new();
    for (index, provenance) in proof_bundle.evidence_producers.iter().enumerate() {
        let expected = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("producer provenance count fits u64")
                .checked_add(1)
                .expect("one-based producer provenance identity fits u64"),
        )
        .expect("one-based producer provenance identity is nonzero");
        if provenance.id != expected {
            return Err(VerificationError::NonDenseEvidenceProducer {
                expected,
                actual: provenance.id,
            });
        }
        if previous_id.is_some_and(|previous| previous >= provenance.id) {
            return Err(VerificationError::NonCanonicalEvidenceProducerOrder);
        }
        previous_id = Some(provenance.id);
        if previous_term.is_some_and(|previous| previous >= provenance.term) {
            return Err(VerificationError::NonCanonicalEvidenceProducerOrder);
        }
        previous_term = Some(provenance.term);
        if !produced_terms.insert(provenance.term) {
            return Err(VerificationError::DuplicateEvidenceProducerTerm(
                provenance.term,
            ));
        }
        let term =
            terms
                .get(&provenance.term)
                .ok_or(VerificationError::UnknownEvidenceProducerTerm(
                    provenance.term,
                ))?;
        if unmatched_ensures.get(&provenance.term).copied() != Some(1) {
            return Err(VerificationError::UnusedEvidenceProducerTerm(
                provenance.term,
            ));
        }
        if provenance.conformance_identity.is_empty()
            || provenance.evidence_trait_identity.is_empty()
        {
            return Err(VerificationError::InvalidEvidenceProducer(provenance.id));
        }
        if provenance.evidence_trait_identity != term.interface.trait_identity {
            return Err(VerificationError::EvidenceProducerInterfaceMismatch(
                provenance.term,
            ));
        }
        let mut previous_row = None;
        for row in &provenance.rows {
            if row.declaring_trait_identity.is_empty()
                || row.declaring_trait_arguments.iter().any(String::is_empty)
                || row.requirement_identity.is_empty()
                || row.realization_machine_identity.is_empty()
                || row.realization_state_identity.is_empty()
            {
                return Err(VerificationError::InvalidEvidenceProducer(provenance.id));
            }
            if previous_row.is_some_and(|previous: &EvidenceProducerRealization| previous >= row) {
                return Err(VerificationError::NonCanonicalEvidenceProducerRows(
                    provenance.id,
                ));
            }
            previous_row = Some(row);
        }
        let mut realized_requirements = provenance
            .rows
            .iter()
            .map(|row| psi_terminal::EvidenceRequirementIdentity {
                declaring_trait_identity: row.declaring_trait_identity.clone(),
                declaring_trait_arguments: row.declaring_trait_arguments.clone(),
                requirement_identity: row.requirement_identity.clone(),
            })
            .collect::<Vec<_>>();
        realized_requirements.sort();
        if realized_requirements != term.interface.requirements {
            return Err(VerificationError::EvidenceProducerInterfaceMismatch(
                provenance.term,
            ));
        }
    }
    if let Some(term) = unmatched_ensures
        .keys()
        .find(|term| !produced_terms.contains(term))
        .copied()
    {
        return Err(VerificationError::MissingEvidenceProducer(term));
    }
    Ok(())
}
