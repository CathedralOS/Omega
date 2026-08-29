//! Verified and transformed optimizer-context validation.

use super::*;

pub fn validate_verified_psi_optimization_unit(
    verified: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(verified.input(), verified.unit(), true)
}

/// Validate a committed optimization revision while retaining the immutable
/// verifier context that authorized its proof and ownership facts.
///
/// Unlike [`validate_verified_psi_optimization_unit`], this permits the unit's
/// revision identity and executable shape to differ from the initial verified
/// seed. The admitted-fact projection and every surviving provenance frontier
/// must still match the original artifact exactly.
pub fn validate_transformed_psi_optimization_unit(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false)
}

pub(crate) fn validate_psi_optimization_unit_with_context(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    require_initial_revision: bool,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let context = input.context();
    let terminal_identity = psi_terminal_codec::terminal_psi_identity(context.module())
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if input.plan().psi != terminal_identity || unit.psi != terminal_identity {
        return Err(OptimizationUnitValidationError::TerminalIdentityMismatch);
    }
    let proof_fingerprint = psi_terminal_codec::proof_bundle_fingerprint(context.proof_bundle())
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

    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    if reconstructed.len() != accepted.len() {
        let obligation = reconstructed
            .keys()
            .find(|id| !accepted.contains_key(id))
            .or_else(|| accepted.keys().find(|id| !reconstructed.contains_key(id)))
            .copied()
            .expect("different finite obligation maps have a differing key");
        return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
            obligation,
        ));
    }
    for (obligation, row) in &reconstructed {
        if accepted
            .get(obligation)
            .is_none_or(|fact| fact.proposition != row.obligation.proposition)
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
                *obligation,
            ));
        }
    }

    let mut seed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        unit.fuel_schedule,
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    attach_verified_structural_context(&mut seed, context.module())?;
    if !same_immutable_signature_custody(&seed, unit) {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }
    let mut projected_facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let row = reconstructed.get(obligation).filter(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            let fact = accepted.get(obligation);
            let (Some(row), Some(fact)) = (row, fact) else {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            };
            if row.obligation.proposition != fact.proposition {
                return Err(
                    OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
                );
            }
            let proposition =
                psi_terminal_codec::canonical_proposition_order_key(&fact.proposition)
                    .map_err(OptimizationUnitValidationError::ContextIdentity)?;
            projected_facts.push(omega_optimization_unit::AcceptedObligationFact::new(
                seed.psi,
                *proof_fingerprint.as_bytes(),
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    let projected =
        omega_optimization_unit::attach_accepted_obligation_facts(seed, projected_facts).map_err(
            |_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch,
        )?;
    let projected = omega_optimization_unit::attach_proof_questions(projected, proof_questions)
        .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    let projected =
        omega_optimization_unit::attach_ownership_frontier_facts(projected, ownership_frontiers)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    if (require_initial_revision && projected.identity != unit.identity)
        || projected.accepted_obligation_facts != unit.accepted_obligation_facts
        || projected.proof_questions != unit.proof_questions
    {
        return Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch);
    }

    for function in &unit.functions {
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn attach_verified_structural_context(
    unit: &mut PsiOptimizationUnit,
    module: &psi_terminal::TerminalModule,
) -> Result<(), OptimizationUnitValidationError> {
    unit.structural_domains = module.structural_domains.clone().into();
    unit.services = module.services.clone().into();
    unit.root_service_reach = module.root_service_reach.clone();
    for function in &mut unit.functions {
        let source = module
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .ok_or(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
        function.verified_contract = Some(source.contract.clone());
        function.evidence_contract_lanes = module
            .evidence_contract_lanes
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .cloned()
            .collect();
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
    Ok(())
}

pub(crate) fn independently_project_proof_questions(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
) -> Result<Vec<ProofQuestion>, psi_terminal_codec::CodecError> {
    let context = input.context();
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| {
            let owner = match row.owner {
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                    machine,
                    operation,
                } => ProofQuestionOwner::Operation { machine, operation },
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                } => ProofQuestionOwner::CallRequires {
                    machine,
                    operation,
                    requirement_position,
                },
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::NominalCleanupRequires {
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
                psi_terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures {
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
                psi_proof_admission::ObligationClass::Derivable => ProofQuestionClass::Derivable,
                psi_proof_admission::ObligationClass::AdmissionAuthorized(admission) => {
                    let kind = match admission.kind {
                        psi_proof_admission::AdmissionKind::ForeignBoundaryGuarantee => {
                            ProofQuestionAdmissionKind::ForeignBoundaryGuarantee
                        }
                        psi_proof_admission::AdmissionKind::ProviderFact => {
                            ProofQuestionAdmissionKind::ProviderFact
                        }
                        psi_proof_admission::AdmissionKind::CheckedAssemblyClaim => {
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

pub(crate) fn independently_project_ownership_frontiers(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
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

pub(crate) fn push_independent_ownership_frontier(
    facts: &mut Vec<OwnershipFrontierFact>,
    psi: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
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

pub(crate) fn same_immutable_signature_custody(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    seed.psi == unit.psi
        && seed.entry == unit.entry
        && seed.structural_types == unit.structural_types
        && structural_domain_catalog_identity(seed.structural_domains.as_ref())
            == structural_domain_catalog_identity(unit.structural_domains.as_ref())
        && seed.services == unit.services
        && seed.boundary_machines == unit.boundary_machines
        && seed.provider_candidates == unit.provider_candidates
        && source_roster_partition_is_exact(seed, unit)
        && unit.functions.iter().all(|unit| {
            seed.functions
                .iter()
                .find(|seed| seed.machine == unit.machine)
                .is_some_and(|seed| {
                    seed.machine == unit.machine
                        && seed.attachment == unit.attachment
                        && seed.parameters == unit.parameters
                        && seed.structural_parameters == unit.structural_parameters
                        && seed.structural_places == unit.structural_places
                        && seed.result == unit.result
                        && seed.entry_claim_declarations == unit.entry_claim_declarations
                        && seed.content_entry_claims == unit.content_entry_claims
                        && seed.verified_contract == unit.verified_contract
                        && seed.evidence_contract_lanes == unit.evidence_contract_lanes
                        && seed.entry_claims == unit.entry_claims
                        && seed.published_service_ceiling == unit.published_service_ceiling
                })
        })
}

pub(crate) fn source_roster_partition_is_exact(
    seed: &PsiOptimizationUnit,
    unit: &PsiOptimizationUnit,
) -> bool {
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    if active.len() != unit.functions.len() || active.len() + pruned.len() != seed.functions.len() {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source) in seed.functions.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok();
        if active.contains(&source.machine) {
            if active_order.next() != Some(source.machine) {
                return false;
            }
        } else if ordinal.and_then(|ordinal| pruned.get(&ordinal).copied()) != Some(source.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}
