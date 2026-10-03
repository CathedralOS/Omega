use std::sync::Arc;

use super::fixtures::{id, plan, write_only_store_plan};
use crate::{
    AcceptedObligationFact, FuelSettlement, OptimizationEdge, OptimizationUnitIdentity,
    OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    ProofQuestion, ProofQuestionClass, ProofQuestionOwner, PrunedMachineCustody,
    PsiOptimizationUnit, PsiProvenance, StructuralPlaceKind, ValueDefinitionSite,
    encode_value_definition_site_identity, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed, structural_domain_catalog_identity,
};
use abstract_operations::{
    AbstractDynamicCallResult, AbstractDynamicDispatch, AbstractFunctionResult, AbstractOperation,
    AbstractStructuralCallResult, ValueBinding,
};
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
fn fixed_byte_range_call_identity_binds_both_endpoints() {
    let mut baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallStructural {
        psi_operation: id(1, OperationId::new),
        callee: id(2, MachineId::new),
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: id(1, PlaceId::new),
            access: terminal_psi::StructuralAccess::MutableBorrow,
            path: vec![terminal_psi::StructuralPathSegment::FixedByteRange { start: 0, end: 2 }],
        }],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
    };
    let original = recompute_psi_optimization_unit_identity(&baseline);
    for (start, end) in [(1, 2), (0, 3), (1, 3)] {
        let mut changed = baseline.clone();
        let AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        structural_arguments[0].path[0] =
            terminal_psi::StructuralPathSegment::FixedByteRange { start, end };
        let identity = recompute_psi_optimization_unit_identity(&changed);
        assert_ne!(original, identity, "range {start}..{end}");
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&changed);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            identity
        );
    }
}

#[test]
fn fixed_byte_range_frontier_identity_binds_both_endpoints() {
    let unit =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let original = OwnershipFrontierFact::new(
        unit.psi,
        unit.functions[0].machine,
        OwnershipFrontierSite::BlockEntry(unit.functions[0].blocks[0].id),
        OwnershipFrontierSnapshot {
            claims: vec![crate::OwnershipFrontierLiveClaim {
                claim: id(1, ClaimId::new),
                input: Some(id(1, PlaceId::new)),
                path: vec![terminal_psi::StructuralPathSegment::FixedByteRange {
                    start: 0,
                    end: 2,
                }],
                multiplicity: None,
            }],
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        },
    );
    assert!(original.has_canonical_identity());
    for (start, end) in [(1, 2), (0, 3), (1, 3)] {
        let mut changed = original.clone();
        changed.snapshot.claims[0].path[0] =
            terminal_psi::StructuralPathSegment::FixedByteRange { start, end };
        assert!(!changed.has_canonical_identity(), "range {start}..{end}");
    }
}

#[test]
fn boundary_identity_binds_completion_progress_kind_and_fixed_reach() {
    let source = plan();
    let mut unit =
        reconstruct_psi_optimization_unit_seed(&source, FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let service = id(1, ServiceId::new);
    unit.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: id(1, BoundaryMachineId::new),
        identity: "Installer::step".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![service],
        crash_routes: Vec::new(),
    });
    let original = recompute_psi_optimization_unit_identity(&unit);
    let published = unit.boundary_machines[0].published_service_ceiling.clone();
    for (completion, unconditional_termination) in [
        (terminal_psi::BoundaryCompletion::NormalReturn, true),
        (
            terminal_psi::BoundaryCompletion::External(
                terminal_psi::ExternalCompletionKind::ProcessExit,
            ),
            false,
        ),
    ] {
        let mut changed = unit.clone();
        changed.boundary_machines[0].completion = completion;
        changed.boundary_machines[0].unconditional_termination = unconditional_termination;
        let identity = recompute_psi_optimization_unit_identity(&changed);
        assert_ne!(
            original, identity,
            "{completion:?}, finite={unconditional_termination}"
        );
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&changed);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            identity
        );
    }
    unit.boundary_machines[0].kind = terminal_psi::BoundaryRequirementKind::Operator;
    assert_ne!(
        original,
        recompute_psi_optimization_unit_identity(&unit),
        "equal signatures and attachment absence do not erase the requirement kind"
    );
    unit.boundary_machines[0].kind = terminal_psi::BoundaryRequirementKind::Trait;
    unit.boundary_machines[0].fixed_service_reach.push(service);
    assert_eq!(
        unit.boundary_machines[0].published_service_ceiling,
        published
    );
    assert_ne!(original, recompute_psi_optimization_unit_identity(&unit));
}

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
    let abstract_operations::StructuralScalarStoreDestination::Parameter(destination) = destination
    else {
        panic!("parameter destination");
    };
    destination.position = 1;
    assert_ne!(baseline.identity, changed_identity(destination_drift));

    let mut place_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut place_drift.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("first node is the store")
    };
    let abstract_operations::StructuralScalarStoreDestination::Parameter(destination) = destination
    else {
        panic!("parameter destination");
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

    let mut previous = baseline.identity;
    for element in [0, 1, u64::MAX] {
        let mut projected = baseline.clone();
        let AbstractOperation::WriteOnlyPrimitiveStore { path, .. } =
            &mut projected.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("store");
        };
        *path = vec![terminal_psi::StructuralPathSegment::FixedIndex(element)];
        let identity = changed_identity(projected);
        assert_ne!(baseline.identity, identity);
        assert_ne!(previous, identity);
        previous = identity;
    }
}

#[test]
fn owned_block_store_identity_binds_its_block_declaration_and_variant() {
    use abstract_operations::StructuralScalarStoreDestination as Destination;

    let schedule = FuelScheduleIdentity::new(70).unwrap();
    let baseline = reconstruct_psi_optimization_unit_seed(&write_only_store_plan(), schedule)
        .expect("store plan reconstructs");
    let with_destination = |edit: &dyn Fn(&mut Destination)| {
        let mut unit = baseline.clone();
        let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
            &mut unit.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("first node is the store")
        };
        edit(destination);
        recompute_psi_optimization_unit_identity(&unit)
    };
    let owned = |block: u64, place: u64| {
        move |destination: &mut Destination| {
            let Destination::Parameter(parameter) = destination else {
                panic!("parameter destination");
            };
            let mut parameter = parameter.clone();
            parameter.access = terminal_psi::StructuralAccess::Owned;
            parameter.place = id(place, PlaceId::new);
            *destination = Destination::OwnedBlockParameter {
                block: id(block, BlockId::new),
                parameter,
            };
        }
    };
    let arrival = with_destination(&owned(71, 73));
    assert_ne!(baseline.identity, arrival);
    // Another block's arrival, or another place, is another destination.
    assert_ne!(arrival, with_destination(&owned(79, 73)));
    assert_ne!(arrival, with_destination(&owned(71, 78)));
    // The same owned declaration retagged as a loan keeps its own identity.
    let retagged = with_destination(&|destination: &mut Destination| {
        let Destination::Parameter(parameter) = destination else {
            panic!("parameter destination");
        };
        parameter.access = terminal_psi::StructuralAccess::Owned;
    });
    assert_ne!(arrival, retagged);
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
    baseline.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallStructural {
        psi_operation: id(1, OperationId::new),
        callee: id(2, MachineId::new),
        arguments: vec![id(3, ValueId::new)],
        structural_arguments: Vec::new(),
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
    };
    baseline.identity = recompute_psi_optimization_unit_identity(&baseline);

    let mut changed = baseline.clone();
    let AbstractOperation::CallStructural {
        arguments,
        result: AbstractStructuralCallResult::Unit,
        ..
    } = &mut changed.functions[0].blocks[0].nodes[0].operation
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
    for unit in [&scalar, &structural] {
        let bytes = crate::optimization_unit::identity::collect_unit_canonical_bytes(unit);
        let prefix = b"omega.psi-optimization-unit-content.v33\0";
        assert_eq!(&bytes[..prefix.len()], prefix);
        assert_eq!(
            &bytes[prefix.len()..prefix.len() + 2],
            &117_u16.to_le_bytes()
        );
    }
    assert_eq!(
        (scalar.identity.bytes(), structural.identity.bytes()),
        (
            [
                113, 104, 86, 147, 52, 213, 97, 243, 162, 238, 244, 44, 208, 30, 8, 134, 246, 100,
                81, 184, 176, 71, 182, 40, 181, 12, 112, 221, 78, 254, 183, 239
            ],
            [
                58, 199, 106, 53, 160, 76, 154, 14, 120, 204, 119, 158, 58, 214, 82, 203, 220, 242,
                106, 7, 97, 195, 38, 51, 92, 81, 64, 112, 80, 137, 29, 39
            ]
        ),
        "identities bind vocabulary 117 and unit schema 33 alongside unchanged operation tags",
    );
}

fn descriptor_selection(ordinal: u32) -> terminal_psi::TerminalDynamicConformanceSelection {
    terminal_psi::TerminalDynamicConformanceSelection {
        owner: id(1, MachineId::new),
        ordinal,
        source: terminal_psi::StructuralArgument {
            place: id(11, PlaceId::new),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: Vec::new(),
        },
        conformance_application_report_fingerprint: 17,
        conformance_application_commitment:
            terminal_psi::ClosedConformanceApplicationCommitment::from_digest([9; 32]),
    }
}

fn descriptor_application(
    subject: &str,
    result: terminal_psi::ClosedConformanceCallableResult,
) -> terminal_psi::ClosedConformanceApplication {
    terminal_psi::ClosedConformanceApplication {
        owner: id(1, MachineId::new),
        declaration_identity: format!("test::{subject}ImplementsScanner"),
        telescope: Vec::new(),
        subject_identity: Some(format!("test::{subject}")),
        trait_identity: "test::Scanner".to_owned(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: format!("test::{subject}::scan::callable"),
            machine: id(92, MachineId::new),
            result,
        }],
        rows: vec![terminal_psi::ClosedConformanceRow {
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec!["named(integer-const(8))".to_owned()],
            requirement_identity: "test::Scanner::scan".to_owned(),
            realization_identity: format!("test::{subject}::scan"),
            realization_callable_identity: Some(format!("test::{subject}::scan::callable")),
        }],
        report_fingerprint: 17,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::from_digest([9; 32]),
    }
}

/// A rebound descriptor whose initializer and latest applications differ, so
/// the optional initial-application arm is part of the pinned stream.
fn rebound_descriptor_dispatch(
    result: terminal_psi::ClosedConformanceCallableResult,
) -> abstract_operations::AbstractReboundDynamicDispatch {
    abstract_operations::AbstractReboundDynamicDispatch {
        initial: descriptor_selection(0),
        rebound: descriptor_selection(1),
        descriptor: terminal_psi::TerminalReboundDynamicDescriptor {
            owner: id(1, MachineId::new),
            ordinal: 3,
            initial_selection_ordinal: 0,
            rebound_selection_ordinal: 1,
        },
        initial_application: descriptor_application("Initial", result),
        application: descriptor_application("Latest", result),
        dispatch: terminal_psi::TerminalIndirectDynamicDispatch {
            owner: id(1, MachineId::new),
            operation: id(1, OperationId::new),
            descriptor_ordinal: 3,
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec!["named(integer-const(8))".to_owned()],
            requirement_identity: "test::Scanner::scan".to_owned(),
            realization_identity: "test::Latest::scan".to_owned(),
            realization_callable_identity: "test::Latest::scan::callable".to_owned(),
            realization: id(92, MachineId::new),
        },
    }
}

fn stored_descriptor_dispatch() -> abstract_operations::AbstractStoredDynamicDispatch {
    abstract_operations::AbstractStoredDynamicDispatch {
        stored: abstract_operations::AbstractStoredDynamicDescriptor {
            selection: descriptor_selection(2),
            descriptor: terminal_psi::TerminalStoredDynamicDescriptor {
                owner: id(1, MachineId::new),
                ordinal: 4,
                establishment_operation: id(6, OperationId::new),
                selection_ordinal: 2,
                aggregate_type_identity: "test::Holder".to_owned(),
                field_identity: "test::Holder::scanner".to_owned(),
            },
            application: descriptor_application(
                "Stored",
                terminal_psi::ClosedConformanceCallableResult::I32,
            ),
        },
        dispatch: terminal_psi::TerminalStoredDynamicDispatch {
            owner: id(1, MachineId::new),
            operation: id(1, OperationId::new),
            descriptor_ordinal: 4,
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec!["named(integer-const(8))".to_owned()],
            requirement_identity: "test::Scanner::scan".to_owned(),
            realization_identity: "test::Stored::scan".to_owned(),
            realization_callable_identity: "test::Stored::scan::callable".to_owned(),
            realization: id(92, MachineId::new),
        },
    }
}

fn parameter_descriptor_dispatch(
    result: terminal_psi::ClosedConformanceCallableResult,
) -> abstract_operations::AbstractParameterDynamicDispatch {
    abstract_operations::AbstractParameterDynamicDispatch {
        parameter: terminal_psi::TerminalDynamicDescriptorParameter {
            owner: id(1, MachineId::new),
            ordinal: 0,
            source_position: 1,
            trait_identity: "test::Scanner".to_owned(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            requirements: vec![terminal_psi::TerminalDynamicRequirement {
                slot: 0,
                declaring_trait_identity: "test::Scanner".to_owned(),
                public_requirement_identity: "test::Scanner::scan()".to_owned(),
                family_tuple: vec!["named(integer-const(8))".to_owned()],
                result,
            }],
        },
        dispatch: terminal_psi::TerminalParameterDynamicDispatch {
            owner: id(1, MachineId::new),
            operation: id(1, OperationId::new),
            parameter_ordinal: 0,
            requirement_slot: 0,
        },
    }
}

/// Pins the complete unit identity of one fixture per descriptor-call shape.
/// Each fixture carries a nonempty requirement and crash roster so the pinned
/// digest binds the legacy tag, result position, dispatch payload, and tail
/// order of its call row.
#[test]
fn descriptor_call_identity_bytes_are_stable() {
    let baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let result = abstract_operations::AbstractResult {
        value: id(4, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    };
    let obligations = || vec![id(8, ObligationId::new)];
    let crashes = || {
        vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    };
    let scalar = terminal_psi::ClosedConformanceCallableResult::I32;
    let unit = terminal_psi::ClosedConformanceCallableResult::Unit;
    let call = |dispatch, result| AbstractOperation::CallDynamic {
        psi_operation: id(1, OperationId::new),
        dispatch,
        result,
        requirement_obligations: obligations(),
        crash_continuations: crashes(),
    };
    let scalar_result = AbstractDynamicCallResult::Scalar(result);
    let unit_result = AbstractDynamicCallResult::Unit;
    let fixtures = [
        (
            "rebound scalar (tag 52)",
            call(
                AbstractDynamicDispatch::Rebound(rebound_descriptor_dispatch(scalar)),
                scalar_result,
            ),
            [
                184, 219, 54, 109, 2, 221, 111, 225, 213, 38, 66, 116, 146, 18, 63, 228, 113, 49,
                183, 84, 22, 134, 178, 73, 27, 188, 252, 162, 125, 163, 109, 190,
            ],
        ),
        (
            "parameter scalar (tag 53)",
            call(
                AbstractDynamicDispatch::Parameter(parameter_descriptor_dispatch(scalar)),
                scalar_result,
            ),
            [
                199, 197, 135, 63, 172, 224, 163, 153, 33, 226, 162, 35, 34, 99, 183, 102, 232, 77,
                51, 57, 6, 167, 56, 234, 81, 153, 22, 71, 201, 94, 33, 254,
            ],
        ),
        (
            "rebound unit (tag 57)",
            call(
                AbstractDynamicDispatch::Rebound(rebound_descriptor_dispatch(unit)),
                unit_result,
            ),
            [
                44, 148, 185, 6, 235, 95, 238, 70, 237, 111, 192, 206, 40, 72, 167, 118, 218, 37,
                153, 25, 248, 133, 174, 49, 136, 182, 151, 16, 255, 214, 114, 54,
            ],
        ),
        (
            "parameter unit (tag 58)",
            call(
                AbstractDynamicDispatch::Parameter(parameter_descriptor_dispatch(unit)),
                unit_result,
            ),
            [
                173, 26, 116, 233, 239, 174, 23, 102, 71, 15, 61, 234, 34, 107, 149, 206, 255, 120,
                1, 193, 40, 149, 73, 8, 219, 152, 228, 239, 251, 171, 195, 192,
            ],
        ),
        (
            "stored scalar (tag 61)",
            call(
                AbstractDynamicDispatch::Stored(stored_descriptor_dispatch()),
                scalar_result,
            ),
            [
                192, 79, 123, 57, 48, 40, 17, 248, 36, 175, 230, 232, 69, 254, 157, 22, 61, 240,
                88, 18, 3, 211, 111, 167, 218, 225, 43, 84, 125, 8, 17, 71,
            ],
        ),
        // Stage 00 never produces stored custody with a Unit result; the
        // encoder still gives it a distinct tag instead of panicking.
        (
            "stored unit (tag 93)",
            call(
                AbstractDynamicDispatch::Stored(stored_descriptor_dispatch()),
                unit_result,
            ),
            [
                151, 226, 228, 33, 19, 233, 119, 219, 10, 155, 62, 234, 178, 48, 1, 19, 214, 195,
                197, 192, 13, 65, 33, 230, 205, 83, 238, 32, 189, 6, 35, 67,
            ],
        ),
    ];
    for (shape, operation, expected) in fixtures {
        let mut changed = baseline.clone();
        changed.functions[0].blocks[0].nodes[0].operation = operation;
        let identity = recompute_psi_optimization_unit_identity(&changed);
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&changed);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            identity,
            "{shape}"
        );
        assert_eq!(identity.bytes(), expected, "{shape}");
    }
}

/// The callee-side interface a forwarded descriptor argument binds.
fn descriptor_argument_target(ordinal: u32) -> terminal_psi::TerminalDynamicDescriptorParameter {
    terminal_psi::TerminalDynamicDescriptorParameter {
        owner: id(2, MachineId::new),
        ordinal,
        source_position: ordinal + 1,
        trait_identity: "test::Scanner".to_owned(),
        access: terminal_psi::StructuralAccess::SharedBorrow,
        requirements: vec![terminal_psi::TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "test::Scanner".to_owned(),
            public_requirement_identity: "test::Scanner::scan()".to_owned(),
            family_tuple: vec!["named(integer-const(8))".to_owned()],
            result: terminal_psi::ClosedConformanceCallableResult::I32,
        }],
    }
}

/// One forwarded descriptor per source arm: a caller selection, a rebound
/// descriptor whose initializer and latest applications differ, and a
/// caller descriptor parameter.
fn forwarded_descriptor_arguments() -> Vec<abstract_operations::AbstractDynamicDescriptorArgument> {
    let scalar = terminal_psi::ClosedConformanceCallableResult::I32;
    let argument = |parameter_ordinal, source| terminal_psi::TerminalDynamicDescriptorArgument {
        owner: id(1, MachineId::new),
        operation: id(1, OperationId::new),
        parameter_ordinal,
        source,
    };
    let rebound = rebound_descriptor_dispatch(scalar);
    let caller_parameter = parameter_descriptor_dispatch(scalar).parameter;
    vec![
        abstract_operations::AbstractDynamicDescriptorArgument {
            argument: argument(
                0,
                terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            ),
            target: descriptor_argument_target(0),
            source: abstract_operations::AbstractDynamicDescriptorSource::Selection {
                selection: descriptor_selection(0),
                application: descriptor_application("Selected", scalar),
            },
        },
        abstract_operations::AbstractDynamicDescriptorArgument {
            argument: argument(
                1,
                terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 3 },
            ),
            target: descriptor_argument_target(1),
            source: abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                initial: rebound.initial,
                rebound: rebound.rebound,
                descriptor: rebound.descriptor,
                initial_application: rebound.initial_application,
                application: rebound.application,
            },
        },
        abstract_operations::AbstractDynamicDescriptorArgument {
            argument: argument(
                2,
                terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
            ),
            target: descriptor_argument_target(2),
            source: abstract_operations::AbstractDynamicDescriptorSource::Parameter(
                caller_parameter,
            ),
        },
    ]
}

/// Pins the complete unit identity of the ordinary Unit and scalar-result
/// calls, with and without forwarded descriptor arguments. Every roster the
/// shape encodes is nonempty so the digest binds the tag, the result position,
/// the argument rows and the tail order of each call row.
#[test]
fn descriptor_argument_call_identity_bytes_are_stable() {
    let baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let result = abstract_operations::AbstractResult {
        value: id(4, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    };
    let structural_arguments = || {
        vec![terminal_psi::StructuralArgument {
            place: id(11, PlaceId::new),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: Vec::new(),
        }]
    };
    let claim_transfers = || {
        vec![terminal_psi::ClaimTransfer {
            claim: id(5, ClaimId::new),
            argument_index: 0,
        }]
    };
    let obligations = || vec![id(8, ObligationId::new)];
    let crashes = || {
        vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    };
    let fixtures = [
        (
            "unit (tag 3)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                callee: id(2, MachineId::new),
                arguments: vec![id(3, ValueId::new)],
                structural_arguments: structural_arguments(),
                dynamic_arguments: Vec::new(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
                result: AbstractStructuralCallResult::Unit,
            },
            [
                99, 179, 223, 48, 230, 158, 20, 62, 68, 13, 60, 90, 168, 142, 239, 196, 122, 64,
                239, 16, 20, 52, 55, 171, 124, 46, 236, 208, 244, 0, 141, 190,
            ],
        ),
        (
            "unit with descriptor arguments (tag 59)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                callee: id(2, MachineId::new),
                arguments: Vec::new(),
                structural_arguments: structural_arguments(),
                dynamic_arguments: forwarded_descriptor_arguments(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
                result: AbstractStructuralCallResult::Unit,
            },
            [
                0, 100, 37, 181, 243, 194, 185, 204, 175, 152, 158, 205, 90, 128, 88, 181, 34, 89,
                95, 140, 9, 217, 158, 76, 49, 14, 100, 130, 136, 111, 86, 204,
            ],
        ),
        (
            "scalar (tag 4)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                result: AbstractStructuralCallResult::Scalar(result),
                callee: id(2, MachineId::new),
                arguments: vec![id(3, ValueId::new)],
                structural_arguments: structural_arguments(),
                dynamic_arguments: Vec::new(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
            },
            [
                86, 54, 247, 103, 250, 78, 130, 114, 82, 112, 121, 12, 122, 125, 171, 211, 82, 234,
                104, 223, 97, 129, 214, 191, 127, 188, 29, 142, 196, 205, 174, 24,
            ],
        ),
        (
            "scalar with descriptor arguments (tag 54)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                result: AbstractStructuralCallResult::Scalar(result),
                callee: id(2, MachineId::new),
                arguments: Vec::new(),
                structural_arguments: structural_arguments(),
                dynamic_arguments: forwarded_descriptor_arguments(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
            },
            [
                100, 6, 243, 222, 158, 151, 246, 39, 188, 71, 5, 214, 105, 234, 44, 59, 121, 133,
                172, 20, 153, 234, 29, 1, 18, 207, 62, 122, 148, 68, 33, 115,
            ],
        ),
        // Stage 00 never produces scalar arguments beside descriptor
        // arguments; the encoder still gives that shape distinct tags.
        (
            "unit with scalar and descriptor arguments (tag 94)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                callee: id(2, MachineId::new),
                arguments: vec![id(3, ValueId::new)],
                structural_arguments: structural_arguments(),
                dynamic_arguments: forwarded_descriptor_arguments(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
                result: AbstractStructuralCallResult::Unit,
            },
            [
                207, 254, 147, 160, 13, 35, 226, 159, 41, 75, 196, 190, 227, 19, 138, 195, 123, 1,
                188, 184, 135, 87, 12, 61, 213, 242, 32, 152, 201, 65, 252, 117,
            ],
        ),
        (
            "scalar with scalar and descriptor arguments (tag 95)",
            AbstractOperation::CallStructural {
                psi_operation: id(1, OperationId::new),
                result: AbstractStructuralCallResult::Scalar(result),
                callee: id(2, MachineId::new),
                arguments: vec![id(3, ValueId::new)],
                structural_arguments: structural_arguments(),
                dynamic_arguments: forwarded_descriptor_arguments(),
                claim_transfers: claim_transfers(),
                requirement_obligations: obligations(),
                crash_continuations: crashes(),
            },
            [
                185, 160, 141, 117, 231, 5, 6, 65, 23, 246, 187, 94, 137, 224, 44, 194, 194, 94,
                116, 131, 8, 11, 167, 90, 10, 28, 27, 39, 105, 56, 237, 201,
            ],
        ),
    ];
    for (shape, operation, expected) in fixtures {
        let mut changed = baseline.clone();
        changed.functions[0].blocks[0].nodes[0].operation = operation;
        let identity = recompute_psi_optimization_unit_identity(&changed);
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&changed);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            identity,
            "{shape}"
        );
        assert_eq!(identity.bytes(), expected, "{shape}");
    }
}

/// Pins the complete unit identity of one structural-result call, with and
/// without forwarded descriptor arguments. Every roster the row encodes is
/// nonempty, and the result carries qualifications, a projected qualification
/// and a claim, so the digest binds the tag, the result position, the argument
/// rows, both claim rosters and the tail order.
#[test]
fn structural_result_call_identity_bytes_are_stable() {
    let baseline =
        reconstruct_psi_optimization_unit_seed(&plan(), FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    let result_place = id(12, PlaceId::new);
    let result_type = id(13, StructuralTypeId::new);
    let call = |dynamic_arguments| AbstractOperation::CallStructural {
        psi_operation: id(1, OperationId::new),
        result: AbstractStructuralCallResult::Structural {
            result: terminal_psi::StructuralOperationResult {
                place: result_place,
                structural_type: result_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: vec![id(14, StructuralDomainId::new)],
                projected_qualifications: vec![terminal_psi::StructuralPathQualification {
                    path: vec![terminal_psi::StructuralPathSegment::Field(
                        "left".to_owned(),
                    )],
                    domain: id(15, StructuralDomainId::new),
                }],
                qualification_establishments: Vec::new(),
                claims: vec![terminal_psi::StructuralResultClaimBinding {
                    claim: id(6, ClaimId::new),
                    path: Vec::new(),
                }],
            },
            returned_claim_transfers: vec![terminal_psi::StructuralResultClaimTransfer {
                callee_claim: id(7, ClaimId::new),
                caller_claim: id(6, ClaimId::new),
            }],
            selected_evidence: vec![terminal_psi::OutcomeSpecificCallEvidence {
                guard: terminal_psi::OutcomeSpecificGuard {
                    result_type,
                    result_case: id(1, semantic_vocabulary::StructuralCaseId::new),
                },
                position: 0,
                callee_obligation: id(9, ObligationId::new),
                callee_term: id(1, semantic_vocabulary::EvidenceTermId::new),
                output_field: "ready".to_owned(),
                callee_proposition: id(1, semantic_vocabulary::PropositionId::new),
                instantiated_proposition: id(2, semantic_vocabulary::PropositionId::new),
                output: id(2, semantic_vocabulary::EvidenceTermId::new),
                result_substitution: Some(terminal_psi::OutcomeSpecificCallResultSubstitution {
                    argument_position: 0,
                    callee_result: id(16, PlaceId::new),
                    caller_result: result_place,
                }),
                validity: terminal_psi::OutcomeSpecificCallEvidenceValidity {
                    result: result_place,
                    proposition_dependencies: vec![result_place],
                    evidence_interface: terminal_psi::EvidenceInterfaceIdentity {
                        trait_identity: "test::Ready".to_owned(),
                        arguments: Vec::new(),
                        requirements: Vec::new(),
                    },
                    interface_dependencies: vec![result_place],
                },
                expected_use_count: 0,
                uses: Vec::new(),
            }],
        },
        callee: id(2, MachineId::new),
        arguments: vec![id(3, ValueId::new)],
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: id(11, PlaceId::new),
            access: terminal_psi::StructuralAccess::SharedBorrow,
            path: Vec::new(),
        }],
        dynamic_arguments,
        claim_transfers: vec![terminal_psi::ClaimTransfer {
            claim: id(5, ClaimId::new),
            argument_index: 0,
        }],
        requirement_obligations: vec![id(8, ObligationId::new)],
        crash_continuations: vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }],
    };
    let fixtures = [
        (
            "structural (tag 5)",
            call(Vec::new()),
            [
                207, 131, 239, 19, 235, 135, 26, 15, 132, 246, 254, 80, 246, 161, 145, 166, 75, 93,
                235, 104, 58, 142, 58, 176, 125, 147, 197, 105, 213, 68, 114, 117,
            ],
        ),
        // Stage 00 never produces a structural result beside descriptor
        // arguments; the encoder still gives that shape a distinct tag.
        (
            "structural with descriptor arguments (tag 96)",
            call(forwarded_descriptor_arguments()),
            [
                204, 104, 226, 110, 189, 49, 191, 174, 212, 16, 107, 243, 211, 252, 63, 176, 162,
                97, 54, 127, 255, 91, 114, 183, 36, 171, 126, 195, 107, 101, 93, 58,
            ],
        ),
    ];
    for (shape, operation, expected) in fixtures {
        let mut changed = baseline.clone();
        changed.functions[0].blocks[0].nodes[0].operation = operation;
        let identity = recompute_psi_optimization_unit_identity(&changed);
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&changed);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            identity,
            "{shape}"
        );
        assert_eq!(identity.bytes(), expected, "{shape}");
    }
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
    let edge = id(203, EdgeId::new);
    unit.proof_questions.push(ProofQuestion::new(
        unit.psi,
        [5; 32],
        ProofQuestionOwner::ScalarBlockInvariant {
            machine,
            header,
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
        ProofQuestionOwner::ScalarBlockInvariant {
            machine: id(211, MachineId::new),
            header,
            edge,
        },
        ProofQuestionOwner::ScalarBlockInvariant {
            machine,
            header: id(212, BlockId::new),
            edge,
        },
        ProofQuestionOwner::ScalarBlockInvariant {
            machine,
            header,
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
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        });
    mutations.push(("module structural type", unit));
    let mut unit = baseline.clone();
    unit.structural_domains = Arc::from(vec![terminal_psi::StructuralDomainDeclaration {
        byte_predicate: None,
        establishment_routes: Vec::new(),
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
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Machine,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        id: boundary,
        identity: "identity-test-boundary".into(),
        attachment: Some(structural_type),
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
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

#[test]
fn streamed_identity_matches_materialized_canonical_bytes() {
    for source in [plan(), write_only_store_plan()] {
        let unit =
            reconstruct_psi_optimization_unit_seed(&source, FuelScheduleIdentity::new(1).unwrap())
                .unwrap();
        let canonical = crate::optimization_unit::identity::collect_unit_canonical_bytes(&unit);
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&canonical),
            recompute_psi_optimization_unit_identity(&unit),
        );
        let catalog = crate::optimization_unit::identity::collect_structural_domain_catalog_bytes(
            unit.structural_domains.as_ref(),
        );
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_bytes(&catalog),
            structural_domain_catalog_identity(unit.structural_domains.as_ref()),
        );
    }
}

#[test]
fn value_definition_site_identity_bytes_are_pinned_for_every_variant() {
    // Expected bytes ported from the seven copies of this encoder that
    // selected-instructions, register-homes and legalized-operations carried
    // before de-duplication. `BlockId` writes its nonzero `u64`; positions and
    // node indices write a `u32`; both little-endian.
    let block = BlockId::new(0x0807_0605_0403_0201).expect("block identity");
    let cases: [(ValueDefinitionSite, &[u8]); 3] = [
        (
            ValueDefinitionSite::FunctionParameter(0x0403_0201),
            &[0, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            ValueDefinitionSite::BlockParameter {
                block,
                position: 0x0c0b_0a09,
            },
            &[
                1, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
            ],
        ),
        (
            ValueDefinitionSite::Node {
                block,
                node: 0x0c0b_0a09,
            },
            &[
                2, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
            ],
        ),
    ];
    for (site, expected) in cases {
        let mut bytes = Vec::new();
        encode_value_definition_site_identity(&mut bytes, site);
        assert_eq!(bytes, expected, "{site:?}");
    }
}
