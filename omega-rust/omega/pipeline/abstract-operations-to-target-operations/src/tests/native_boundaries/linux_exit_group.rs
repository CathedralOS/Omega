use super::*;

#[test]
fn linux_exit_group_i32_requires_exact_literal_shape_and_stays_fail_closed_elsewhere() {
    let machine = MachineId::new(901).unwrap();
    let boundary = BoundaryMachineId::new(901).unwrap();
    let constant_operation = OperationId::new(901).unwrap();
    let settlement_operation = OperationId::new(902).unwrap();
    let return_edge = EdgeId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let block = BlockId::new(901).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(i32_type);
    let provider_execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(901).unwrap(),
        902,
        903,
        904,
        905,
    )
    .unwrap();
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Console::exit_process(i32)->Unit".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: constant_operation,
                    result: value,
                    scalar_type,
                    value: IntegerValue::Signed(37),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: abstract_operations::AbstractBoundaryResult::Unit,
                    boundary,
                    arguments: vec![value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let binding = target_operations::BoundarySettlementBinding {
        boundary,
        execution: provider_execution.into(),
        realization: target_operations::LinuxExitGroupI32Realization.into(),
    };

    let x86 = lower_to_target_operations_with_settlements(
        &plan,
        NativeTarget::linux_x64(),
        std::slice::from_ref(&binding),
    )
    .expect("Linux x86-64 exit_group lowering");
    assert_eq!(
        x86,
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::linux_x64(),
            std::slice::from_ref(&binding),
        )
        .expect("deterministic lowering")
    );
    assert!(matches!(
        &x86.functions[0].operation,
        TargetOperation::ExitProcessI32 { argument, nominal_return_edge, .. }
            if argument.source_value == value
                && argument.scalar_type == scalar_type
                && argument.immediate == IntegerValue::Signed(37)
                && argument.destination == MachineRegister::X86Rdi
                && *nominal_return_edge == return_edge
    ));
    let arm = lower_to_target_operations_with_settlements(
        &plan,
        NativeTarget::linux_arm64(),
        std::slice::from_ref(&binding),
    )
    .expect("Linux AArch64 exit_group lowering");
    assert!(matches!(
        &arm.functions[0].operation,
        TargetOperation::ExitProcessI32 { argument, .. }
            if argument.destination == MachineRegister::Aarch64X(0)
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::windows_x64(),
            std::slice::from_ref(&binding),
        ),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::macos_arm64(),
            std::slice::from_ref(&binding),
        ),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));

    let mut wrong_signature = plan;
    wrong_signature.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean;
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &wrong_signature,
            NativeTarget::linux_x64(),
            std::slice::from_ref(&binding),
        ),
        Err(LoweringError::InvalidLinuxExitGroupShape(machine))
    );
}
