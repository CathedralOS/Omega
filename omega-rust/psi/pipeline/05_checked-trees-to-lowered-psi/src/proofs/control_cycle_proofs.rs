//! Produce certificates for the verifier's exact runtime ranking questions.
use super::{
    CertificateEnvelope, EvidenceIdentity, EvidenceRoute, LoweredPsi, LoweringError, ProofNode,
    ProofRule, ProofSearchDeclines, ProofSystemMarker,
};
use proof_admission::{
    CertificateObligation, RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use std::collections::btree_map::{BTreeMap, Entry};
use terminal_psi::ControlCycleEvidence;

/// Produce the cycle certificates. `declines` carries this finalization's
/// recorded proof-search declines so an unmet ranking obligation names the
/// capacity that stopped optional inference rather than reading as a
/// disproof of the module.
pub(crate) fn finalize(
    lowered: &mut LoweredPsi,
    declines: &ProofSearchDeclines,
) -> Result<(), LoweringError> {
    let started = std::time::Instant::now();
    let questions =
        terminal_verifier::reconstruct_control_cycle_obligations(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    crate::proofs::proof_timing::count_reconstruct(started.elapsed());
    if questions.is_empty() {
        return Ok(());
    }
    let started = std::time::Instant::now();
    let validated = terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    crate::proofs::proof_timing::count_validate(started.elapsed());
    let mut evidence = Vec::new();
    // A value context is machine-local: questions on one machine share it
    // instead of rebuilding the machine's proposition context per question.
    let mut contexts = BTreeMap::new();
    for question in questions {
        let machine = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == question.machine)
            .ok_or(LoweringError::Unsupported("cycle proof owner disappeared"))?;
        let context = match contexts.entry(machine.id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let started = std::time::Instant::now();
                let context = validated
                    .value_context(machine)
                    .map_err(LoweringError::InvalidTerminalModule)?;
                crate::proofs::proof_timing::count_value_context(started.elapsed());
                entry.insert(context)
            }
        };
        let context = &*context;
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        let produce = |obligation: &CertificateObligation| -> Result<EvidenceRoute, LoweringError> {
            let proof = if obligation.semantic_axioms.first()
                == Some(&obligation.obligation.proposition)
            {
                Some(ProofNode {
                    conclusion: obligation.obligation.proposition.clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                })
            } else {
                crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof(
                    context,
                    &obligation.obligation.proposition,
                    &obligation.assumptions,
                    &obligation.semantic_axioms,
                    &parameters,
                )
            }
            .ok_or_else(|| declines.unproved_obligation_error(obligation.obligation.id))?;
            Ok(EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(obligation.obligation.id.get())
                    .expect("nonzero obligation"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }))
        };
        let well_foundedness = produce(&question.obligation.well_foundedness)?;
        let edges = question
            .obligation
            .edges
            .iter()
            .map(|edge| {
                Ok(RecursiveEdgeCertificate {
                    obligation: edge.decrease.obligation.id,
                    evidence: produce(&edge.decrease)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        evidence.push(ControlCycleEvidence {
            component: question.component,
            certificate: RecursiveComponentCertificate {
                identity: EvidenceIdentity::new(question.component.get())
                    .expect("nonzero component"),
                ranking_relation: question
                    .obligation
                    .ranking_relation
                    .ok_or(LoweringError::Unsupported("cycle proof has no relation"))?,
                well_foundedness,
                edges,
            },
        });
    }
    lowered.proof_bundle.control_cycles = evidence;
    Ok(())
}
