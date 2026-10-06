use super::{VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError};

pub(super) fn project_proof_questions(
    input: &VerifiedPsiOptimizationInput,
) -> Result<Vec<optimization_unit::ProofQuestion>, VerifiedPsiOptimizationUnitBuildError> {
    let context = input.context();
    // No proof bundle exists; questions carry the zero fingerprint.
    let proof_fingerprint = [0; 32];
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
        terminal_verifier::ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            machine,
            header,
            edge,
        } => optimization_unit::ProofQuestionOwner::ScalarBlockInvariant {
            machine,
            header,
            edge,
        },
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
