use crate::assignment::shared::{
    AssignedUnitOperation, CallSignature, CallingPolicy, MachineId, TargetOperationPlan,
    TargetUnitOperation, evaluate_call_plan,
};
use crate::{AssignmentError, assign_registers};
use assigned_target_operations::AssignedOperation;
use semantic_vocabulary::{
    ObligationId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId,
};
use target::NativeTarget;
use target_operations::{
    TargetFunction, TargetOperation, TargetStructuralParameter, TerminalPsiProvenance,
};
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalPsiIdentity, VocabularyMarker,
};

#[test]
fn unit_assignment_retains_typed_structural_argument_paths() {
    let target = NativeTarget::linux_x64();
    let shape = calling_conventions::ValueShape::integer(2, 1);
    let element_shape = calling_conventions::ValueShape::integer(1, 1);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let element_type = StructuralTypeId::new(2).unwrap();
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![element_shape],
            result: None,
        },
    )
    .unwrap();
    let path = vec![StructuralPathSegment::FixedIndex(1)];
    let plan = TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::UnitBody(target_operations::TargetUnitBody {
                structural_types: vec![
                    StructuralTypeDeclaration {
                        id: structural_type,
                        identity: "[Token; 2]".into(),
                        shape: StructuralTypeShape::FixedArray {
                            element: element_type,
                            length: 2,
                        },
                    },
                    StructuralTypeDeclaration {
                        id: element_type,
                        identity: "Token".into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![terminal_psi::StructuralFieldDeclaration {
                                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                                identity: "value".into(),
                                relevance: terminal_psi::BindingRelevance::Relevant,
                                field_type: terminal_psi::StructuralFieldType::Scalar(
                                    semantic_vocabulary::ScalarType::Boolean,
                                ),
                            }],
                        },
                    },
                ],
                call_plan: call_plan.clone(),
                scalar_parameters: Vec::new(),
                parameters: vec![TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    access: terminal_psi::StructuralAccess::Owned,
                    projected_qualifications: Vec::new(),
                    shape,
                    placement: call_plan.parameters[0].clone(),
                }],
                operations: vec![TargetUnitOperation::Call {
                    psi_operation: OperationId::new(1).unwrap(),
                    callee: MachineId::new(2).unwrap(),
                    call_plan: callee_plan.clone(),
                    scalar_arguments: Vec::new(),
                    arguments: vec![target_operations::TargetStructuralArgument {
                        place,
                        access: terminal_psi::StructuralAccess::Owned,
                        path: path.clone(),
                        root_structural_type: structural_type,
                        structural_type: element_type,
                        shape: element_shape,
                        source_byte_offset: 1,
                        fixed_array_length: Some(2),
                        element_stride: Some(1),
                        source: call_plan.parameters[0].clone(),
                        destination: callee_plan.parameters[0].clone(),
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: vec![ObligationId::new(1).unwrap()],
                    crash_continuations: vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }],
                }],
            }),
        }],
    };

    let assigned = assign_registers(&plan).unwrap();
    let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
        panic!("Unit body")
    };
    let AssignedUnitOperation::Call {
        copies,
        transport,
        requirement_obligations,
        crash_continuations,
        ..
    } = &body.operations[0]
    else {
        panic!("Unit call")
    };
    assert_eq!(copies[0].path, path);
    assert!(
        transport.is_none(),
        "aggregate-only calls have no scalar transport"
    );
    assert_eq!(requirement_obligations, &[ObligationId::new(1).unwrap()]);
    assert_eq!(
        crash_continuations,
        &[CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    );

    let mut missing_types = plan.clone();
    let TargetOperation::UnitBody(body) = &mut missing_types.functions[0].operation else {
        unreachable!()
    };
    body.structural_types.clear();
    assert!(
        assign_registers(&missing_types).is_err(),
        "a path needs its actual root and element declarations"
    );

    let mut corrupted = plan;
    let TargetOperation::UnitBody(body) = &mut corrupted.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::Call { call_plan, .. } = &mut body.operations[0] else {
        unreachable!()
    };
    *call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature::default(),
    )
    .unwrap();
    assert!(matches!(
        assign_registers(&corrupted),
        Err(AssignmentError::UnitCallCustodyMismatch { .. })
    ));
}

fn trivial_affine_local_call_plan(target: NativeTarget) -> TargetOperationPlan {
    let machine = MachineId::new(11).unwrap();
    let callee = MachineId::new(12).unwrap();
    let structural_type = StructuralTypeId::new(11).unwrap();
    let place = PlaceId::new(11).unwrap();
    let shape = calling_conventions::ValueShape::integer(0, 1);
    let caller_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .unwrap();
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let declaration = StructuralTypeDeclaration {
        id: structural_type,
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x11; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            operation: TargetOperation::UnitBody(target_operations::TargetUnitBody {
                structural_types: vec![declaration.clone()],
                call_plan: caller_plan,
                scalar_parameters: Vec::new(),
                parameters: Vec::new(),
                operations: vec![
                    TargetUnitOperation::EstablishTrivialAffineLocal {
                        psi_operation: OperationId::new(11).unwrap(),
                        place: StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::TrivialAffineLocal {
                                declaration_ordinal: 0,
                                structural_type,
                                construction: None,
                            },
                        },
                        structural_type: declaration,
                    },
                    TargetUnitOperation::Call {
                        psi_operation: OperationId::new(12).unwrap(),
                        callee,
                        call_plan: callee_plan.clone(),
                        scalar_arguments: Vec::new(),
                        arguments: vec![target_operations::TargetStructuralArgument {
                            place,
                            access: terminal_psi::StructuralAccess::Owned,
                            path: Vec::new(),
                            root_structural_type: structural_type,
                            structural_type,
                            shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            source: calling_conventions::ValuePlacement {
                                shape,
                                locations: Vec::new(),
                            },
                            destination: callee_plan.parameters[0].clone(),
                        }],
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                ],
            }),
        }],
    }
}

#[test]
fn unit_assignment_replays_trivial_affine_local_call_sources() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = trivial_affine_local_call_plan(target);
        assign_registers(&plan).expect("established affine local assigns");

        let mut before_establishment = plan.clone();
        let TargetOperation::UnitBody(body) = &mut before_establishment.functions[0].operation
        else {
            unreachable!()
        };
        body.operations.remove(0);
        assert!(matches!(
            assign_registers(&before_establishment),
            Err(AssignmentError::UnitCallCustodyMismatch { .. })
        ));

        let mut repeated = plan;
        let TargetOperation::UnitBody(body) = &mut repeated.functions[0].operation else {
            unreachable!()
        };
        body.operations.push(body.operations[1].clone());
        assert!(matches!(
            assign_registers(&repeated),
            Err(AssignmentError::UnitCallCustodyMismatch { .. })
        ));
    }
}
