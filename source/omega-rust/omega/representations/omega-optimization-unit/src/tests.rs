//! Optimization-unit model and reconstruction tests.

use super::*;
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractParameter, AbstractResult, ValueBinding,
};
use psi_core::{
    BoundaryMachineId, ContentPlaceVersion, DomainSemanticId, IntegerSign, IntegerType,
    IntegerValue, PlaceId, ServiceId, StructuralDomainId, StructuralTypeId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ByteSequenceCarrier, ProviderCandidateConformance,
    ProviderUnitRefinement, ProviderUnitSignature, SemanticFingerprint, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, VocabularyMarker,
};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}

fn plan() -> AbstractOperationPlan {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let value = id(3, ValueId::new);
    let result = id(4, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter {
                value,
                scalar_type: ScalarType::Integer(integer),
            }],
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(omega_abstract_operations::AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(integer),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(5, OperationId::new),
                    result,
                    scalar_type: ScalarType::Integer(integer),
                    value: IntegerValue::Unsigned(9),
                },
                AbstractOperation::Return {
                    psi_edge: id(6, EdgeId::new),
                    result,
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn write_only_store_plan() -> AbstractOperationPlan {
    let machine = id(70, MachineId::new);
    let block = id(71, BlockId::new);
    let value = id(72, ValueId::new);
    let place = id(73, PlaceId::new);
    let structural_type = id(74, StructuralTypeId::new);
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let destination = StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
    };
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([70; 32]),
        },
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter { value, scalar_type }],
            structural_parameters: vec![destination.clone()],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::WriteOnlyPrimitiveStore {
                    psi_operation: id(75, OperationId::new),
                    destination,
                    value: AbstractResult { value, scalar_type },
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: id(76, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

#[test]
fn write_only_store_identity_binds_destination_value_and_scalar_type() {
    let schedule = FuelScheduleIdentity::new(70).unwrap();
    let baseline = reconstruct_psi_optimization_unit_seed(&write_only_store_plan(), schedule)
        .expect("store plan reconstructs");

    let changed_identity = |mut unit: PsiOptimizationUnit| {
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit.identity
    };
    let mut destination_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut destination_drift.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("first node is the store")
    };
    destination.position = 1;
    assert_ne!(baseline.identity, changed_identity(destination_drift));

    let mut place_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut place_drift.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("first node is the store")
    };
    destination.place = id(78, PlaceId::new);
    assert_ne!(baseline.identity, changed_identity(place_drift));

    let mut value_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
        &mut value_drift.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("first node is the store")
    };
    value.value = id(77, ValueId::new);
    assert_ne!(baseline.identity, changed_identity(value_drift));

    let mut type_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
        &mut type_drift.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("first node is the store")
    };
    value.scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap());
    assert_ne!(baseline.identity, changed_identity(type_drift));
}

#[test]
fn rebuild_is_deterministic_and_keeps_distinct_fuel_sites() {
    let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
    let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
    let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.functions[0].blocks[0].nodes.len(), 2);
    assert_ne!(
        first.functions[0].blocks[0].nodes[0].fuel[0].site,
        first.functions[0].blocks[0].nodes[1].fuel[0].site
    );
    let source = plan();
    assert_eq!(first.structural_types, source.structural_types);
    assert_eq!(first.boundary_machines, source.boundary_machines);
    assert_eq!(first.provider_candidates, source.provider_candidates);
    assert!(first.accepted_obligation_facts.is_empty());
    assert!(first.proof_questions.is_empty());
    assert!(first.ownership_frontier_facts.is_empty());
    assert_eq!(
        first.functions[0].attachment,
        source.functions[0].attachment
    );
    assert_eq!(first.functions[0].result, source.functions[0].result);
    assert_eq!(
        first.functions[0].entry_claim_declarations,
        source.functions[0].entry_claims
    );
    assert_eq!(
        first.functions[0].published_service_ceiling,
        source.functions[0].published_service_ceiling
    );
}

#[test]
fn canonical_identity_is_content_recomputable_and_history_independent() {
    let schedule = FuelScheduleIdentity::new(1).expect("nonzero schedule");
    let first = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
    let second = reconstruct_psi_optimization_unit_seed(&plan(), schedule).unwrap();
    assert_eq!(
        recompute_psi_optimization_unit_identity(&first),
        recompute_psi_optimization_unit_identity(&second)
    );

    let mut different_stored_history = first.clone();
    different_stored_history.identity =
        OptimizationUnitIdentity::from_canonical_bytes(b"unrelated stored history");
    assert_eq!(
        recompute_psi_optimization_unit_identity(&first),
        recompute_psi_optimization_unit_identity(&different_stored_history)
    );
}

#[test]
fn canonical_identity_binds_every_retained_field_class() {
    let baseline = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let baseline_identity = recompute_psi_optimization_unit_identity(&baseline);
    let machine = baseline.functions[0].machine;
    let block = baseline.functions[0].blocks[0].id;
    let scalar_type = baseline.functions[0].parameters[0].scalar_type;
    let mut mutations = Vec::new();

    let mut unit = baseline.clone();
    unit.psi.program_fingerprint = SemanticFingerprint::from_bytes([8; 32]);
    mutations.push(("terminal identity", unit));
    let mut unit = baseline.clone();
    unit.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
    mutations.push(("fuel schedule", unit));
    let mut unit = baseline.clone();
    unit.entry = id(90, MachineId::new);
    mutations.push(("entry machine", unit));
    let structural_type = id(105, StructuralTypeId::new);
    let boundary = id(106, BoundaryMachineId::new);
    let mut unit = baseline.clone();
    unit.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "identity-test-structural-type".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    mutations.push(("module structural type", unit));
    let mut unit = baseline.clone();
    unit.structural_domains = Arc::from(vec![psi_terminal::StructuralDomainDeclaration {
        id: id(112, StructuralDomainId::new),
        semantic_domain: id(113, DomainSemanticId::new),
        identity: "identity-test-structural-domain".into(),
        carrier: structural_type,
        content_projection: None,
    }]);
    mutations.push(("module structural domain", unit));
    let mut unit = baseline.clone();
    unit.root_service_reach.concrete = vec![id(116, ServiceId::new)];
    mutations.push(("root concrete service reach", unit));
    let mut unit = baseline.clone();
    unit.root_service_reach.installation_dependencies =
        vec![psi_terminal::InstallationReachDependency {
            requirement_identity: "identity-test-installation-requirement".into(),
            upper_bound: vec![id(117, ServiceId::new)],
        }];
    mutations.push(("root installation service reach", unit));
    let mut unit = baseline.clone();
    unit.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "identity-test-boundary".into(),
        attachment: Some(structural_type),
        scalar_parameters: vec![ScalarType::Boolean],
        structural_parameters: Vec::new(),
        result: Some(ScalarType::Boolean),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: vec![id(107, ServiceId::new)],
    });
    mutations.push(("module boundary declaration", unit));
    let mut unit = baseline.clone();
    unit.provider_candidates.push(ProviderCandidateConformance {
        boundary,
        requirement_identity: "identity-test-requirement".into(),
        provider_identity: "identity-test-provider".into(),
        candidate_identity: "identity-test-candidate".into(),
        candidate: machine,
        signature: ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: vec![id(108, ServiceId::new)],
        },
    });
    mutations.push(("module provider candidate", unit));
    let mut unit = baseline.clone();
    unit.accepted_obligation_facts
        .push(AcceptedObligationFact::new(
            unit.psi,
            [4; 32],
            machine,
            id(5, OperationId::new),
            id(91, ObligationId::new),
            vec![1, 2, 3],
        ));
    mutations.push(("accepted fact", unit));
    let mut unit = baseline.clone();
    unit.proof_questions.push(ProofQuestion::new(
        unit.psi,
        [5; 32],
        ProofQuestionOwner::Operation {
            machine,
            operation: id(5, OperationId::new),
        },
        id(118, ObligationId::new),
        ProofQuestionClass::Derivable,
        vec![1, 2],
        vec![vec![3]],
        vec![vec![4]],
        true,
    ));
    mutations.push(("proof question", unit));
    let mut unit = baseline.clone();
    unit.ownership_frontier_facts
        .push(OwnershipFrontierFact::new(
            unit.psi,
            machine,
            OwnershipFrontierSite::BlockEntry(block),
            OwnershipFrontierSnapshot {
                claims: Vec::new(),
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        ));
    mutations.push(("ownership frontier fact", unit));
    let mut unit = baseline.clone();
    unit.pruned_machines.push(PrunedMachineCustody {
        machine: id(109, MachineId::new),
        source_ordinal: 1,
    });
    mutations.push(("pruned machine custody", unit));
    let mut unit = baseline.clone();
    unit.functions[0].machine = id(92, MachineId::new);
    mutations.push(("function identity", unit));
    let mut unit = baseline.clone();
    unit.functions[0].attachment = Some(structural_type);
    mutations.push(("function attachment", unit));
    let mut unit = baseline.clone();
    unit.functions[0].parameters[0].value = id(93, ValueId::new);
    mutations.push(("scalar parameter", unit));
    let mut unit = baseline.clone();
    unit.functions[0]
        .structural_parameters
        .push(psi_terminal::StructuralParameterDeclaration {
            place: id(94, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: id(95, psi_core::StructuralTypeId::new),
            multiplicity: psi_terminal::StructuralMultiplicity::Affine,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    mutations.push(("structural parameter", unit));
    let mut unit = baseline.clone();
    let structural_place = id(114, PlaceId::new);
    unit.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: structural_place,
            kind: StructuralPlaceKind::Result,
        });
    mutations.push(("structural place declaration", unit));
    let mut unit = baseline.clone();
    unit.functions[0]
        .content_entry_claims
        .push(psi_terminal::ContentEntryClaim {
            claim: id(115, ClaimId::new),
            input: psi_core::ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: structural_place,
                segments: Vec::new(),
            },
            projections: Vec::new(),
        });
    mutations.push(("content entry claim", unit));
    let mut unit = baseline.clone();
    unit.functions[0].result = AbstractFunctionResult::Unit;
    mutations.push(("function result signature", unit));
    let mut unit = baseline.clone();
    unit.functions[0]
        .declared_places
        .insert(id(96, PlaceId::new));
    mutations.push(("declared place", unit));
    let mut unit = baseline.clone();
    unit.functions[0].entry_claim_declarations.push(EntryClaim {
        claim: id(109, ClaimId::new),
        input: id(110, PlaceId::new),
        path: Vec::new(),
    });
    mutations.push(("entry claim declaration", unit));
    let mut unit = baseline.clone();
    unit.functions[0].entry_claims.insert(id(97, ClaimId::new));
    mutations.push(("entry claim", unit));
    let mut unit = baseline.clone();
    unit.functions[0]
        .published_service_ceiling
        .push(id(111, ServiceId::new));
    mutations.push(("function service ceiling", unit));
    let mut unit = baseline.clone();
    unit.functions[0].facts.clear();
    mutations.push(("optimization fact", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].id = id(98, BlockId::new);
    mutations.push(("block", unit));
    let mut unit = baseline.clone();
    let AbstractOperation::IntegerConstant { value, .. } =
        &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(10);
    mutations.push(("operation payload", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0].provenance[0] =
        PsiProvenance::Operation(id(99, OperationId::new));
    mutations.push(("provenance", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0].fuel[0].units = 2;
    mutations.push(("fuel settlement", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0].effect.output = 77;
    mutations.push(("effect", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0].definitions[0].scalar_type = ScalarType::Boolean;
    mutations.push(("definition", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[1].uses[0].value = id(100, ValueId::new);
    mutations.push(("use", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0]
        .successors
        .push(OptimizationEdge {
            psi_edge: id(101, EdgeId::new),
            target: block,
            bindings: vec![ValueBinding {
                parameter: id(102, ValueId::new),
                argument: id(103, ValueId::new),
                scalar_type,
            }],
            trivial_affine_discards: Vec::new(),
            provenance: vec![PsiProvenance::Edge(id(101, EdgeId::new))],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(id(101, EdgeId::new)),
                units: 1,
            }],
        });
    mutations.push(("successor", unit));
    let mut unit = baseline.clone();
    unit.functions[0].blocks[0].nodes[0]
        .ownership
        .push(OwnershipEvent::ClaimTransfer(vec![id(104, ClaimId::new)]));
    mutations.push(("ownership", unit));

    for (field_class, unit) in mutations {
        assert_ne!(
            recompute_psi_optimization_unit_identity(&unit),
            baseline_identity,
            "{field_class} must contribute to canonical content identity"
        );
    }
}

#[test]
fn proof_question_attachment_preserves_order_and_rejects_forgery_or_duplicates() {
    let seed = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let machine = seed.functions[0].machine;
    let first = ProofQuestion::new(
        seed.psi,
        [5; 32],
        ProofQuestionOwner::Operation {
            machine,
            operation: id(5, OperationId::new),
        },
        id(118, ObligationId::new),
        ProofQuestionClass::Derivable,
        vec![1],
        vec![vec![2]],
        vec![vec![3]],
        true,
    );
    let second = ProofQuestion::new(
        seed.psi,
        [5; 32],
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract: id(119, ContractId::new),
            clause_position: 0,
        },
        id(120, ObligationId::new),
        ProofQuestionClass::AdmissionAuthorized {
            site: id(121, AdmissionSiteId::new),
            kind: ProofQuestionAdmissionKind::ProviderFact,
            authority_identity: id(122, EvidenceIdentity::new),
        },
        vec![4],
        vec![vec![5], vec![6]],
        vec![vec![7]],
        false,
    );
    let attached = attach_proof_questions(seed.clone(), vec![second.clone(), first.clone()])
        .expect("verifier order is retained, not sorted");
    assert_eq!(
        attached.proof_questions,
        vec![second.clone(), first.clone()]
    );
    assert_eq!(
        attach_proof_questions(attached, Vec::new()),
        Err(ProofQuestionIndexError::AlreadyAttached)
    );
    assert_eq!(
        attach_proof_questions(seed.clone(), vec![first.clone(), first]),
        Err(ProofQuestionIndexError::DuplicateQuestion)
    );
    let mut forged = second;
    forged.semantic_axioms.push(vec![8]);
    assert_eq!(
        attach_proof_questions(seed, vec![forged]),
        Err(ProofQuestionIndexError::InvalidQuestionIdentity)
    );
}

#[test]
fn ownership_frontier_attachment_is_canonical_and_single_use() {
    let seed = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let machine = seed.functions[0].machine;
    let block = seed.functions[0].entry;
    let empty = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: Vec::new(),
        partial_custody: Vec::new(),
    };
    let block_fact = OwnershipFrontierFact::new(
        seed.psi,
        machine,
        OwnershipFrontierSite::BlockEntry(block),
        empty.clone(),
    );
    let edge_fact = OwnershipFrontierFact::new(
        seed.psi,
        machine,
        OwnershipFrontierSite::EdgeEntry(id(6, EdgeId::new)),
        empty,
    );
    assert_eq!(
        attach_ownership_frontier_facts(seed.clone(), vec![edge_fact.clone(), block_fact.clone()]),
        Err(OwnershipFrontierFactIndexError::NonCanonicalOrder)
    );
    let place = id(20, PlaceId::new);
    let duplicate_place_snapshot = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![
            OwnershipFrontierOwnedPlace {
                place,
                multiplicity: StructuralMultiplicity::Affine,
            },
            OwnershipFrontierOwnedPlace {
                place,
                multiplicity: StructuralMultiplicity::Affine,
            },
        ],
        partial_custody: Vec::new(),
    };
    assert_eq!(
        attach_ownership_frontier_facts(
            seed.clone(),
            vec![OwnershipFrontierFact::new(
                seed.psi,
                machine,
                OwnershipFrontierSite::BlockEntry(block),
                duplicate_place_snapshot,
            )],
        ),
        Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot)
    );

    let attached =
        attach_ownership_frontier_facts(seed.clone(), vec![block_fact.clone(), edge_fact.clone()])
            .unwrap();
    let replay = attach_ownership_frontier_facts(seed, vec![block_fact, edge_fact]).unwrap();
    assert_eq!(attached, replay);
    assert_eq!(
        attach_ownership_frontier_facts(attached, Vec::new()),
        Err(OwnershipFrontierFactIndexError::AlreadyAttached)
    );
}

#[test]
fn observation_projection_keeps_external_events_and_semantic_accounting() {
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let observations = reconstruct_psi_observation_model(&unit);

    assert_eq!(observations.revision, unit.identity);
    assert_eq!(observations.nodes.len(), 2);
    assert!(observations.nodes[0].events.is_empty());
    assert_eq!(observations.nodes[0].crash, ObservationKnowledge::No);
    assert_eq!(observations.nodes[0].provenance.len(), 1);
    assert_eq!(observations.nodes[0].fuel.len(), 1);
    assert_eq!(observations.nodes[1].events.len(), 1);
    assert_eq!(
        observations.nodes[1].events[0].class,
        ObservationEventClass::NormalExit
    );
    assert!(matches!(
        observations.nodes[1].events[0].operation,
        AbstractOperation::Return { .. }
    ));
}

#[test]
fn block_parameters_keep_terminal_declaration_order() {
    let mut plan = plan();
    let function = &mut plan.functions[0];
    let entry = function.entry;
    let target = id(20, BlockId::new);
    // Deliberately descending identities prove this is declaration order,
    // not the previous BTreeMap order.
    let first_parameter = id(90, ValueId::new);
    let second_parameter = id(80, ValueId::new);
    let first_argument = function.parameters[0].value;
    let second_argument = id(70, ValueId::new);
    let scalar_type = function.parameters[0].scalar_type;
    function.parameters.push(AbstractParameter {
        value: second_argument,
        scalar_type,
    });
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: first_parameter,
        scalar_type,
    });
    function.block_entries = vec![
        AbstractBlockEntry {
            block: entry,
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            block: target,
            parameters: vec![
                AbstractParameter {
                    value: first_parameter,
                    scalar_type,
                },
                AbstractParameter {
                    value: second_parameter,
                    scalar_type,
                },
            ],
            operation_offset: 1,
        },
    ];
    function.operations = vec![
        AbstractOperation::Jump {
            psi_edge: id(60, EdgeId::new),
            target,
            bindings: vec![
                ValueBinding {
                    parameter: first_parameter,
                    argument: first_argument,
                    scalar_type,
                },
                ValueBinding {
                    parameter: second_parameter,
                    argument: second_argument,
                    scalar_type,
                },
            ],
            trivial_affine_discards: Vec::new(),
        },
        AbstractOperation::Return {
            psi_edge: id(61, EdgeId::new),
            result: first_parameter,
            value: first_parameter,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];

    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("ordered block parameters");
    assert_eq!(
        unit.functions[0].blocks[1]
            .parameters
            .iter()
            .map(|parameter| parameter.value)
            .collect::<Vec<_>>(),
        vec![first_parameter, second_parameter]
    );
}

#[test]
fn value_range_identity_rejects_malformed_type_support_and_region_axes() {
    let revision = OptimizationUnitIdentity::from_canonical_bytes(b"range revision");
    let machine = id(91, MachineId::new);
    let block = id(92, BlockId::new);
    let value = id(93, ValueId::new);
    let other_value = id(94, ValueId::new);
    let operation = id(95, OperationId::new);
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let support = ValueRangeSupport::ScalarConstant(
        ScalarConstantFactIdentity::from_canonical_bytes(b"range constant"),
    );
    let entire = ValueRangeRegion {
        revision,
        machine,
        value,
        scope: ValueRangeScope::EntireValue,
        dominated_blocks: Vec::new(),
    };
    let baseline = value_range_fact_identity(
        value,
        scalar_type,
        IntegerValue::Unsigned(1),
        IntegerValue::Unsigned(7),
        &support,
        &entire,
    )
    .expect("well-formed range identity");
    assert_ne!(
        baseline,
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(8),
            &support,
            &entire,
        )
        .unwrap()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                value: other_value,
                ..entire.clone()
            },
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(8),
            IntegerValue::Unsigned(7),
            &support,
            &entire,
        )
        .is_none()
    );
    assert!(
        value_range_fact_identity(
            value,
            scalar_type,
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(7),
            &support,
            &ValueRangeRegion {
                scope: ValueRangeScope::DominatedOperationEntry {
                    block,
                    node: 1,
                    operation,
                },
                dominated_blocks: vec![block],
                ..entire.clone()
            },
        )
        .is_none()
    );
}
