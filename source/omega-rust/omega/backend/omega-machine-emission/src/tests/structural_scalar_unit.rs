use super::{EmissionError, emit_machine_code};
use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedCallDestination, AssignedFunction, AssignedIntegerExpression,
    AssignedOperation, AssignedOperationPlan, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument, ExpressionFrame,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_target_operations::{
    AbstractResult, FixedIntegerScalarAbiValue, TargetStructuralParameter, TerminalPsiProvenance,
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
                mixed_structural_scalar_abi: None,
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
                            scalar_arguments: Vec::new(),
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
                mixed_structural_scalar_abi: None,
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

fn assigned_structural_scalar_return_plan(target: NativeTarget) -> AssignedOperationPlan {
    let mut plan = assigned_direct_plan(target);
    let caller = &mut plan.functions[0];
    let AssignedOperation::UnitBody(body) = &caller.operation else {
        unreachable!()
    };
    let AssignedUnitOperation::StructuralScalarCall {
        psi_operation,
        result,
        callee,
        call_plan: _,
        scalar_arguments: _,
        copies,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &body.operations[2]
    else {
        unreachable!()
    };
    let psi_operation = *psi_operation;
    let result = result.clone();
    let callee = *callee;
    let mut copies = copies.clone();
    let structural_types = body.structural_types.clone();
    let mut structural_parameters = body.parameters.clone();
    let claim_transfers = claim_transfers.clone();
    let requirement_obligations = requirement_obligations.clone();
    let crash_continuations = crash_continuations.clone();
    let result_shape = ValueShape::integer(4, 4);
    let return_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: structural_parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: Some(result_shape),
        },
    )
    .expect("structural-scalar return ABI");
    structural_parameters[0].placement = return_call_plan.parameters[0].clone();
    copies[0].source = return_call_plan.parameters[0].clone();
    let psi_edge = EdgeId::new(962).unwrap();
    caller.provenance = TerminalPsiProvenance {
        operations: vec![psi_operation],
        edges: vec![psi_edge],
    };
    caller.operation = AssignedOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        callee,
        structural_types,
        call_plan: return_call_plan,
        structural_parameters,
        copies,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    };
    plan
}

fn scalar_destination(placement: &ValuePlacement) -> AssignedCallDestination {
    match placement.locations.as_slice() {
        [ValueLocation::Register { register, .. }] => AssignedCallDestination::Register(*register),
        [
            ValueLocation::Stack {
                stack_byte_offset, ..
            },
        ] => AssignedCallDestination::OutgoingStack {
            byte_offset: *stack_byte_offset,
        },
        _ => panic!("fixed integer argument has one direct destination"),
    }
}

fn assigned_mixed_call_plan(target: NativeTarget) -> AssignedOperationPlan {
    let mut plan = assigned_direct_plan(target);
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_shape = ValueShape::integer(4, 4);
    let caller = &mut plan.functions[0];
    let AssignedOperation::UnitBody(body) = &mut caller.operation else {
        unreachable!()
    };
    let root_placement = body.parameters[0].placement.clone();
    let AssignedUnitOperation::StructuralScalarCall {
        copies,
        result,
        callee,
        ..
    } = &body.operations[2]
    else {
        unreachable!()
    };
    let mut copy = copies[0].clone();
    let result = result.clone();
    let callee = *callee;
    let mixed_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: std::iter::repeat_n(scalar_shape, 8)
                .chain(std::iter::once(copy.shape))
                .collect(),
            result: Some(scalar_shape),
        },
    )
    .expect("eight scalar arguments and one aggregate have one native ABI");
    copy.destination = mixed_call_plan.parameters[8].clone();
    let constants = (0_u64..8)
        .map(|ordinal| {
            (
                OperationId::new(970 + ordinal).unwrap(),
                ValueId::new(970 + ordinal).unwrap(),
                IntegerValue::Signed(i128::from(ordinal + 1)),
            )
        })
        .collect::<Vec<_>>();
    let scalar_arguments = constants
        .iter()
        .enumerate()
        .map(
            |(parameter_index, (defining_operation, source_value, value))| {
                AssignedUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index).unwrap(),
                    source: AssignedUnitScalarArgumentSource::IntegerImmediate {
                        defining_operation: *defining_operation,
                        source_value: *source_value,
                        scalar_type: integer_type,
                        value: *value,
                    },
                    destination: scalar_destination(&mixed_call_plan.parameters[parameter_index]),
                }
            },
        )
        .collect::<Vec<_>>();
    let mut operations = constants
        .iter()
        .map(
            |(psi_operation, result, value)| AssignedUnitOperation::IntegerConstant {
                psi_operation: *psi_operation,
                result: *result,
                scalar_type: integer_type,
                value: *value,
            },
        )
        .collect::<Vec<_>>();
    operations.push(AssignedUnitOperation::StructuralScalarCall {
        psi_operation: OperationId::new(980).unwrap(),
        result,
        callee,
        call_plan: mixed_call_plan,
        scalar_arguments,
        copies: vec![copy],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    });
    operations.push(AssignedUnitOperation::Return {
        psi_edge: EdgeId::new(980).unwrap(),
        cleanup_actions: Vec::new(),
    });
    body.operations = operations;
    body.parameters[0].placement = root_placement;
    plan.functions[1].operation = AssignedOperation::ReturnIntegerImmediate {
        psi_edge: EdgeId::new(981).unwrap(),
        source_value: ValueId::new(981).unwrap(),
        scalar_type: integer_type,
        value: IntegerValue::Signed(7),
    };
    plan.functions[1].mixed_structural_scalar_abi = Some(mixed_callee_abi(&plan));
    plan
}

fn mixed_call_mut(plan: &mut AssignedOperationPlan) -> &mut AssignedUnitOperation {
    let AssignedOperation::UnitBody(body) = &mut plan.functions[0].operation else {
        unreachable!()
    };
    body.operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                AssignedUnitOperation::StructuralScalarCall { .. }
            )
        })
        .expect("mixed call")
}

fn mixed_callee_abi(
    plan: &AssignedOperationPlan,
) -> omega_target_operations::MixedStructuralScalarFunctionAbi {
    let AssignedOperation::UnitBody(body) = &plan.functions[0].operation else {
        unreachable!()
    };
    let AssignedUnitOperation::StructuralScalarCall {
        call_plan,
        scalar_arguments,
        copies,
        ..
    } = &body.operations[8]
    else {
        unreachable!()
    };
    let [copy] = copies.as_slice() else {
        unreachable!()
    };
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    omega_target_operations::MixedStructuralScalarFunctionAbi {
        call_plan: call_plan.clone(),
        scalar_parameters: scalar_arguments
            .iter()
            .enumerate()
            .map(|(index, _)| FixedIntegerScalarAbiValue {
                value: ValueId::new(990 + u64::try_from(index).unwrap()).unwrap(),
                scalar_type: integer_type,
                placement: call_plan.parameters[index].clone(),
            })
            .collect(),
        structural_parameters: vec![TargetStructuralParameter {
            place: PlaceId::new(961).unwrap(),
            structural_type: copy.structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: copy.access,
            projected_qualifications: Vec::new(),
            shape: copy.shape,
            placement: copy.destination.clone(),
        }],
        result: omega_target_operations::MixedStructuralScalarAbiResult {
            value: ValueId::new(981).unwrap(),
            scalar_type: ScalarType::Integer(integer_type),
            placement: call_plan.result.clone().expect("scalar result placement"),
        },
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
        assert_eq!(
            call.semantic_result,
            Some(AbstractResult {
                value: ValueId::new(961).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap()
                ),
            })
        );
        assert_eq!(call.arguments.len(), 1);
        assert!(call.scalar_arguments.is_empty());
        assert!(caller.unit_scalar_homes.is_empty());
    }
}

#[test]
fn mixed_fixed_integer_and_aggregate_call_emits_one_exact_outbound_area() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = assigned_mixed_call_plan(target);
        let callee_abi = source.functions[1]
            .mixed_structural_scalar_abi
            .clone()
            .expect("callee owns mixed ABI");
        let AssignedOperation::UnitBody(body) = &source.functions[0].operation else {
            unreachable!()
        };
        let AssignedUnitOperation::StructuralScalarCall { call_plan, .. } = &body.operations[8]
        else {
            unreachable!()
        };
        let emitted = emit_machine_code(&source).expect("mixed structural-scalar call emits");
        let caller = &emitted.functions[0];
        assert_eq!(
            emitted.functions[1].mixed_structural_scalar_abi,
            Some(callee_abi)
        );
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one mixed internal call record")
        };
        let [relocation] = caller.internal_calls.as_slice() else {
            panic!("one mixed internal call relocation")
        };
        let outbound = relocation
            .unit_stack
            .and_then(|stack| stack.outbound)
            .expect("nine mixed arguments force one outbound area");
        assert!(outbound.byte_size >= 16);
        assert_eq!(call.scalar_arguments.len(), 8);
        assert_eq!(call.arguments.len(), 1);
        for (parameter_index, argument) in call.scalar_arguments.iter().enumerate() {
            assert_eq!(
                usize::try_from(argument.parameter_index),
                Ok(parameter_index)
            );
            assert_eq!(argument.destination, call_plan.parameters[parameter_index]);
            assert!(argument.byte_count > 0);
            assert!(argument.code_offset >= call.code_offset);
            assert!(argument.code_offset + argument.byte_count <= relocation.offset);
            assert!(matches!(
                argument.source,
                omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    source_value,
                    value: IntegerValue::Signed(value),
                    ..
                } if source_value == ValueId::new(970 + u64::try_from(parameter_index).unwrap()).unwrap()
                    && value == i128::try_from(parameter_index + 1).unwrap()
            ));
        }
        assert_eq!(call.arguments[0].destination, call_plan.parameters[8]);
        assert_eq!(call.arguments[0].call_stack_bytes, outbound.byte_size);
        assert!(call.arguments[0].code_offset >= call.scalar_arguments[7].code_offset);
        assert!(outbound.allocation_offset >= call.code_offset);
        assert!(outbound.release_offset > relocation.offset);
        assert_eq!(relocation.owner, call.owner);
        assert_eq!(relocation.target, call.target);
    }
}

#[test]
fn mixed_call_rejects_source_partition_plan_and_callee_drift() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let rejects = |plan: &AssignedOperationPlan| {
            assert!(matches!(
                emit_machine_code(plan),
                Err(EmissionError::InvalidStructuralScalarCallCustody(_))
                    | Err(EmissionError::InvalidMixedStructuralScalarFunctionAbi(_))
            ));
        };

        let mut source = assigned_mixed_call_plan(target);
        let AssignedUnitOperation::StructuralScalarCall {
            scalar_arguments, ..
        } = mixed_call_mut(&mut source)
        else {
            unreachable!()
        };
        let AssignedUnitScalarArgumentSource::IntegerImmediate { value, .. } =
            &mut scalar_arguments[0].source
        else {
            unreachable!()
        };
        *value = IntegerValue::Signed(99);
        rejects(&source);

        let mut scalar_partition = assigned_mixed_call_plan(target);
        let AssignedUnitOperation::StructuralScalarCall {
            call_plan,
            scalar_arguments,
            ..
        } = mixed_call_mut(&mut scalar_partition)
        else {
            unreachable!()
        };
        scalar_arguments[0].destination = scalar_destination(&call_plan.parameters[1]);
        rejects(&scalar_partition);

        let mut aggregate_partition = assigned_mixed_call_plan(target);
        let AssignedUnitOperation::StructuralScalarCall {
            call_plan, copies, ..
        } = mixed_call_mut(&mut aggregate_partition)
        else {
            unreachable!()
        };
        copies[0].destination = call_plan.parameters[0].clone();
        rejects(&aggregate_partition);

        let mut plan_drift = assigned_mixed_call_plan(target);
        let AssignedUnitOperation::StructuralScalarCall { call_plan, .. } =
            mixed_call_mut(&mut plan_drift)
        else {
            unreachable!()
        };
        call_plan.parameters.swap(0, 1);
        rejects(&plan_drift);

        let mut callee_result = assigned_mixed_call_plan(target);
        let AssignedOperation::ReturnIntegerImmediate { scalar_type, .. } =
            &mut callee_result.functions[1].operation
        else {
            unreachable!()
        };
        *scalar_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        rejects(&callee_result);

        let mut duplicate_callee = assigned_mixed_call_plan(target);
        duplicate_callee
            .functions
            .push(duplicate_callee.functions[1].clone());
        rejects(&duplicate_callee);

        let mut missing_abi = assigned_mixed_call_plan(target);
        missing_abi.functions[1].mixed_structural_scalar_abi = None;
        assert!(matches!(
            emit_machine_code(&missing_abi),
            Err(EmissionError::InvalidStructuralScalarCallCustody(_))
        ));

        let mut scalar_type_abi = assigned_mixed_call_plan(target);
        scalar_type_abi.functions[1]
            .mixed_structural_scalar_abi
            .as_mut()
            .unwrap()
            .scalar_parameters[0]
            .scalar_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        rejects(&scalar_type_abi);

        let mut structural_type_abi = assigned_mixed_call_plan(target);
        structural_type_abi.functions[1]
            .mixed_structural_scalar_abi
            .as_mut()
            .unwrap()
            .structural_parameters[0]
            .structural_type = StructuralTypeId::new(999).unwrap();
        rejects(&structural_type_abi);
    }
}

#[test]
fn structural_scalar_return_emits_exact_value_and_return_evidence() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emit_machine_code(&assigned_structural_scalar_return_plan(target))
            .expect("structural scalar return emits");
        let caller = &emitted.functions[0];
        let expected = omega_machine_code::StructuralCallScalarReturnEvidence {
            psi_edge: EdgeId::new(962).unwrap(),
            psi_operation: OperationId::new(962).unwrap(),
            source_value: ValueId::new(961).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            callee: MachineId::new(961).unwrap(),
        };
        assert_eq!(caller.structural_call_scalar_return, Some(expected));
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one exact structural scalar return call")
        };
        assert_eq!(
            call.semantic_result,
            Some(AbstractResult {
                value: expected.source_value,
                scalar_type: expected.scalar_type,
            })
        );
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
