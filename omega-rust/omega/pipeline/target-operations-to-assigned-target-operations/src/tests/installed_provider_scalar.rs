use crate::{AssignmentError, assign_registers};
use assigned_target_operations::{
    AssignedOperation, AssignedScalarLocation, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetFunction, TargetOperation, TargetOperationPlan, TargetUnitBody, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TerminalPsiProvenance,
    UnitScalarAbiValue,
};
use terminal_psi::{
    ProviderCandidateConformance, ProviderUnitRefinement, ProviderUnitSignature,
    SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker,
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
        signature: ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderUnitRefinement {
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
fn exact_i32_installed_provider_keeps_parameter_and_provider_custody() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assign_registers(&fixture(target)).expect("exact scalar provider assigns");
        let AssignedOperation::UnitBody(body) = &assigned.functions[0].operation else {
            panic!("caller remains Unit")
        };
        assert_eq!(body.scalar_parameters.len(), 1);
        assert!(matches!(
            body.operations.as_slice(),
            [AssignedUnitOperation::InstalledProviderCall {
                boundary,
                provider,
                scalar_arguments,
                copies,
                ..
            }, AssignedUnitOperation::Return { .. }]
                if *boundary == BoundaryMachineId::new(970).unwrap()
                    && provider.candidate == MachineId::new(971).unwrap()
                    && copies.is_empty()
                    && matches!(scalar_arguments.as_slice(), [argument]
                        if matches!(argument.source,
                            AssignedUnitScalarArgumentSource::Parameter {
                                parameter_index: 0,
                                source_value,
                                scalar_type,
                                location: AssignedScalarLocation::Register(_),
                            } if source_value == ValueId::new(970).unwrap()
                                && scalar_type == ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Signed, 32).unwrap()
                                )))
        ));
    }
}

#[test]
fn scalar_installed_provider_rejects_source_parameter_substitution() {
    let mut plan = fixture(NativeTarget::linux_x64());
    let TargetOperation::UnitBody(body) = &mut plan.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::InstalledProviderCall {
        scalar_arguments, ..
    } = &mut body.operations[0]
    else {
        unreachable!()
    };
    let TargetUnitScalarArgumentSource::Parameter { source_value, .. } =
        &mut scalar_arguments[0].source
    else {
        unreachable!()
    };
    *source_value = ValueId::new(9_700).unwrap();
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::InstalledProviderScalarCallCustodyMismatch { .. })
    ));
}

#[test]
fn scalar_installed_provider_rejects_empty_call_plan_parameters_without_panicking() {
    let mut caller_plan = fixture(NativeTarget::linux_x64());
    let TargetOperation::UnitBody(body) = &mut caller_plan.functions[0].operation else {
        unreachable!()
    };
    body.call_plan.parameters.clear();
    assert!(matches!(
        assign_registers(&caller_plan),
        Err(AssignmentError::InstalledProviderScalarCallCustodyMismatch { .. })
    ));

    let mut callee_plan = fixture(NativeTarget::linux_x64());
    let TargetOperation::UnitBody(body) = &mut callee_plan.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::InstalledProviderCall { call_plan, .. } = &mut body.operations[0]
    else {
        unreachable!()
    };
    call_plan.parameters.clear();
    assert!(matches!(
        assign_registers(&callee_plan),
        Err(AssignmentError::InstalledProviderScalarCallCustodyMismatch { .. })
    ));
}

#[test]
fn scalar_installed_provider_rejects_a_second_caller_saved_use() {
    let mut plan = fixture(NativeTarget::linux_x64());
    let TargetOperation::UnitBody(body) = &mut plan.functions[0].operation else {
        unreachable!()
    };
    let repeated = body.operations[0].clone();
    body.operations.insert(1, repeated);
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::InstalledProviderScalarCallCustodyMismatch { .. })
    ));
}
