//! Independent proof-question and ownership-frontier context projection.

use super::*;

pub(super) struct ContextProjection {
    pub(super) proof_fingerprint: [u8; 32],
    pub(super) proof_questions: Vec<ProofQuestion>,
    pub(super) ownership_frontiers: Vec<OwnershipFrontierFact>,
}

pub(super) fn validate_context_projection(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ContextProjection, OptimizationUnitValidationError> {
    let context = input.context();
    let terminal_identity = terminal_codec::terminal_psi_identity(context.module())
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if input.plan().psi != terminal_identity || unit.psi != terminal_identity {
        return Err(OptimizationUnitValidationError::TerminalIdentityMismatch);
    }
    let proof_fingerprint = terminal_codec::proof_bundle_fingerprint(context.proof_bundle())
        .map_err(OptimizationUnitValidationError::ContextProofFingerprint)?;
    if proof_fingerprint != context.proof_bundle_fingerprint() {
        return Err(OptimizationUnitValidationError::ProofFingerprintMismatch);
    }
    let proof_questions = independently_project_proof_questions(input)
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if proof_questions != unit.proof_questions {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    let ownership_frontiers = independently_project_ownership_frontiers(input)
        .ok_or(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)?;
    if ownership_frontiers != unit.ownership_frontier_facts {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }
    Ok(ContextProjection {
        proof_fingerprint: *proof_fingerprint.as_bytes(),
        proof_questions,
        ownership_frontiers,
    })
}

fn independently_project_proof_questions(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Result<Vec<ProofQuestion>, terminal_codec::CodecError> {
    let context = input.context();
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| {
            let owner = match row.owner {
                terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                    machine,
                    operation,
                } => ProofQuestionOwner::Operation { machine, operation },
                terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                } => ProofQuestionOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                },
                terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                    machine,
                    edge,
                    cleanup_position,
                    requirement_position,
                } => ProofQuestionOwner::NominalCleanupRequires {
                    machine,
                    edge,
                    cleanup_position,
                    requirement_position,
                },
                terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures {
                    machine,
                    contract,
                    clause_position,
                } => ProofQuestionOwner::ContractEnsures {
                    machine,
                    contract,
                    clause_position,
                },
            };
            let class = match row.obligation.class {
                proof_admission::ObligationClass::Derivable => ProofQuestionClass::Derivable,
                proof_admission::ObligationClass::AdmissionAuthorized(admission) => {
                    let kind = match admission.kind {
                        proof_admission::AdmissionKind::ForeignBoundaryGuarantee => {
                            ProofQuestionAdmissionKind::ForeignBoundaryGuarantee
                        }
                        proof_admission::AdmissionKind::ProviderFact => {
                            ProofQuestionAdmissionKind::ProviderFact
                        }
                        proof_admission::AdmissionKind::CheckedAssemblyClaim => {
                            ProofQuestionAdmissionKind::CheckedAssemblyClaim
                        }
                    };
                    ProofQuestionClass::AdmissionAuthorized {
                        site: admission.site,
                        kind,
                        authority_identity: admission.authority_identity,
                    }
                }
            };
            let proposition =
                terminal_codec::canonical_proposition_order_key(&row.obligation.proposition)?;
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
            Ok(ProofQuestion::new(
                input.plan().psi,
                proof_fingerprint,
                owner,
                row.obligation.id,
                class,
                proposition,
                requirements,
                semantic_axioms,
                row.canonical_certificate,
            ))
        })
        .collect()
}

fn independently_project_ownership_frontiers(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Option<Vec<OwnershipFrontierFact>> {
    let context = input.context();
    let mut facts = Vec::new();
    for machine in &context.module().machines {
        let frontiers = context.structural_frontiers().machine(machine.id)?;
        for block in &machine.blocks {
            push_independent_ownership_frontier(
                &mut facts,
                input.plan().psi,
                machine.id,
                OwnershipFrontierSite::BlockEntry(block.id),
                frontiers.block_entry(block.id)?,
            );
            for operation in &block.operations {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::OperationEntry(operation.id),
                    frontiers.operation_entry(operation.id)?,
                );
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::OperationExit(operation.id),
                    frontiers.operation_exit(operation.id)?,
                );
            }
            for edge in block.terminator.edges() {
                push_independent_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    OwnershipFrontierSite::EdgeEntry(edge),
                    frontiers.edge_entry(edge)?,
                );
                if let Some(snapshot) = frontiers.edge_exit(edge) {
                    push_independent_ownership_frontier(
                        &mut facts,
                        input.plan().psi,
                        machine.id,
                        OwnershipFrontierSite::EdgeExit(edge),
                        snapshot,
                    );
                }
            }
        }
    }
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Some(facts)
}

fn push_independent_ownership_frontier(
    facts: &mut Vec<OwnershipFrontierFact>,
    psi: terminal_psi::TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &terminal_verifier::VerifiedStructuralOwnershipFrontier,
) {
    facts.push(OwnershipFrontierFact::new(
        psi,
        machine,
        site,
        OwnershipFrontierSnapshot {
            claims: snapshot
                .claims()
                .iter()
                .map(|claim| OwnershipFrontierLiveClaim {
                    claim: claim.claim,
                    input: claim.input,
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                })
                .collect(),
            owned_places: snapshot
                .owned_places()
                .iter()
                .map(|place| OwnershipFrontierOwnedPlace {
                    place: place.place,
                    multiplicity: place.multiplicity,
                })
                .collect(),
            partial_custody: snapshot
                .partial_custody()
                .iter()
                .map(|partial| OwnershipFrontierPartialCustody {
                    place: partial.place,
                    moved_paths: partial.moved_paths.clone(),
                })
                .collect(),
        },
    ));
}
