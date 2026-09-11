//! Machine parameters do not depend on the body matching a special entry shape.
use super::*;

#[test]
fn hosted_byte_output_rejects_noncanonical_or_unsupported_targets() {
    let plan = fixture();
    let binding = crate::AdmittedBoundarySettlement {
        boundary: plan.boundary_machines[0].id,
        execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
        ),
        realization: target_operations::HostedWriteByteI32Realization.into(),
    };
    for target in [
        NativeTarget::windows_x64(),
        NativeTarget {
            architecture: target::Architecture::X86_64,
            ..NativeTarget::macos_arm64()
        },
        NativeTarget {
            pointer_size: 4,
            ..NativeTarget::macos_arm64()
        },
        NativeTarget {
            pointer_alignment: 4,
            ..NativeTarget::linux_arm64()
        },
    ] {
        assert!(!target_operations::HostedWriteByteI32Realization::supports_target(target));
        assert!(
            crate::lower_to_target_operations_with_provider_executions(
                &plan,
                target,
                std::slice::from_ref(&binding),
            )
            .is_err(),
            "unsupported target {target:?}"
        );
    }
}

pub(super) fn fixture() -> AbstractOperationPlan {
    let machine = MachineId::new(901).unwrap();
    let boundary = BoundaryMachineId::new(901).unwrap();
    let block = BlockId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new().into(),
        provider_candidates: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Console::write_byte(i32)->Unit".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: vec![AbstractParameter { value, scalar_type }],
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                structural_parameters: Vec::new(),
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::BoundaryCall {
                    psi_operation: OperationId::new(902).unwrap(),
                    result: abstract_operations::AbstractBoundaryResult::Unit,
                    boundary,
                    arguments: vec![value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(901).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

#[test]
fn returning_byte_output_accepts_canonical_empty_or_declared_entry_parameters() {
    let plan = fixture();
    let binding = crate::AdmittedBoundarySettlement {
        boundary: plan.boundary_machines[0].id,
        execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
        ),
        realization: target_operations::HostedWriteByteI32Realization.into(),
    };
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lower = |plan: &AbstractOperationPlan| {
            crate::lower_to_target_operations_with_provider_executions(
                plan,
                target,
                std::slice::from_ref(&binding),
            )
        };
        let expected = lower(&plan).unwrap();
        let body = &expected.functions[0].graph;
        assert_eq!(
            body.scalar_parameters[0].value,
            plan.functions[0].parameters[0].value
        );
        let TargetUnitOperation::BoundarySettlement {
            runtime_scalar_arguments,
            ..
        } = &body.blocks[0].operations[0]
        else {
            panic!("returning byte boundary");
        };
        assert_eq!(
            runtime_scalar_arguments[0].source.source_value(),
            plan.functions[0].parameters[0].value
        );
        assert!(matches!(
            body.blocks[0].terminator,
            target_operations::TargetControlTerminator::Return { .. }
        ));
        let mut declared = plan.clone();
        declared.functions[0].block_entries[0].parameters =
            declared.functions[0].parameters.clone();
        let declared_target = lower(&declared).unwrap();
        let graph = &declared_target.functions[0].graph;
        assert_eq!(graph.call_plan, body.call_plan);
        assert_eq!(graph.blocks[0].operations, body.blocks[0].operations);
        assert_eq!(graph.blocks[0].parameters.len(), 1);
        assert_eq!(
            graph.blocks[0].parameters[0].value,
            declared.functions[0].parameters[0].value
        );

        let mut unused = plan.clone();
        unused.functions[0].parameters.push(AbstractParameter {
            value: ValueId::new(903).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        assert_eq!(lower(&unused).unwrap().functions.len(), 1);
        for mutation in 0..5 {
            let mut changed = plan.clone();
            match mutation {
                0 => {
                    changed.functions[0].block_entries[0].parameters =
                        changed.functions[0].parameters.clone();
                    changed.functions[0].block_entries[0].parameters[0].value =
                        ValueId::new(999).unwrap();
                }
                1 => changed.functions[0].parameters[0].scalar_type = ScalarType::Boolean,
                2 => changed.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean,
                3 => changed.functions[0].block_entries.push(AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: BlockId::new(999).unwrap(),
                    parameters: Vec::new(),
                    operation_offset: 1,
                }),
                _ => {
                    changed.functions[0].parameters[0].scalar_type =
                        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                }
            }
            assert!(lower(&changed).is_err(), "parameter corruption {mutation}");
        }
    }
}
