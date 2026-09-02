use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedCallDestination, AssignedFunction, AssignedOperation,
    AssignedOperationPlan, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use omega_image_emission::{ObjectError, build_object_artifact};
use omega_machine_emission::emit_machine_code;
use omega_target::NativeTarget;
use omega_target_operations::{
    AbstractResult, FixedIntegerScalarAbiValue, MixedStructuralScalarFunctionAbi,
    TargetStructuralParameter, TerminalPsiProvenance,
};
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalPsiIdentity, VocabularyMarker,
};

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

fn mixed_plan(target: NativeTarget) -> AssignedOperationPlan {
    let caller = MachineId::new(9920).unwrap();
    let callee = MachineId::new(9921).unwrap();
    let structural_type = StructuralTypeId::new(9920).unwrap();
    let caller_place = PlaceId::new(9920).unwrap();
    let callee_place = PlaceId::new(9921).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let shape = ValueShape::integer(4, 4);
    let caller_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let mixed_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape, shape],
            result: Some(shape),
        },
    )
    .unwrap();
    let constant_operation = OperationId::new(9920).unwrap();
    let call_operation = OperationId::new(9921).unwrap();
    let constant = ValueId::new(9920).unwrap();
    let call_result = ValueId::new(9921).unwrap();
    let callee_scalar = ValueId::new(9922).unwrap();
    let callee_result = ValueId::new(9923).unwrap();
    let declaration = StructuralTypeDeclaration {
        id: structural_type,
        identity: "Word".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(9920).unwrap(),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
            }],
        },
    };
    let caller_parameter = TargetStructuralParameter {
        place: caller_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        projected_qualifications: Vec::new(),
        shape,
        placement: caller_plan.parameters[0].clone(),
    };
    let callee_parameter = TargetStructuralParameter {
        place: callee_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        projected_qualifications: Vec::new(),
        shape,
        placement: mixed_call_plan.parameters[1].clone(),
    };
    let mixed_abi = MixedStructuralScalarFunctionAbi {
        call_plan: mixed_call_plan.clone(),
        scalar_parameters: vec![FixedIntegerScalarAbiValue {
            value: callee_scalar,
            scalar_type: integer,
            placement: mixed_call_plan.parameters[0].clone(),
        }],
        structural_parameters: vec![callee_parameter],
        result: FixedIntegerScalarAbiValue {
            value: callee_result,
            scalar_type: integer,
            placement: mixed_call_plan.result.clone().unwrap(),
        },
    };
    AssignedOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x99; 32]),
        },
        target,
        entry: caller,
        functions: vec![
            AssignedFunction {
                machine: caller,
                attachment: Some(structural_type),
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![constant_operation, call_operation],
                    edges: vec![EdgeId::new(9920).unwrap()],
                },
                operation: AssignedOperation::UnitBody(AssignedUnitBody {
                    structural_types: vec![declaration],
                    call_plan: caller_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![caller_parameter],
                    operations: vec![
                        AssignedUnitOperation::IntegerConstant {
                            psi_operation: constant_operation,
                            result: constant,
                            scalar_type: integer,
                            value: IntegerValue::Signed(17),
                        },
                        AssignedUnitOperation::StructuralScalarCall {
                            psi_operation: call_operation,
                            result: AbstractResult {
                                value: call_result,
                                scalar_type: ScalarType::Integer(integer),
                            },
                            callee,
                            call_plan: mixed_call_plan.clone(),
                            scalar_arguments: vec![AssignedUnitScalarCallArgument {
                                parameter_index: 0,
                                source: AssignedUnitScalarArgumentSource::IntegerImmediate {
                                    defining_operation: constant_operation,
                                    source_value: constant,
                                    scalar_type: integer,
                                    value: IntegerValue::Signed(17),
                                },
                                destination: scalar_destination(&mixed_call_plan.parameters[0]),
                            }],
                            copies: vec![AssignedAggregateCopy {
                                place: caller_place,
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                                root_structural_type: structural_type,
                                structural_type,
                                shape,
                                source_byte_offset: 0,
                                fixed_array_length: None,
                                element_stride: None,
                                source: caller_plan.parameters[0].clone(),
                                destination: mixed_call_plan.parameters[1].clone(),
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        AssignedUnitOperation::Return {
                            psi_edge: EdgeId::new(9920).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            AssignedFunction {
                machine: callee,
                attachment: Some(structural_type),
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: Some(mixed_abi),
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![EdgeId::new(9921).unwrap()],
                },
                operation: AssignedOperation::ReturnIntegerImmediate {
                    psi_edge: EdgeId::new(9921).unwrap(),
                    source_value: callee_result,
                    scalar_type: integer,
                    value: IntegerValue::Signed(7),
                },
            },
        ],
    }
}

fn emitted(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    emit_machine_code(&mixed_plan(target)).expect("emit mixed call")
}

#[test]
fn object_replays_nonempty_mixed_scalar_and_structural_rosters() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let machine = emitted(target);
        let object = build_object_artifact(&machine).expect("replay mixed object custody");
        assert_eq!(
            object.functions()[0].internal_unit_calls[0]
                .scalar_arguments
                .len(),
            1
        );
        assert_eq!(
            object.functions()[1].mixed_structural_scalar_abi,
            machine.functions[1].mixed_structural_scalar_abi
        );
    }
}

#[test]
fn object_rejects_mixed_scalar_roster_and_abi_drift() {
    let caller = MachineId::new(9920).unwrap();
    let mut missing = emitted(NativeTarget::linux_x64());
    missing.functions[0].internal_unit_calls[0]
        .scalar_arguments
        .clear();
    assert_eq!(
        build_object_artifact(&missing),
        Err(ObjectError::InvalidInternalUnitCallEvidence(caller))
    );

    let mut changed_type = emitted(NativeTarget::linux_arm64());
    changed_type.functions[1]
        .mixed_structural_scalar_abi
        .as_mut()
        .unwrap()
        .scalar_parameters[0]
        .scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    assert_eq!(
        build_object_artifact(&changed_type),
        Err(ObjectError::InvalidInternalUnitCallEvidence(caller))
    );
}
