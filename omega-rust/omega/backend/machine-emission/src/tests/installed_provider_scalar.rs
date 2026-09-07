use super::{EmissionError, emit_machine_code};
use assigned_target_operations::{
    AssignedOperation, AssignedScalarLocation, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_code::{InternalUnitScalarArgumentSourceRecord, UnitScalarParameterLocationRecord};
use semantic_vocabulary::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use target::{Architecture, NativeTarget};
use target_operations::{
    CallSiteOwner, MachineRegister, TargetFunction, TargetOperation, TargetOperationPlan,
    TargetUnitBody, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TerminalPsiProvenance, UnitScalarAbiValue,
};
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_psi::{
    ProviderCandidateConformance, ProviderRefinement, ProviderSignature, SemanticFingerprint,
    TerminalPsiIdentity, VocabularyMarker,
};

fn fixture(target: NativeTarget) -> TargetOperationPlan {
    let caller = MachineId::new(970).unwrap();
    let candidate = MachineId::new(971).unwrap();
    let boundary = BoundaryMachineId::new(970).unwrap();
    let operation = OperationId::new(970).unwrap();
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
            scalar_parameters: vec![UnitScalarAbiValue {
                value,
                scalar_type: ScalarType::Integer(scalar_type),
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
        signature: ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    TargetOperationPlan {
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
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
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
                                    scalar_type: ScalarType::Integer(scalar_type),
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
                mixed_structural_scalar_abi: None,
                provenance: TerminalPsiProvenance::default(),
                operation: body(
                    candidate_value,
                    vec![TargetUnitOperation::Return {
                        psi_edge: EdgeId::new(971).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            },
        ],
    }
}

#[test]
fn installed_provider_i32_emits_exact_call_and_parameter_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emit_machine_code(&fixture(target)).expect("exact scalar provider emits");
        let caller = &emitted.functions[0];
        let candidate = &emitted.functions[1];
        let caller_abi = caller
            .unit_scalar_abi
            .as_ref()
            .expect("caller Unit scalar ABI");
        let candidate_abi = candidate
            .unit_scalar_abi
            .as_ref()
            .expect("candidate Unit scalar ABI");
        assert_eq!(caller_abi.call_plan, candidate_abi.call_plan);
        assert_eq!(caller_abi.parameters.len(), 1);
        assert_eq!(candidate_abi.parameters.len(), 1);

        let [call] = caller.installed_provider_unit_scalar_calls.as_slice() else {
            panic!("one installed-provider scalar call")
        };
        assert_eq!(
            call.owner,
            CallSiteOwner::Operation(OperationId::new(970).unwrap())
        );
        assert_eq!(call.boundary, BoundaryMachineId::new(970).unwrap());
        assert_eq!(call.provider.candidate, MachineId::new(971).unwrap());
        assert_eq!(call.call_plan, caller_abi.call_plan);
        assert!(call.source_arguments.is_empty());
        assert!(call.claim_transfers.is_empty());
        assert!(call.completion_claim_sources.is_empty());
        assert!(call.completion_receipts.is_empty());
        let [argument] = call.arguments.as_slice() else {
            panic!("one scalar argument")
        };
        let expected_register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rdi,
            Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(argument.byte_count, 0);
        assert!(matches!(
            argument.source,
            InternalUnitScalarArgumentSourceRecord::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type,
                location: UnitScalarParameterLocationRecord::Register(register),
            } if source_value == ValueId::new(970).unwrap()
                && scalar_type == ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap()
                )
                && register == expected_register
        ));
        let call_bytes = &caller.bytes[call.code_offset..call.code_offset + call.byte_count];
        match target.architecture {
            Architecture::X86_64 => {
                assert_eq!(
                    call_bytes,
                    [
                        0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08,
                    ]
                );
                assert_eq!(argument.code_offset, call.code_offset + 4);
            }
            Architecture::Aarch64 => {
                assert_eq!(call_bytes, 0x9400_0000_u32.to_le_bytes());
                assert_eq!(argument.code_offset, call.code_offset);
            }
        }
        let [relocation] = caller.internal_calls.as_slice() else {
            panic!("one internal relocation")
        };
        assert_eq!(relocation.owner, call.owner);
        assert_eq!(relocation.target, call.provider.candidate);
        assert!(relocation.scalar_stack.is_none());
        let outbound = relocation
            .unit_stack
            .as_ref()
            .expect("Unit stack evidence")
            .outbound;
        match target.architecture {
            Architecture::X86_64 => {
                assert_eq!(outbound.expect("x86 outbound alignment").byte_size, 8)
            }
            Architecture::Aarch64 => assert!(outbound.is_none()),
        }
    }
}

#[test]
fn installed_provider_i32_rejects_assigned_parameter_location_substitution() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut assigned = assign_registers(&fixture(target)).expect("exact fixture assigns");
        let AssignedOperation::UnitBody(body) = &mut assigned.functions[0].operation else {
            unreachable!()
        };
        let AssignedUnitOperation::InstalledProviderCall {
            scalar_arguments, ..
        } = &mut body.operations[0]
        else {
            unreachable!()
        };
        let AssignedUnitScalarArgumentSource::Parameter { location, .. } =
            &mut scalar_arguments[0].source
        else {
            unreachable!()
        };
        *location = AssignedScalarLocation::Register(match target.architecture {
            Architecture::X86_64 => MachineRegister::X86Rsi,
            Architecture::Aarch64 => MachineRegister::Aarch64X(1),
        });
        assert!(matches!(
            crate::emit_machine_code(&assigned),
            Err(EmissionError::InvalidInstalledProviderScalarCallCustody(operation))
                if operation == OperationId::new(970).unwrap()
        ));
    }
}

#[test]
fn installed_provider_i32_rejects_a_second_assigned_caller_saved_use() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut assigned = assign_registers(&fixture(target)).expect("exact fixture assigns");
        let AssignedOperation::UnitBody(body) = &mut assigned.functions[0].operation else {
            unreachable!()
        };
        let repeated = body.operations[0].clone();
        body.operations.insert(1, repeated);
        assert!(matches!(
            crate::emit_machine_code(&assigned),
            Err(EmissionError::InvalidInstalledProviderScalarCallCustody(operation))
                if operation == OperationId::new(970).unwrap()
        ));
    }
}
