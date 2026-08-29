use super::{VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError};

#[cfg(test)]
use crate::shared::*;

pub(super) fn project_proof_questions(
    input: &VerifiedPsiOptimizationInput,
) -> Result<Vec<omega_optimization_unit::ProofQuestion>, VerifiedPsiOptimizationUnitBuildError> {
    let context = input.context();
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| project_proof_question_row(input.plan().psi, proof_fingerprint, row))
        .collect()
}

fn project_proof_question_row(
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    proof_fingerprint: [u8; 32],
    row: &psi_terminal_verifier::ReconstructedTerminalObligation,
) -> Result<omega_optimization_unit::ProofQuestion, VerifiedPsiOptimizationUnitBuildError> {
    let owner = match row.owner {
        psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
            machine,
            operation,
        } => omega_optimization_unit::ProofQuestionOwner::Operation { machine, operation },
        psi_terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => omega_optimization_unit::ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        },
        psi_terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => omega_optimization_unit::ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        },
        psi_terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => omega_optimization_unit::ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        },
    };
    let class = match row.obligation.class {
        psi_proof_admission::ObligationClass::Derivable => {
            omega_optimization_unit::ProofQuestionClass::Derivable
        }
        psi_proof_admission::ObligationClass::AdmissionAuthorized(admission) => {
            let kind = match admission.kind {
                psi_proof_admission::AdmissionKind::ForeignBoundaryGuarantee => {
                    omega_optimization_unit::ProofQuestionAdmissionKind::ForeignBoundaryGuarantee
                }
                psi_proof_admission::AdmissionKind::ProviderFact => {
                    omega_optimization_unit::ProofQuestionAdmissionKind::ProviderFact
                }
                psi_proof_admission::AdmissionKind::CheckedAssemblyClaim => {
                    omega_optimization_unit::ProofQuestionAdmissionKind::CheckedAssemblyClaim
                }
            };
            omega_optimization_unit::ProofQuestionClass::AdmissionAuthorized {
                site: admission.site,
                kind,
                authority_identity: admission.authority_identity,
            }
        }
    };
    let proposition =
        psi_terminal_codec::canonical_proposition_order_key(&row.obligation.proposition)?;
    let requirements = row
        .requirements
        .iter()
        .map(psi_terminal_codec::canonical_proposition_order_key)
        .collect::<Result<Vec<_>, _>>()?;
    let semantic_axioms = row
        .semantic_axioms
        .iter()
        .map(psi_terminal_codec::canonical_proposition_order_key)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(omega_optimization_unit::ProofQuestion::new(
        terminal_psi,
        proof_fingerprint,
        owner,
        row.obligation.id,
        class,
        proposition,
        requirements,
        semantic_axioms,
        row.canonical_certificate,
    ))
}

#[test]
fn proof_question_projection_retains_call_owner_and_admission_class() {
    let machine = MachineId::new(1).unwrap();
    let operation = OperationId::new(2).unwrap();
    let row = psi_terminal_verifier::ReconstructedTerminalObligation {
        owner: psi_terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position: 3,
        },
        obligation: psi_proof_admission::Obligation {
            id: ObligationId::new(4).unwrap(),
            proposition: Proposition::Truth,
            class: psi_proof_admission::ObligationClass::AdmissionAuthorized(
                psi_proof_admission::AuthorizedAdmission {
                    site: psi_core::AdmissionSiteId::new(5).unwrap(),
                    kind: psi_proof_admission::AdmissionKind::ForeignBoundaryGuarantee,
                    authority_identity: psi_core::EvidenceIdentity::new(6).unwrap(),
                },
            ),
        },
        requirements: vec![Proposition::Truth],
        semantic_axioms: vec![Proposition::Truth, Proposition::Falsehood],
        canonical_certificate: false,
    };
    let terminal_psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([7; 32]),
    };
    let projected = project_proof_question_row(terminal_psi, [8; 32], &row).unwrap();

    assert_eq!(
        projected.owner,
        omega_optimization_unit::ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position: 3,
        }
    );
    assert_eq!(
        projected.class,
        omega_optimization_unit::ProofQuestionClass::AdmissionAuthorized {
            site: psi_core::AdmissionSiteId::new(5).unwrap(),
            kind: omega_optimization_unit::ProofQuestionAdmissionKind::ForeignBoundaryGuarantee,
            authority_identity: psi_core::EvidenceIdentity::new(6).unwrap(),
        }
    );
    assert_eq!(projected.requirements.len(), 1);
    assert_eq!(projected.semantic_axioms.len(), 2);
    assert!(projected.has_canonical_identity());
}
