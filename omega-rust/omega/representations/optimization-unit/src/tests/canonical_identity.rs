use std::sync::Arc;

use super::fixtures::{id, plan, write_only_store_plan};
use crate::{
    AcceptedObligationFact, FuelSettlement, OptimizationEdge, OptimizationUnitIdentity,
    OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    ProofQuestion, ProofQuestionClass, ProofQuestionOwner, PrunedMachineCustody,
    PsiOptimizationUnit, PsiProvenance, StructuralPlaceKind,
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use abstract_operations::{AbstractFunctionResult, AbstractOperation, ValueBinding};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContentPlaceVersion, DomainSemanticId, EdgeId,
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ByteSequenceCarrier, CrashCause, CrashRouteBucket, CrashRouteGuard,
    EntryClaim, ProviderCandidateConformance, ProviderRefinement, ProviderSignature,
    SemanticFingerprint, StructuralTypeDeclaration, StructuralTypeShape,
};

#[test]
fn float_comparison_identity_binds_relation_format_operands_and_provenance() {
    use semantic_vocabulary::{IeeeFloatComparisonOperation as Comparison, IeeeFloatFormat};
    let mut source = plan();
    source.functions[0].operations[0] = AbstractOperation::IeeeFloatCompare {
        psi_operation: id(5, OperationId::new),
        result: id(4, ValueId::new),
        comparison: Comparison::Less,
        format: IeeeFloatFormat::Binary32,
        left: id(3, ValueId::new),
        right: id(8, ValueId::new),
    };
    let seed =
        reconstruct_psi_optimization_unit_seed(&source, FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let original = recompute_psi_optimization_unit_identity(&seed);
    for mutation in 0..5 {
        let mut changed = seed.clone();
        let AbstractOperation::IeeeFloatCompare {
            psi_operation,
            comparison,
            format,
            left,
            right,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("comparison");
        };
        match mutation {
            0 => *comparison = Comparison::Greater,
            1 => *format = IeeeFloatFormat::Binary64,
            2 => std::mem::swap(left, right),
            3 => *right = *left,
            4 => *psi_operation = id(9, OperationId::new),
            _ => unreachable!(),
        }
        assert_ne!(
            original,
            recompute_psi_optimization_unit_identity(&changed),
            "mutation {mutation}"
        );
    }
}

#[test]
fn jump_residual_identity_binds_both_operation_and_derived_edge() {
    let mut source_plan = plan();
    let residuals = vec![
        terminal_psi::StructuralAffineDiscard {
            place: id(201, PlaceId::new),
            path: vec![terminal_psi::StructuralPathSegment::Field("right".into())],
            structural_type: id(202, StructuralTypeId::new),
        },
        terminal_psi::StructuralAffineDiscard {
            place: id(201, PlaceId::new),
            path: vec![terminal_psi::StructuralPathSegment::Field("left".into())],
            structural_type: id(203, StructuralTypeId::new),
        },
    ];
    source_plan.functions[0].operations[0] = AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(204, EdgeId::new),
        target: id(205, BlockId::new),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: residuals.clone(),
    };
    let baseline =
        reconstruct_psi_optimization_unit_seed(&source_plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let identity = recompute_psi_optimization_unit_identity(&baseline);
    for derived in [false, true] {
        for mutation in 0..5 {
            let mut changed = baseline.clone();
            let node = &mut changed.functions[0].blocks[0].nodes[0];
            let rows = if derived {
                &mut node.successors[0].residual_affine_discards
            } else {
                let AbstractOperation::Jump {
                    residual_affine_discards,
                    ..
                } = &mut node.operation
                else {
                    unreachable!()
                };
                residual_affine_discards
            };
            match mutation {
                0 => rows.clear(),
                1 => rows.reverse(),
                2 => rows[0].place = id(206, PlaceId::new),
                3 => rows[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(2)],
                4 => rows[0].structural_type = id(207, StructuralTypeId::new),
                _ => unreachable!(),
            }
            assert_ne!(
                identity,
                recompute_psi_optimization_unit_identity(&changed),
                "derived={derived}, mutation={mutation}"
            );
        }
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
fn call_identity_binds_requirement_and_crash_rosters_independently() {
    let mut baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::Call {
        psi_operation: id(1, OperationId::new),
        result: id(1, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        callee: id(2, MachineId::new),
        arguments: Vec::new(),
        requirement_obligations: vec![id(1, ObligationId::new)],
        crash_continuations: vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }],
    };
    baseline.identity = recompute_psi_optimization_unit_identity(&baseline);

    let mut requirement_drift = baseline.clone();
    let AbstractOperation::Call {
        requirement_obligations,
        ..
    } = &mut requirement_drift.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    requirement_obligations[0] = id(2, ObligationId::new);
    requirement_drift.identity = recompute_psi_optimization_unit_identity(&requirement_drift);
    assert_ne!(baseline.identity, requirement_drift.identity);

    let mut crash_drift = baseline.clone();
    let AbstractOperation::Call {
        crash_continuations,
        ..
    } = &mut crash_drift.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    crash_continuations[0].cause = CrashCause::Abort;
    crash_drift.identity = recompute_psi_optimization_unit_identity(&crash_drift);
    assert_ne!(baseline.identity, crash_drift.identity);
}

#[test]
fn unit_call_identity_binds_scalar_arguments() {
    let mut baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallUnit {
        psi_operation: id(1, OperationId::new),
        callee: id(2, MachineId::new),
        arguments: vec![id(3, ValueId::new)],
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    baseline.identity = recompute_psi_optimization_unit_identity(&baseline);

    let mut changed = baseline.clone();
    let AbstractOperation::CallUnit { arguments, .. } =
        &mut changed.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    arguments[0] = id(4, ValueId::new);
    changed.identity = recompute_psi_optimization_unit_identity(&changed);
    assert_ne!(baseline.identity, changed.identity);
}

#[test]
fn canonical_operation_identity_bytes_are_stable() {
    let scalar = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let structural = reconstruct_psi_optimization_unit_seed(
        &write_only_store_plan(),
        FuelScheduleIdentity::new(70).expect("nonzero schedule"),
    )
    .unwrap();
    assert_eq!(
        scalar.identity.bytes(),
        [
            83, 181, 16, 142, 109, 25, 184, 105, 73, 190, 207, 17, 180, 245, 221, 230, 185, 24, 18,
            154, 148, 194, 92, 10, 87, 241, 175, 73, 247, 194, 68, 216,
        ],
        "identity binds vocabulary 99 and scalar-case payloads",
    );
    assert_eq!(
        structural.identity.bytes(),
        [
            29, 68, 182, 16, 91, 74, 8, 194, 13, 108, 80, 89, 146, 164, 38, 29, 184, 139, 105, 212,
            64, 110, 62, 73, 141, 62, 31, 78, 202, 229, 197, 150,
        ],
        "identity binds vocabulary 99 alongside unchanged storage and return tags",
    );
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
fn invariant_question_coordinates_each_bind_the_unit_identity() {
    let mut unit =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let machine = unit.functions[0].machine;
    let header = id(201, BlockId::new);
    let parameter = id(202, ValueId::new);
    let edge = id(203, EdgeId::new);
    unit.proof_questions.push(ProofQuestion::new(
        unit.psi,
        [5; 32],
        ProofQuestionOwner::ScalarRangeInvariant {
            machine,
            header,
            parameter,
            edge,
        },
        id(204, ObligationId::new),
        ProofQuestionClass::Derivable,
        vec![1],
        Vec::new(),
        Vec::new(),
        false,
    ));
    let baseline = recompute_psi_optimization_unit_identity(&unit);
    for owner in [
        ProofQuestionOwner::ScalarRangeInvariant {
            machine: id(211, MachineId::new),
            header,
            parameter,
            edge,
        },
        ProofQuestionOwner::ScalarRangeInvariant {
            machine,
            header: id(212, BlockId::new),
            parameter,
            edge,
        },
        ProofQuestionOwner::ScalarRangeInvariant {
            machine,
            header,
            parameter: id(213, ValueId::new),
            edge,
        },
        ProofQuestionOwner::ScalarRangeInvariant {
            machine,
            header,
            parameter,
            edge: id(214, EdgeId::new),
        },
    ] {
        let mut changed = unit.clone();
        // Retain the old question digest to check the unit's own owner encoding.
        changed.proof_questions[0].owner = owner;
        assert_ne!(recompute_psi_optimization_unit_identity(&changed), baseline);
    }
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
    unit.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "identity-test-structural-type".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    mutations.push(("module structural type", unit));
    let mut unit = baseline.clone();
    unit.structural_domains = Arc::from(vec![terminal_psi::StructuralDomainDeclaration {
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
        vec![terminal_psi::InstallationReachDependency {
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
        result: terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Boolean),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: vec![id(107, ServiceId::new)],
        crash_routes: Vec::new(),
    });
    mutations.push(("module boundary declaration", unit));
    let mut unit = baseline.clone();
    unit.provider_candidates.push(ProviderCandidateConformance {
        boundary,
        requirement_identity: "identity-test-requirement".into(),
        provider_identity: "identity-test-provider".into(),
        candidate_identity: "identity-test-candidate".into(),
        candidate: machine,
        signature: ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderRefinement {
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
        .push(terminal_psi::StructuralParameterDeclaration {
            place: id(94, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: id(95, semantic_vocabulary::StructuralTypeId::new),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    mutations.push(("structural parameter", unit));
    let mut unit = baseline.clone();
    let structural_place = id(114, PlaceId::new);
    unit.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: structural_place,
            kind: StructuralPlaceKind::Result,
        });
    mutations.push(("structural place declaration", unit));
    let mut unit = baseline.clone();
    unit.functions[0]
        .content_entry_claims
        .push(terminal_psi::ContentEntryClaim {
            claim: id(115, ClaimId::new),
            input: semantic_vocabulary::ContentStructuralPlace {
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
            structural_bindings: Vec::new(),
            psi_edge: id(101, EdgeId::new),
            target: block,
            bindings: vec![ValueBinding {
                parameter: id(102, ValueId::new),
                argument: id(103, ValueId::new),
                scalar_type,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
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
