//! Verified context, identity, frontier, and proof-catalog tests.

use super::*;

#[test]
fn stale_stored_content_identity_is_rejected_before_structural_validation() {
    let mut stale = unit();
    stale.functions[0].blocks[0].nodes[0].effect.output += 1;
    let recomputed = recompute_psi_optimization_unit_identity(&stale);
    assert!(matches!(
        validate_psi_optimization_unit(&stale),
        Err(OptimizationUnitValidationError::ContentIdentityMismatch {
            stored,
            recomputed: actual,
        }) if stored == stale.identity && actual == recomputed
    ));
}

#[test]
fn recomputed_immutable_signature_forgery_is_rejected_by_verified_context() {
    let verified = verified_unit();
    let structural_type = id(120, psi_core::StructuralTypeId::new);
    let boundary = id(121, psi_core::BoundaryMachineId::new);
    let service = id(122, psi_core::ServiceId::new);
    let mut forged = Vec::new();

    let mut unit = verified.unit().clone();
    unit.structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "forged-structural-type".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        });
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: "forged-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.provider_candidates
        .push(psi_terminal::ProviderCandidateConformance {
            boundary,
            requirement_identity: "forged-requirement".into(),
            provider_identity: "forged-provider".into(),
            candidate_identity: "forged-candidate".into(),
            candidate: unit.functions[0].machine,
            signature: psi_terminal::ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: psi_terminal::ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.functions[0].attachment = Some(structural_type);
    forged.push(unit);

    let mut unit = verified.unit().clone();
    let result_value = id(126, ValueId::new);
    unit.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result_value,
        scalar_type: ScalarType::Boolean,
    });
    unit.functions[0].parameters.push(ValueDefinition {
        value: result_value,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::FunctionParameter(0),
    });
    let block = unit.functions[0].blocks[0].id;
    let node = &mut unit.functions[0].blocks[0].nodes[0];
    let psi_edge = match &node.operation {
        AbstractOperation::ReturnUnit { psi_edge, .. } => *psi_edge,
        _ => panic!("verified fixture must return Unit"),
    };
    node.operation = AbstractOperation::Return {
        psi_edge,
        result: result_value,
        value: result_value,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: Vec::new(),
    };
    node.uses = vec![ValueUse {
        value: result_value,
        block,
        node: 0,
    }];
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.services = vec![psi_terminal::ServiceDeclaration {
        id: service,
        identity: "forged-service".into(),
        parents: Vec::new(),
    }]
    .into();
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.services = vec![psi_terminal::ServiceDeclaration {
        id: service,
        identity: "forged-service".into(),
        parents: Vec::new(),
    }]
    .into();
    unit.functions[0].published_service_ceiling.push(service);
    forged.push(unit);

    let mut unit = verified.unit().clone();
    let claim = id(123, ClaimId::new);
    let place = id(124, PlaceId::new);
    unit.functions[0]
        .entry_claim_declarations
        .push(psi_terminal::EntryClaim {
            claim,
            input: place,
            path: Vec::new(),
        });
    unit.functions[0].entry_claims.insert(claim);
    unit.functions[0].declared_places.insert(place);
    forged.push(unit);

    for (index, mut unit) in forged.into_iter().enumerate() {
        refresh_identity(&mut unit);
        let result = validate_transformed_psi_optimization_unit(verified.input(), &unit);
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)
                    | Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
                    | Err(OptimizationUnitValidationError::InvalidProviderServiceRefinement { .. })
            ),
            "forgery class {index} returned {result:?}"
        );
    }
}

#[test]
fn ownership_frontier_catalog_rejects_reordering_duplication_and_context_forgery() {
    let verified = verified_unit();
    let original = verified.unit();
    assert!(original.ownership_frontier_facts.len() >= 2);

    let mut reordered = original.clone();
    reordered.ownership_frontier_facts.swap(0, 1);
    refresh_identity(&mut reordered);
    assert_eq!(
        validate_psi_optimization_unit(&reordered),
        Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
    );

    let mut duplicated = original.clone();
    duplicated
        .ownership_frontier_facts
        .insert(1, duplicated.ownership_frontier_facts[0].clone());
    refresh_identity(&mut duplicated);
    assert_eq!(
        validate_psi_optimization_unit(&duplicated),
        Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
    );

    let mut missing = original.clone();
    missing.ownership_frontier_facts.pop();
    refresh_identity(&mut missing);
    assert_eq!(
        validate_transformed_psi_optimization_unit(verified.input(), &missing),
        Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
    );

    let mut forged = original.clone();
    let prior = forged.ownership_frontier_facts[0].clone();
    let mut snapshot = prior.snapshot;
    snapshot.owned_places.push(OwnershipFrontierOwnedPlace {
        place: id(130, PlaceId::new),
        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
    });
    snapshot.owned_places.sort_by_key(|place| place.place);
    forged.ownership_frontier_facts[0] =
        OwnershipFrontierFact::new(prior.psi, prior.machine, prior.site, snapshot);
    refresh_identity(&mut forged);
    assert_eq!(
        validate_transformed_psi_optimization_unit(verified.input(), &forged),
        Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)
    );
}

#[test]
fn proof_question_catalog_rejects_missing_reordered_duplicate_and_forged_rows() {
    let verified = verified_unit();
    let original = verified.unit();
    assert_eq!(original.proof_questions.len(), 2);
    assert!(
        original
            .proof_questions
            .iter()
            .all(|question| matches!(question.owner, ProofQuestionOwner::ContractEnsures { .. }))
    );

    let mut reordered = original.clone();
    reordered.proof_questions.swap(0, 1);
    refresh_identity(&mut reordered);
    assert_eq!(
        validate_transformed_psi_optimization_unit(verified.input(), &reordered),
        Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
    );

    let mut duplicated = original.clone();
    duplicated
        .proof_questions
        .insert(1, duplicated.proof_questions[0].clone());
    refresh_identity(&mut duplicated);
    assert_eq!(
        validate_psi_optimization_unit(&duplicated),
        Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
    );

    let mut missing = original.clone();
    missing.proof_questions.pop();
    refresh_identity(&mut missing);
    assert_eq!(
        validate_transformed_psi_optimization_unit(verified.input(), &missing),
        Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch)
    );

    let mut corruptions = Vec::new();
    let mut owner = original.clone();
    owner.proof_questions[0].owner = ProofQuestionOwner::ContractEnsures {
        machine: owner.functions[0].machine,
        contract: id(104, psi_core::ContractId::new),
        clause_position: 7,
    };
    corruptions.push(owner);
    let mut obligation = original.clone();
    obligation.proof_questions[0].obligation = id(107, psi_core::ObligationId::new);
    corruptions.push(obligation);
    let mut class = original.clone();
    class.proof_questions[0].class = ProofQuestionClass::AdmissionAuthorized {
        site: id(108, psi_core::AdmissionSiteId::new),
        kind: ProofQuestionAdmissionKind::CheckedAssemblyClaim,
        authority_identity: id(109, psi_core::EvidenceIdentity::new),
    };
    corruptions.push(class);
    let mut proposition = original.clone();
    proposition.proof_questions[0].proposition.push(1);
    corruptions.push(proposition);
    let mut requirements = original.clone();
    requirements.proof_questions[0].requirements.push(vec![2]);
    corruptions.push(requirements);
    let mut axioms = original.clone();
    axioms.proof_questions[0].semantic_axioms.push(vec![3]);
    corruptions.push(axioms);
    let mut certificate = original.clone();
    certificate.proof_questions[0].canonical_certificate = true;
    corruptions.push(certificate);
    let mut fingerprint = original.clone();
    fingerprint.proof_questions[0].proof_bundle_fingerprint[0] ^= 1;
    corruptions.push(fingerprint);

    for (index, mut corruption) in corruptions.into_iter().enumerate() {
        refresh_proof_question_identity(&mut corruption.proof_questions[0]);
        refresh_identity(&mut corruption);
        assert_eq!(
            validate_transformed_psi_optimization_unit(verified.input(), &corruption),
            Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch),
            "self-consistent proof-question forgery {index}"
        );
    }
}

#[test]
fn bare_unit_result_signature_must_match_normal_exits() {
    let mut forged = verified_unit().unit().clone();
    forged.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
        value: id(125, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    refresh_identity(&mut forged);
    assert!(matches!(
        validate_psi_optimization_unit(&forged),
        Err(OptimizationUnitValidationError::FunctionResultMismatch(_))
    ));
}

#[test]
fn independently_accepts_verified_context_and_frontier_coverage() {
    validate_verified_psi_optimization_unit(&verified_unit()).unwrap();
}
