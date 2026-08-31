use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_image_emission::{
    INSTALLATION_FORMAT_MARKER, InstallationError, build_installation_record,
    build_object_artifact, decode_installation_record, emit_executable_image,
    encode_installation_record, validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_target::NativeTarget;
use omega_target_operations::{
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, ScalarParameterLocation,
    TargetFunction, TargetOperation, TargetOperationPlan, TargetUnitBody, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ProfileDecisionId,
    StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn scalar_call_plan() -> omega_calling_conventions::CallPlan {
    evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("canonical scalar call plan")
}

fn scalar_transport_plan() -> TargetOperationPlan {
    let caller = MachineId::new(1).expect("caller");
    let callee = MachineId::new(2).expect("callee");
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let shape = ValueShape::integer(4, 4);
    let parameter_value = ValueId::new(20).expect("parameter");
    let function_result = ValueId::new(21).expect("function result");
    let constant_value = ValueId::new(10).expect("constant value");
    let call_result = ValueId::new(11).expect("call result");
    let constant_operation = OperationId::new(10).expect("constant operation");
    let call_operation = OperationId::new(11).expect("call operation");
    let call_plan = scalar_call_plan();
    let parameter_register = match call_plan.parameters[0].locations.as_slice() {
        [ValueLocation::Register { register, .. }] => *register,
        _ => panic!("fixed i32 parameter uses one register"),
    };
    let abi = FixedIntegerScalarFunctionAbi {
        parameters: vec![FixedIntegerScalarAbiValue {
            value: parameter_value,
            scalar_type,
            placement: call_plan.parameters[0].clone(),
        }],
        result: FixedIntegerScalarAbiValue {
            value: function_result,
            scalar_type,
            placement: call_plan.result.clone().expect("result placement"),
        },
        call_plan: call_plan.clone(),
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x46; 32]),
        },
        target: NativeTarget::linux_x64(),
        entry: caller,
        functions: vec![
            TargetFunction {
                machine: caller,
                attachment: Some(StructuralTypeId::new(1).expect("attachment")),
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![constant_operation, call_operation],
                    edges: vec![EdgeId::new(1).expect("caller return")],
                },
                operation: TargetOperation::UnitBody(TargetUnitBody {
                    structural_types: Vec::new(),
                    call_plan: evaluate_call_plan(
                        CallingPolicy::SystemVAMD64,
                        &CallSignature::default(),
                    )
                    .expect("Unit call plan"),
                    parameters: Vec::new(),
                    operations: vec![
                        TargetUnitOperation::IntegerConstant {
                            psi_operation: constant_operation,
                            result: constant_value,
                            scalar_type,
                            value: IntegerValue::Signed(-17),
                        },
                        TargetUnitOperation::ScalarCall {
                            psi_operation: call_operation,
                            callee,
                            call_plan: call_plan.clone(),
                            result_home: TargetUnitScalarHomeRequirement {
                                defining_operation: call_operation,
                                source_value: call_result,
                                scalar_type,
                                shape,
                            },
                            arguments: vec![TargetUnitScalarCallArgument {
                                parameter_index: 0,
                                source: TargetUnitScalarArgumentSource::IntegerImmediate {
                                    defining_operation: constant_operation,
                                    source_value: constant_value,
                                    scalar_type,
                                    value: IntegerValue::Signed(-17),
                                },
                                placement: call_plan.parameters[0].clone(),
                            }],
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(1).expect("caller return"),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }),
            },
            TargetFunction {
                machine: callee,
                attachment: None,
                fixed_integer_scalar_abi: Some(abi),
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![EdgeId::new(2).expect("callee return")],
                },
                operation: TargetOperation::ReturnIntegerParameter {
                    psi_edge: EdgeId::new(2).expect("callee return"),
                    source_value: function_result,
                    scalar_type,
                    parameter_index: 0,
                    location: ScalarParameterLocation::Register(parameter_register),
                },
            },
        ],
    }
}

#[test]
fn scalar_installation_record_round_trips_and_rejects_tampering() {
    let target = scalar_transport_plan();
    let assigned = assign_registers(&target).expect("assign scalar call plan");
    let machine = emit_machine_code(&assigned).expect("emit scalar call plan");
    let object = build_object_artifact(&machine).expect("validate scalar call object");
    let image = emit_executable_image(&object, 3).expect("emit scalar call image");
    let record =
        build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
            .expect("build scalar installation record");

    assert!(record.functions()[0].fixed_integer_scalar_abi.is_none());
    assert!(record.functions()[1].fixed_integer_scalar_abi.is_some());
    assert_eq!(record.functions()[0].unit_scalar_homes.len(), 1);
    assert_eq!(record.functions()[0].unit_integer_constants.len(), 1);
    assert_eq!(record.internal_unit_scalar_calls().len(), 1);

    let bytes = encode_installation_record(&record).expect("encode installation");
    let decoded = decode_installation_record(&bytes).expect("decode installation");
    assert_eq!(decoded, record);
    validate_installation_record(&decoded, &image).expect("bind decoded installation to image");

    let mut old_marker = bytes.clone();
    let previous_marker = INSTALLATION_FORMAT_MARKER - 1;
    old_marker[8..10].copy_from_slice(&previous_marker.to_le_bytes());
    assert_eq!(
        decode_installation_record(&old_marker),
        Err(InstallationError::UnsupportedFormatMarker(previous_marker))
    );

    let mut changed_constant = bytes;
    let encoded_constant = (-17_i128).to_le_bytes();
    let constant_offset = changed_constant
        .windows(encoded_constant.len())
        .position(|window| window == encoded_constant)
        .expect("encoded scalar constant");
    changed_constant[constant_offset..constant_offset + 16]
        .copy_from_slice(&i128::MAX.to_le_bytes());
    assert!(decode_installation_record(&changed_constant).is_err());
}
