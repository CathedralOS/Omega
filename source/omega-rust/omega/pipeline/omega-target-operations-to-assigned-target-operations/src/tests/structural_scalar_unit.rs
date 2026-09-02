use crate::{AssignmentError, assign_registers};
use omega_assigned_target_operations::{AssignedOperation, AssignedUnitOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{
    AbstractResult, TargetFunction, TargetIntegerExpression, TargetOperation, TargetOperationPlan,
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitBody, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TerminalPsiProvenance,
};
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity,
    VocabularyMarker,
};

fn direct_plan(target: NativeTarget) -> TargetOperationPlan {
    let caller = MachineId::new(950).unwrap();
    let callee = MachineId::new(951).unwrap();
    let root_type = StructuralTypeId::new(950).unwrap();
    let carrier_type = StructuralTypeId::new(951).unwrap();
    let root = PlaceId::new(950).unwrap();
    let field = StructuralFieldId::new(951).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let root_shape = ValueShape::integer(4, 4);
    let scalar_shape = ValueShape::integer(4, 4);
    let caller_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape],
            result: Some(scalar_shape),
        },
    )
    .unwrap();
    let destination = StructuralParameterDeclaration {
        place: root,
        position: 0,
        is_self: true,
        structural_type: root_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let path = vec![StructuralPathSegment::Field("item".into())];
    let constant_operation = OperationId::new(950).unwrap();
    let store_operation = OperationId::new(951).unwrap();
    let call_operation = OperationId::new(952).unwrap();
    let constant = ValueId::new(950).unwrap();
    let discarded_result = ValueId::new(951).unwrap();
    let declarations = vec![
        StructuralTypeDeclaration {
            id: root_type,
            identity: "DriverState".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(950).unwrap(),
                    identity: "item".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(carrier_type),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: carrier_type,
            identity: "Register".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                }],
            },
        },
    ];
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x95; 32]),
        },
        target,
        entry: caller,
        functions: vec![
            TargetFunction {
                machine: caller,
                attachment: Some(root_type),
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: declarations,
                    call_plan: caller_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![TargetStructuralParameter {
                        place: root,
                        structural_type: root_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::MutableBorrow,
                        projected_qualifications: Vec::new(),
                        shape: root_shape,
                        placement: caller_plan.parameters[0].clone(),
                    }],
                    operations: vec![
                        TargetUnitOperation::IntegerConstant {
                            psi_operation: constant_operation,
                            result: constant,
                            scalar_type: integer_type,
                            value: IntegerValue::Signed(17),
                        },
                        TargetUnitOperation::StructuralScalarFieldStore {
                            psi_operation: store_operation,
                            destination,
                            path: path.clone(),
                            field,
                            destination_placement: caller_plan.parameters[0].clone(),
                            field_byte_offset: 0,
                            source: TargetUnitScalarArgumentSource::IntegerImmediate {
                                defining_operation: constant_operation,
                                source_value: constant,
                                scalar_type: integer_type,
                                value: IntegerValue::Signed(17),
                            },
                        },
                        TargetUnitOperation::StructuralScalarCall {
                            psi_operation: call_operation,
                            result: AbstractResult {
                                value: discarded_result,
                                scalar_type: ScalarType::Integer(integer_type),
                            },
                            callee,
                            call_plan: callee_plan.clone(),
                            arguments: vec![TargetStructuralArgument {
                                place: root,
                                access: StructuralAccess::SharedBorrow,
                                path,
                                root_structural_type: root_type,
                                structural_type: carrier_type,
                                shape: scalar_shape,
                                source_byte_offset: 0,
                                fixed_array_length: None,
                                element_stride: None,
                                source: caller_plan.parameters[0].clone(),
                                destination: callee_plan.parameters[0].clone(),
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(950).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            TargetFunction {
                machine: callee,
                attachment: Some(carrier_type),
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: TargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(951).unwrap(),
                    source_value: ValueId::new(952).unwrap(),
                    scalar_type: integer_type,
                    expression: TargetIntegerExpression::StructuralField {
                        psi_operation: OperationId::new(953).unwrap(),
                        source_value: ValueId::new(952).unwrap(),
                        source: PlaceId::new(951).unwrap(),
                        field,
                        source_placement: callee_plan.parameters[0].clone(),
                        field_byte_offset: 0,
                        integer_type,
                    },
                },
            },
        ],
    }
}

#[test]
fn attached_unit_structural_scalar_lane_replays_exact_custody_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&direct_plan(target)).expect("direct lane assigns");
        let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
            panic!("caller remains a Unit body")
        };
        assert!(matches!(
            &body.operations[1],
            AssignedUnitOperation::StructuralScalarFieldStore {
                field_byte_offset: 0,
                source: omega_assigned_target_operations::AssignedUnitScalarArgumentSource::IntegerImmediate {
                    value: IntegerValue::Signed(17),
                    ..
                },
                ..
            }
        ));
        let AssignedUnitOperation::StructuralScalarCall { result, copies, .. } =
            &body.operations[2]
        else {
            panic!("projected scalar call remains explicit")
        };
        assert_eq!(
            result.scalar_type,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
        );
        assert_eq!(copies.len(), 1);
        assert_eq!(
            copies[0].path,
            [StructuralPathSegment::Field("item".into())]
        );
    }
}

#[test]
fn attached_unit_structural_scalar_lane_rejects_offset_source_and_abi_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut offset = direct_plan(target);
        let TargetOperation::UnitBody(body) = &mut offset.functions[0].operation else {
            unreachable!()
        };
        let TargetUnitOperation::StructuralScalarFieldStore {
            field_byte_offset, ..
        } = &mut body.operations[1]
        else {
            unreachable!()
        };
        *field_byte_offset = 4;
        assert!(matches!(
            assign_registers(&offset),
            Err(AssignmentError::StructuralScalarFieldStoreCustodyMismatch { .. })
        ));

        let mut source_plan = direct_plan(target);
        let TargetOperation::UnitBody(body) = &mut source_plan.functions[0].operation else {
            unreachable!()
        };
        let TargetUnitOperation::StructuralScalarFieldStore { source, .. } =
            &mut body.operations[1]
        else {
            unreachable!()
        };
        let TargetUnitScalarArgumentSource::IntegerImmediate { source_value, .. } = source else {
            unreachable!()
        };
        *source_value = ValueId::new(999).unwrap();
        assert!(matches!(
            assign_registers(&source_plan),
            Err(AssignmentError::StructuralScalarFieldStoreCustodyMismatch { .. })
        ));

        let mut abi = direct_plan(target);
        let TargetOperation::UnitBody(body) = &mut abi.functions[0].operation else {
            unreachable!()
        };
        let TargetUnitOperation::StructuralScalarCall { call_plan, .. } = &mut body.operations[2]
        else {
            unreachable!()
        };
        call_plan.result = None;
        assert!(matches!(
            assign_registers(&abi),
            Err(AssignmentError::StructuralScalarCallCustodyMismatch { .. })
        ));
    }
}
