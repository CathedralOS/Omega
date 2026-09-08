use super::*;

#[test]
fn hosted_exit_process_i32_retains_runtime_source_abi_and_nonreturning_tail() {
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
                structural_parameters: Vec::new(),
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
        realization: target_operations::HostedExitProcessI32Realization.into(),
    };

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations_with_settlements(
            &plan,
            target,
            std::slice::from_ref(&binding),
        )
        .unwrap();
        assert_eq!(
            lowered,
            lower_to_target_operations_with_settlements(
                &plan,
                target,
                std::slice::from_ref(&binding)
            )
            .unwrap()
        );
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("ordinary Unit body");
        };
        let [
            TargetUnitOperation::IntegerConstant { .. },
            TargetUnitOperation::BoundarySettlement {
                psi_operation,
                boundary: actual_boundary,
                execution,
                realization,
                scalar_arguments,
                runtime_scalar_arguments,
                ..
            },
            TargetUnitOperation::Return { psi_edge, .. },
        ] = body.operations.as_slice()
        else {
            panic!("ordered constant, exit, nominal return");
        };
        assert_eq!(*psi_operation, settlement_operation);
        assert_eq!(*actual_boundary, boundary);
        assert_eq!(*execution, binding.execution);
        assert_eq!(
            *realization,
            target_operations::BoundaryRealization::HostedExitProcessI32(Default::default())
        );
        assert_eq!(*psi_edge, return_edge);
        assert!(scalar_arguments.is_empty());
        let [argument] = runtime_scalar_arguments.as_slice() else {
            panic!("one i32 source");
        };
        assert_eq!(argument.parameter_index, 0);
        assert_eq!(
            argument.source,
            target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation: constant_operation,
                source_value: value,
                scalar_type: i32_type,
                value: IntegerValue::Signed(37),
            }
        );
        let call_plan = calling_conventions::evaluate_call_plan(
            calling_conventions::CallingPolicy::native_for_target(target),
            &calling_conventions::CallSignature {
                parameters: vec![calling_conventions::ValueShape::integer(4, 4)],
                result: None,
            },
        )
        .unwrap();
        assert_eq!(argument.placement, call_plan.parameters[0]);
    }
    for target in [
        NativeTarget::windows_x64(),
        NativeTarget {
            pointer_size: 4,
            ..NativeTarget::macos_arm64()
        },
    ] {
        assert_eq!(
            lower_to_target_operations_with_settlements(
                &plan,
                target,
                std::slice::from_ref(&binding)
            ),
            Err(LoweringError::HostedExitProcessUnsupportedTarget { machine, target })
        );
    }
    let mut wrong_signature = plan.clone();
    wrong_signature.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean;
    assert!(
        lower_to_target_operations_with_settlements(
            &wrong_signature,
            NativeTarget::linux_x64(),
            std::slice::from_ref(&binding)
        )
        .is_err()
    );

    let mut after_exit = plan;
    after_exit.functions[0].operations.insert(
        2,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(903).unwrap(),
            result: ValueId::new(903).unwrap(),
            scalar_type,
            value: IntegerValue::Signed(1),
        },
    );
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &after_exit,
            NativeTarget::linux_x64(),
            std::slice::from_ref(&binding)
        ),
        Err(LoweringError::InvalidHostedExitProcessShape(machine))
    );
}
