use super::*;

#[test]
fn checked_scalar_call_and_literal_exit_compose_in_one_shared_unit_body() {
    let mut plan = crate::tests::unit_scalar_calls::attached_unit_scalar_call_plan();
    let boundary = BoundaryMachineId::new(970).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let exit_value = ValueId::new(970).unwrap();
    plan.boundary_machines.push(BoundaryMachineDeclaration {
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
    });
    let caller = &mut plan.functions[0];
    let return_operation = caller.operations.pop().expect("caller return");
    caller.operations.extend([
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(970).unwrap(),
            result: exit_value,
            scalar_type,
            value: IntegerValue::Signed(37),
        },
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(971).unwrap(),
            result: abstract_operations::AbstractBoundaryResult::Unit,
            boundary,
            arguments: vec![exit_value],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
        return_operation,
    ]);
    let settlement = target_operations::BoundarySettlementBinding {
        boundary,
        execution: target_operations::ProviderExecutionBinding::from_execution_record(
            target_operations::ProviderPlanReportIdentity::new(970).unwrap(),
            971,
            972,
            973,
            974,
        )
        .unwrap()
        .into(),
        realization: target_operations::LinuxExitGroupI32Realization.into(),
    };

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations_with_settlements(
            &plan,
            target,
            std::slice::from_ref(&settlement),
        )
        .expect("checked scalar call before literal exit lowers as a validated Unit body");
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("checked call and exit must remain one Unit body")
        };
        assert!(
            body.operations
                .iter()
                .any(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
        );
        assert!(body.operations.iter().any(|operation| matches!(
            operation,
            TargetUnitOperation::BoundarySettlement {
                realization: target_operations::BoundaryRealization::LinuxExitGroupI32(_),
                scalar_arguments,
                ..
            } if scalar_arguments[0].immediate == IntegerValue::Signed(37)
        )));
    }

    let mut multi_block = plan;
    multi_block.functions[0]
        .block_entries
        .push(AbstractBlockEntry {
            block: BlockId::new(970).unwrap(),
            parameters: Vec::new(),
            operation_offset: 3,
        });
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &multi_block,
            NativeTarget::linux_x64(),
            std::slice::from_ref(&settlement),
        ),
        Err(LoweringError::InvalidLinuxExitGroupShape(multi_block.entry)),
    );
}
