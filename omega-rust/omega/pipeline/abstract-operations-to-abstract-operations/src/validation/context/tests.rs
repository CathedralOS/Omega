//! Verified context, identity, frontier, and proof-catalog tests.

use super::*;
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractResult};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::VocabularyMarker;

#[test]
fn recomputed_immutable_signature_forgery_is_rejected_by_verified_context() {
    let verified = verified_unit();
    let structural_type = id(120, semantic_vocabulary::StructuralTypeId::new);
    let boundary = id(121, semantic_vocabulary::BoundaryMachineId::new);
    let service = id(122, semantic_vocabulary::ServiceId::new);
    let mut forged = Vec::new();

    let mut unit = verified.unit().clone();
    unit.structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "forged-structural-type".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        });
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: "forged-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.provider_candidates
        .push(terminal_psi::ProviderCandidateConformance {
            boundary,
            requirement_identity: "forged-requirement".into(),
            provider_identity: "forged-provider".into(),
            candidate_identity: "forged-candidate".into(),
            candidate: unit.functions[0].machine,
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
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
    unit.services = vec![terminal_psi::ServiceDeclaration {
        id: service,
        identity: "forged-service".into(),
        parents: Vec::new(),
    }]
    .into();
    forged.push(unit);

    let mut unit = verified.unit().clone();
    unit.services = vec![terminal_psi::ServiceDeclaration {
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
        .push(terminal_psi::EntryClaim {
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
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
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
        contract: id(104, semantic_vocabulary::ContractId::new),
        clause_position: 7,
    };
    corruptions.push(owner);
    let mut obligation = original.clone();
    obligation.proof_questions[0].obligation = id(107, semantic_vocabulary::ObligationId::new);
    corruptions.push(obligation);
    let mut class = original.clone();
    class.proof_questions[0].class = ProofQuestionClass::AdmissionAuthorized {
        site: id(108, semantic_vocabulary::AdmissionSiteId::new),
        kind: ProofQuestionAdmissionKind::CheckedAssemblyClaim,
        authority_identity: id(109, semantic_vocabulary::EvidenceIdentity::new),
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

fn refresh_identity(unit: &mut PsiOptimizationUnit) {
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

fn refresh_proof_question_identity(question: &mut ProofQuestion) {
    question.identity = optimization_unit::proof_question_identity(
        question.terminal_psi,
        question.proof_bundle_fingerprint,
        question.owner,
        question.obligation,
        question.class,
        &question.proposition,
        &question.requirements,
        &question.semantic_axioms,
        question.canonical_certificate,
    );
}

fn verified_unit() -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    use terminal_psi::{
        Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator,
    };

    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(101, MachineId::new),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: id(101, MachineId::new),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id(102, BlockId::new),
            blocks: vec![Block {
                id: id(102, BlockId::new),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id(103, EdgeId::new),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: id(104, semantic_vocabulary::ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![
                    ContractClause {
                        obligation: id(105, semantic_vocabulary::ObligationId::new),
                        proposition: Proposition::Truth,
                    },
                    ContractClause {
                        obligation: id(106, semantic_vocabulary::ObligationId::new),
                        proposition: Proposition::Truth,
                    },
                ],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = terminal_verifier::ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: [105, 106]
            .into_iter()
            .map(|obligation| terminal_verifier::ObligationEvidence {
                obligation: id(obligation, semantic_vocabulary::ObligationId::new),
                route: proof_admission::EvidenceRoute::KernelDerived(
                    proof_admission::PrimitiveJudgment::Truth,
                ),
            })
            .collect(),
    };
    let semantic = terminal_codec::encode_module(&module).expect("encode unit module");
    let proof = terminal_codec::encode_proof_bundle(&proof).expect("encode empty proof");
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified optimizer input");
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("verified optimizer unit")
}

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}
