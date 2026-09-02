use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_image_emission::{
    ObjectError, build_object_artifact, emit_executable_image, validate_executable_image,
};
use omega_machine_code::{
    InternalUnitScalarArgumentSourceRecord, UnitScalarParameterLocationRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{
    FixedIntegerScalarAbiValue, MachineRegister, TargetFunction, TargetOperation,
    TargetOperationPlan, TargetUnitBody, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ValueId,
};
use psi_terminal::{
    ProviderCandidateConformance, ProviderUnitRefinement, ProviderUnitSignature,
    SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker,
};

const CALLER_RAW: u64 = 970;
const CANDIDATE_RAW: u64 = 971;
const OPERATION_RAW: u64 = 970;

fn emitted_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let caller = MachineId::new(CALLER_RAW).unwrap();
    let candidate = MachineId::new(CANDIDATE_RAW).unwrap();
    let boundary = BoundaryMachineId::new(970).unwrap();
    let operation = OperationId::new(OPERATION_RAW).unwrap();
    let source_value = ValueId::new(970).unwrap();
    let candidate_value = ValueId::new(971).unwrap();
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let body = |value, operations| {
        TargetOperation::UnitBody(TargetUnitBody {
            structural_types: Vec::new(),
            call_plan: call_plan.clone(),
            scalar_parameters: vec![FixedIntegerScalarAbiValue {
                value,
                scalar_type,
                placement: call_plan.parameters[0].clone(),
            }],
            parameters: Vec::new(),
            operations,
        })
    };
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: "Ping::ping_value".into(),
        provider_identity: "PingProvider".into(),
        candidate_identity: "PingProvider::ping_value".into(),
        candidate,
        signature: ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let target_plan = TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x97; 32]),
        },
        target,
        entry: caller,
        functions: vec![
            TargetFunction {
                machine: caller,
                attachment: None,
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: vec![EdgeId::new(970).unwrap()],
                },
                operation: body(
                    source_value,
                    vec![
                        TargetUnitOperation::InstalledProviderCall {
                            psi_operation: operation,
                            boundary,
                            provider,
                            call_plan: call_plan.clone(),
                            scalar_arguments: vec![TargetUnitScalarCallArgument {
                                parameter_index: 0,
                                source: TargetUnitScalarArgumentSource::Parameter {
                                    parameter_index: 0,
                                    source_value,
                                    scalar_type,
                                },
                                placement: call_plan.parameters[0].clone(),
                            }],
                            source_arguments: Vec::new(),
                            arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            completion_claim_sources: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                        TargetUnitOperation::Return {
                            psi_edge: EdgeId::new(970).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                ),
            },
            TargetFunction {
                machine: candidate,
                attachment: None,
                fixed_integer_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![EdgeId::new(971).unwrap()],
                },
                operation: body(
                    candidate_value,
                    vec![TargetUnitOperation::Return {
                        psi_edge: EdgeId::new(971).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            },
        ],
    };
    let assigned =
        omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
            .unwrap();
    omega_machine_emission::emit_machine_code(&assigned).unwrap()
}

fn expected_custody_error() -> ObjectError {
    ObjectError::InvalidInstalledProviderUnitScalarCallEvidence(MachineId::new(CALLER_RAW).unwrap())
}

#[test]
fn object_replays_and_retains_installed_provider_i32_custody_on_each_architecture() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = emitted_plan(target);
        let artifact = build_object_artifact(&plan).expect("exact provider call reaches object");
        let caller = artifact.entry_function();
        assert_eq!(caller.unit_scalar_abi, plan.functions[0].unit_scalar_abi);
        assert_eq!(
            caller.installed_provider_unit_scalar_calls,
            plan.functions[0].installed_provider_unit_scalar_calls
        );
        let [call] = caller.installed_provider_unit_scalar_calls.as_slice() else {
            panic!("one retained provider call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("one retained scalar argument")
        };
        let expected_register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rdi,
            Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        assert!(matches!(
            argument.source,
            InternalUnitScalarArgumentSourceRecord::Parameter {
                parameter_index: 0,
                location: UnitScalarParameterLocationRecord::Register(register),
                ..
            } if register == expected_register
        ));
        assert_eq!(
            argument.code_offset,
            call.code_offset
                + match target.architecture {
                    Architecture::X86_64 => 4,
                    Architecture::Aarch64 => 0,
                }
        );
        assert_eq!(argument.byte_count, 0);
        let image = emit_executable_image(&artifact, 3)
            .expect("installed-provider scalar call reaches a final image");
        validate_executable_image(&artifact, &image)
            .expect("installed-provider scalar final image replays");
        assert_eq!(image.functions(), artifact.functions());
    }
}

#[test]
fn object_rejects_provider_parameter_abi_and_call_custody_mutations() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut source_value_plan = emitted_plan(target);
        let call = &mut source_value_plan.functions[0].installed_provider_unit_scalar_calls[0];
        let InternalUnitScalarArgumentSourceRecord::Parameter { source_value, .. } =
            &mut call.arguments[0].source
        else {
            unreachable!()
        };
        *source_value = ValueId::new(999).unwrap();
        assert_eq!(
            build_object_artifact(&source_value_plan),
            Err(expected_custody_error())
        );

        let mut source_location = emitted_plan(target);
        let call = &mut source_location.functions[0].installed_provider_unit_scalar_calls[0];
        let InternalUnitScalarArgumentSourceRecord::Parameter { location, .. } =
            &mut call.arguments[0].source
        else {
            unreachable!()
        };
        *location = UnitScalarParameterLocationRecord::IncomingStack { byte_offset: 0 };
        assert_eq!(
            build_object_artifact(&source_location),
            Err(expected_custody_error())
        );

        let mut provider = emitted_plan(target);
        provider.functions[0].installed_provider_unit_scalar_calls[0]
            .provider
            .provider_identity
            .clear();
        assert_eq!(
            build_object_artifact(&provider),
            Err(expected_custody_error())
        );

        let mut call_plan = emitted_plan(target);
        call_plan.functions[0].installed_provider_unit_scalar_calls[0]
            .call_plan
            .result = Some(
            call_plan.functions[0].installed_provider_unit_scalar_calls[0]
                .call_plan
                .parameters[0]
                .clone(),
        );
        assert_eq!(
            build_object_artifact(&call_plan),
            Err(expected_custody_error())
        );

        let mut argument_interval = emitted_plan(target);
        argument_interval.functions[0].installed_provider_unit_scalar_calls[0].arguments[0]
            .byte_count = 1;
        assert_eq!(
            build_object_artifact(&argument_interval),
            Err(expected_custody_error())
        );

        let mut candidate_abi = emitted_plan(target);
        candidate_abi.functions[1]
            .unit_scalar_abi
            .as_mut()
            .unwrap()
            .parameters[0]
            .scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        assert_eq!(
            build_object_artifact(&candidate_abi),
            Err(expected_custody_error())
        );

        let mut ordinal = emitted_plan(target);
        ordinal.functions[0].installed_provider_unit_scalar_calls[0].operation_ordinal += 1;
        assert_eq!(
            build_object_artifact(&ordinal),
            Err(expected_custody_error())
        );

        let mut repeated = emitted_plan(target);
        let duplicate = repeated.functions[0].installed_provider_unit_scalar_calls[0].clone();
        repeated.functions[0]
            .installed_provider_unit_scalar_calls
            .push(duplicate);
        assert!(build_object_artifact(&repeated).is_err());

        let mut bytes = emitted_plan(target);
        let code_offset = bytes.functions[0].installed_provider_unit_scalar_calls[0].code_offset;
        bytes.functions[0].bytes[code_offset] ^= 1;
        assert!(build_object_artifact(&bytes).is_err());
    }
}
