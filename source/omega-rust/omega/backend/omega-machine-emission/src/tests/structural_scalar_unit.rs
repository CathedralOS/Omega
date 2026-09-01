use super::{EmissionError, emit_machine_code};
use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedFunction, AssignedIntegerExpression, AssignedOperation,
    AssignedOperationPlan, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, ExpressionFrame,
};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{AbstractResult, TargetStructuralParameter, TerminalPsiProvenance};
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

fn assigned_direct_plan(target: NativeTarget) -> AssignedOperationPlan {
    let caller = MachineId::new(960).unwrap();
    let callee = MachineId::new(961).unwrap();
    let root_type = StructuralTypeId::new(960).unwrap();
    let carrier_type = StructuralTypeId::new(961).unwrap();
    let root = PlaceId::new(960).unwrap();
    let callee_root = PlaceId::new(961).unwrap();
    let field = StructuralFieldId::new(961).unwrap();
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
    let constant_operation = OperationId::new(960).unwrap();
    let store_operation = OperationId::new(961).unwrap();
    let call_operation = OperationId::new(962).unwrap();
    let constant = ValueId::new(960).unwrap();
    let result = ValueId::new(961).unwrap();
    let path = vec![StructuralPathSegment::Field("item".into())];
    let source = AssignedUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: constant_operation,
        source_value: constant,
        scalar_type: integer_type,
        value: IntegerValue::Signed(17),
    };
    AssignedOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x96; 32]),
        },
        target,
        entry: caller,
        functions: vec![
            AssignedFunction {
                machine: caller,
                attachment: Some(root_type),
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: AssignedOperation::UnitBody(AssignedUnitBody {
                    structural_types: vec![
                        StructuralTypeDeclaration {
                            id: root_type,
                            identity: "DriverState".into(),
                            shape: StructuralTypeShape::Record {
                                fields: vec![StructuralFieldDeclaration {
                                    id: StructuralFieldId::new(960).unwrap(),
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
                                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                        integer_type,
                                    )),
                                }],
                            },
                        },
                    ],
                    call_plan: caller_plan.clone(),
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
                        AssignedUnitOperation::IntegerConstant {
                            psi_operation: constant_operation,
                            result: constant,
                            scalar_type: integer_type,
                            value: IntegerValue::Signed(17),
                        },
                        AssignedUnitOperation::StructuralScalarFieldStore {
                            psi_operation: store_operation,
                            destination: StructuralParameterDeclaration {
                                place: root,
                                position: 0,
                                is_self: true,
                                structural_type: root_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                access: StructuralAccess::MutableBorrow,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                            },
                            path: path.clone(),
                            field,
                            destination_placement: caller_plan.parameters[0].clone(),
                            field_byte_offset: 0,
                            source,
                        },
                        AssignedUnitOperation::StructuralScalarCall {
                            psi_operation: call_operation,
                            result: AbstractResult {
                                value: result,
                                scalar_type: ScalarType::Integer(integer_type),
                            },
                            callee,
                            call_plan: callee_plan.clone(),
                            copies: vec![AssignedAggregateCopy {
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
                        AssignedUnitOperation::Return {
                            psi_edge: EdgeId::new(960).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            AssignedFunction {
                machine: callee,
                attachment: Some(carrier_type),
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: AssignedOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(961).unwrap(),
                    source_value: ValueId::new(962).unwrap(),
                    scalar_type: integer_type,
                    frame: ExpressionFrame {
                        byte_size: 0,
                        register_spills: Vec::new(),
                    },
                    expression: AssignedIntegerExpression::StructuralField {
                        psi_operation: OperationId::new(963).unwrap(),
                        source_value: ValueId::new(962).unwrap(),
                        source: callee_root,
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
fn projected_store_and_discarded_scalar_call_emit_exact_cross_architecture_custody() {
    for (target, expected_store) in [
        (
            NativeTarget::linux_x64(),
            vec![
                0x49, 0xbb, 17, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x89, 0x5c, 0x24, 0,
            ],
        ),
        (
            NativeTarget::linux_arm64(),
            [0xd280_0230_u32.to_le_bytes(), 0xb900_03f0_u32.to_le_bytes()].concat(),
        ),
    ] {
        let emitted = emit_machine_code(&assigned_direct_plan(target)).expect("direct lane emits");
        let caller = &emitted.functions[0];
        let [store] = caller.unit_structural_scalar_field_stores.as_slice() else {
            panic!("one field-store custody row")
        };
        assert_eq!(store.bytes, expected_store);
        assert_eq!(
            &caller.bytes[store.code_offset..store.code_offset + store.byte_count],
            store.bytes
        );
        assert_eq!(store.field_byte_offset, 0);
        assert_eq!(store.parameter_home_byte_offset, 0);
        assert!(!store.parameter_home_indirect);
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one projected structural call")
        };
        assert_eq!(
            call.result,
            Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap()
            ))
        );
        assert_eq!(call.arguments.len(), 1);
        assert!(caller.unit_scalar_homes.is_empty());
    }
}

#[test]
fn projected_store_and_discarded_scalar_call_reject_machine_custody_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut source_plan = assigned_direct_plan(target);
        let AssignedOperation::UnitBody(body) = &mut source_plan.functions[0].operation else {
            unreachable!()
        };
        let AssignedUnitOperation::StructuralScalarFieldStore { source, .. } =
            &mut body.operations[1]
        else {
            unreachable!()
        };
        let AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. } = source else {
            unreachable!()
        };
        *value = IntegerValue::Signed(18);
        assert!(matches!(
            emit_machine_code(&source_plan),
            Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(_))
        ));

        let mut call = assigned_direct_plan(target);
        let AssignedOperation::UnitBody(body) = &mut call.functions[0].operation else {
            unreachable!()
        };
        let AssignedUnitOperation::StructuralScalarCall { result, .. } = &mut body.operations[2]
        else {
            unreachable!()
        };
        result.scalar_type = ScalarType::Boolean;
        assert!(matches!(
            emit_machine_code(&call),
            Err(EmissionError::InvalidStructuralScalarCallCustody(_))
        ));
    }
}
