use super::{VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError};

#[cfg(test)]
use crate::shared::*;

pub(super) fn project_proof_questions(
    input: &VerifiedPsiOptimizationInput,
) -> Result<Vec<optimization_unit::ProofQuestion>, VerifiedPsiOptimizationUnitBuildError> {
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
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    proof_fingerprint: [u8; 32],
    row: &terminal_verifier::ReconstructedTerminalObligation,
) -> Result<optimization_unit::ProofQuestion, VerifiedPsiOptimizationUnitBuildError> {
    let owner = match row.owner {
        terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
            machine,
            operation,
        } => optimization_unit::ProofQuestionOwner::Operation { machine, operation },
        terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        } => optimization_unit::ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position,
        },
        terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        } => optimization_unit::ProofQuestionOwner::NominalCleanupRequires {
            machine,
            edge,
            cleanup_position,
            requirement_position,
        },
        terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        } => optimization_unit::ProofQuestionOwner::ContractEnsures {
            machine,
            contract,
            clause_position,
        },
    };
    let class = match row.obligation.class {
        proof_admission::ObligationClass::Derivable => {
            optimization_unit::ProofQuestionClass::Derivable
        }
        proof_admission::ObligationClass::AdmissionAuthorized(admission) => {
            let kind = match admission.kind {
                proof_admission::AdmissionKind::ForeignBoundaryGuarantee => {
                    optimization_unit::ProofQuestionAdmissionKind::ForeignBoundaryGuarantee
                }
                proof_admission::AdmissionKind::ProviderFact => {
                    optimization_unit::ProofQuestionAdmissionKind::ProviderFact
                }
                proof_admission::AdmissionKind::CheckedAssemblyClaim => {
                    optimization_unit::ProofQuestionAdmissionKind::CheckedAssemblyClaim
                }
            };
            optimization_unit::ProofQuestionClass::AdmissionAuthorized {
                site: admission.site,
                kind,
                authority_identity: admission.authority_identity,
            }
        }
    };
    let proposition = terminal_codec::canonical_proposition_order_key(&row.obligation.proposition)?;
    let requirements = row
        .requirements
        .iter()
        .map(terminal_codec::canonical_proposition_order_key)
        .collect::<Result<Vec<_>, _>>()?;
    let semantic_axioms = row
        .semantic_axioms
        .iter()
        .map(terminal_codec::canonical_proposition_order_key)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(optimization_unit::ProofQuestion::new(
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
    let row = terminal_verifier::ReconstructedTerminalObligation {
        owner: terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
            machine,
            operation,
            requirement_position: 3,
        },
        obligation: proof_admission::Obligation {
            id: ObligationId::new(4).unwrap(),
            proposition: Proposition::Truth,
            class: proof_admission::ObligationClass::AdmissionAuthorized(
                proof_admission::AuthorizedAdmission {
                    site: semantic_vocabulary::AdmissionSiteId::new(5).unwrap(),
                    kind: proof_admission::AdmissionKind::ForeignBoundaryGuarantee,
                    authority_identity: semantic_vocabulary::EvidenceIdentity::new(6).unwrap(),
                },
            ),
        },
        requirements: vec![Proposition::Truth],
        semantic_axioms: vec![Proposition::Truth, Proposition::Falsehood],
        canonical_certificate: false,
    };
    let terminal_psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
    };
    let projected = project_proof_question_row(terminal_psi, [8; 32], &row).unwrap();

    assert_eq!(
        projected.owner,
        optimization_unit::ProofQuestionOwner::CallRequires {
            machine,
            operation,
            requirement_position: 3,
        }
    );
    assert_eq!(
        projected.class,
        optimization_unit::ProofQuestionClass::AdmissionAuthorized {
            site: semantic_vocabulary::AdmissionSiteId::new(5).unwrap(),
            kind: optimization_unit::ProofQuestionAdmissionKind::ForeignBoundaryGuarantee,
            authority_identity: semantic_vocabulary::EvidenceIdentity::new(6).unwrap(),
        }
    );
    assert_eq!(projected.requirements.len(), 1);
    assert_eq!(projected.semantic_axioms.len(), 2);
    assert!(projected.has_canonical_identity());
}
