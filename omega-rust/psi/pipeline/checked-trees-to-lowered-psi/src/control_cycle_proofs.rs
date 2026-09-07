//! Produce certificates for the verifier's exact runtime ranking questions.

use super::*;
use proof_admission::{
    CertificateObligation, RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use terminal_psi::ControlCycleEvidence;

pub(super) fn finalize(lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
    let questions =
        terminal_verifier::reconstruct_control_cycle_obligations(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    if questions.is_empty() {
        return Ok(());
    }
    let validated = terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut evidence = Vec::new();
    for question in questions {
        let machine = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == question.machine)
            .ok_or(LoweringError::Unsupported("cycle proof owner disappeared"))?;
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        let produce = |obligation: &CertificateObligation| -> Result<EvidenceRoute, LoweringError> {
            let proof =
                if obligation.semantic_axioms.first() == Some(&obligation.obligation.proposition) {
                    Some(ProofNode {
                        conclusion: obligation.obligation.proposition.clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    })
                } else {
                    crate::nonzero_divisor_certificate::produce_checked_canonical_integer_proof(
                        &context,
                        &obligation.obligation.proposition,
                        &obligation.assumptions,
                        &obligation.semantic_axioms,
                        &parameters,
                    )
                }
                .ok_or(LoweringError::OperationProofUnavailable(
                    obligation.obligation.id,
                ))?;
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
