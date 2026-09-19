use super::super::super::LoweringError;
use super::super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, BlockId,
    BoundaryMachineDeclaration, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue,
    MachineId, NativeTarget, OperationId, ScalarType, TargetUnitOperation, ValueId, identity,
    lower_to_target_operations_with_settlements,
};

fn fixture() -> (
    AbstractOperationPlan,
    target_operations::BoundarySettlementBinding,
) {
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
        structural_types: Vec::new().into(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "Console::exit_process(i32)->Unit".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
    (plan, binding)
}

#[test]
fn hosted_exit_process_i32_retains_runtime_source_abi_and_nonreturning_tail() {
    let (plan, binding) = fixture();
    let machine = MachineId::new(901).unwrap();
    let boundary = BoundaryMachineId::new(901).unwrap();
    let constant_operation = OperationId::new(901).unwrap();
    let settlement_operation = OperationId::new(902).unwrap();
    let return_edge = EdgeId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(i32_type);

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
        let body = &lowered.functions[0].graph;
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
        ] = body.blocks[0].operations.as_slice()
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
        assert!(
            matches!(body.blocks[0].terminator, target_operations::TargetControlTerminator::Return { psi_edge, .. } if psi_edge == return_edge)
        );
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

#[test]
fn hosted_exit_settlement_replays_and_rejects_forged_rows() {
    let (plan, binding) = fixture();
    let constant_operation = OperationId::new(901).unwrap();
    let settlement_operation = OperationId::new(902).unwrap();
    let return_edge = EdgeId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(i32_type);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations_with_settlements(
            &plan,
            native,
            std::slice::from_ref(&binding),
        )
        .unwrap();
        crate::validate_abstract_to_target_translation(&plan, native, &lowered)
            .expect("honest hosted exit settlement replays");

        for mutation in 0..17 {
            let mut changed = lowered.clone();
            {
                let TargetUnitOperation::BoundarySettlement {
                    psi_operation,
                    boundary,
                    result,
                    execution,
                    realization,
                    scalar_arguments,
                    runtime_scalar_arguments,
                    arguments,
                    byte_sequence_arguments,
                    completion_claim_sources,
                    completion_receipts,
                } = &mut changed.functions[0].graph.blocks[0].operations[1]
                else {
                    panic!("boundary settlement row")
                };
                match mutation {
                    0 => *psi_operation = OperationId::new(909).unwrap(),
                    1 => *boundary = BoundaryMachineId::new(909).unwrap(),
                    2 => {
                        *execution = target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                        )
                    }
                    3 => {
                        *realization = target_operations::BoundaryRealization::HostedReadByte(
                            target_operations::HostedReadByteRealization,
                        )
                    }
                    4 => {
                        *realization = target_operations::BoundaryRealization::LinuxWriteLine(
                            target_operations::LinuxWriteLineRealization,
                        )
                    }
                    5 => {
                        *result = target_operations::TargetBoundaryResult::Structural(
                            target_operations::TargetStructuralHomeRequirement {
                                origin:
                                    target_operations::TargetStructuralHomeOrigin::OperationResult {
                                        operation: settlement_operation,
                                        result: terminal_psi::StructuralOperationResult {
                                            place: semantic_vocabulary::PlaceId::new(909).unwrap(),
                                            structural_type:
                                                semantic_vocabulary::StructuralTypeId::new(909)
                                                    .unwrap(),
                                            multiplicity:
                                                terminal_psi::StructuralMultiplicity::Affine,
                                            qualifications: Vec::new(),
                                            projected_qualifications: Vec::new(),
                                            claims: Vec::new(),
                                        },
                                    },
                                layout: target_operations::TargetStructuralHomeLayout::Aggregate(
                                    calling_conventions::ValueShape::integer(4, 4),
                                ),
                            },
                        )
                    }
                    6 => runtime_scalar_arguments[0].parameter_index = 1,
                    7 => {
                        runtime_scalar_arguments[0].placement.shape =
                            calling_conventions::ValueShape::integer(8, 8)
                    }
                    8 => {
                        runtime_scalar_arguments[0].source =
                            target_operations::TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: 0,
                                source_value: value,
                                scalar_type,
                            }
                    }
                    9 => {
                        runtime_scalar_arguments[0].source =
                            target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                                defining_operation: OperationId::new(909).unwrap(),
                                source_value: value,
                                scalar_type: i32_type,
                                value: IntegerValue::Signed(37),
                            }
                    }
                    10 => {
                        runtime_scalar_arguments[0].source =
                            target_operations::TargetUnitScalarArgumentSource::Home(
                                target_operations::TargetUnitScalarHomeRequirement {
                                    defining_operation: constant_operation,
                                    source_value: value,
                                    scalar_type,
                                    shape: calling_conventions::ValueShape::integer(4, 4),
                                },
                            )
                    }
                    11 => runtime_scalar_arguments.clear(),
                    12 => scalar_arguments.push(target_operations::BoundaryScalarArgument {
                        source_value: value,
                        scalar_type,
                        immediate: IntegerValue::Signed(37),
                        destination: calling_conventions::MachineRegister::X86Rax,
                    }),
                    13 => byte_sequence_arguments.push(
                        target_operations::BoundaryByteSequenceArgument {
                            argument: terminal_psi::StructuralArgument {
                                place: semantic_vocabulary::PlaceId::new(909).unwrap(),
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::SharedBorrow,
                            },
                            literal_operation: OperationId::new(909).unwrap(),
                            structural_type: terminal_psi::StructuralTypeDeclaration {
                                id: semantic_vocabulary::StructuralTypeId::new(909).unwrap(),
                                identity: "forged".into(),
                                shape: terminal_psi::StructuralTypeShape::Record {
                                    fields: Vec::new(),
                                },
                            },
                            bytes: vec![b'x'],
                        },
                    ),
                    _ => {
                        if mutation == 14 {
                            arguments.push(terminal_psi::StructuralArgument {
                                place: semantic_vocabulary::PlaceId::new(909).unwrap(),
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::SharedBorrow,
                            });
                        } else if mutation == 15 {
                            completion_claim_sources.push(
                                abstract_operations::CompletionClaimSource {
                                    claim: semantic_vocabulary::ClaimId::new(909).unwrap(),
                                    entry: None,
                                    content: None,
                                },
                            );
                        } else {
                            completion_receipts.push(terminal_psi::CompletionReceipt {
                                claim: semantic_vocabulary::ClaimId::new(909).unwrap(),
                                argument_index: 0,
                            });
                        }
                    }
                }
            }
            let forged_key = (mutation == 0).then_some(OperationId::new(909).unwrap());
            assert_eq!(
                crate::validate_abstract_to_target_translation(&plan, native, &changed),
                Err(
                    crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                        machine: plan.entry,
                        operation: forged_key.unwrap_or(settlement_operation),
                    }
                ),
                "accepted forged settlement mutation {mutation} on {native:?}"
            );
        }

        // A settlement that is not its block's last operation cannot be an
        // exit; the nonreturning tail fails before source identity matters.
        let mut changed = lowered.clone();
        let trailing = changed.functions[0].graph.blocks[0].operations[0].clone();
        changed.functions[0].graph.blocks[0]
            .operations
            .push(trailing);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&plan, native, &changed),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: settlement_operation,
                }
            ),
            "accepted an exit settlement outside the nonreturning tail"
        );

        // A `Return` carrying cleanup actions is not the plain return an exit
        // settlement requires.
        let mut changed = lowered.clone();
        changed.functions[0].graph.blocks[0].terminator =
            target_operations::TargetControlTerminator::Return {
                psi_edge: return_edge,
                cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                    semantic_vocabulary::PlaceId::new(909).unwrap(),
                )],
            };
        assert_eq!(
            crate::validate_abstract_to_target_translation(&plan, native, &changed),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: settlement_operation,
                }
            ),
            "accepted an exit settlement under a cleanup return"
        );

        // A second retained settlement row under the same source operation is
        // forged by construction.
        let mut changed = lowered.clone();
        let duplicate = changed.functions[0].graph.blocks[0].operations[1].clone();
        changed.functions[0].graph.blocks[0]
            .operations
            .insert(1, duplicate);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&plan, native, &changed),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: settlement_operation,
                }
            ),
            "accepted a duplicated settlement row"
        );

        // The source-side shape is replayed too: a boundary call that is not
        // the last non-terminator operation of its block cannot own an exit
        // settlement, even against an otherwise honest target row.
        let mut shifted = plan.clone();
        shifted.functions[0].operations.insert(
            2,
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(904).unwrap(),
                result: ValueId::new(904).unwrap(),
                scalar_type,
                value: IntegerValue::Signed(1),
            },
        );
        assert_eq!(
            crate::validate_abstract_to_target_translation(&shifted, native, &lowered),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: settlement_operation,
                }
            ),
            "accepted an exit settlement for a non-tail source call"
        );

        let mut unit_source = plan.clone();
        let AbstractOperation::BoundaryCall { result, .. } =
            &mut unit_source.functions[0].operations[1]
        else {
            panic!("boundary call")
        };
        *result = abstract_operations::AbstractBoundaryResult::Scalar(
            abstract_operations::AbstractResult {
                value: ValueId::new(905).unwrap(),
                scalar_type,
            },
        );
        assert_eq!(
            crate::validate_abstract_to_target_translation(&unit_source, native, &lowered),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: settlement_operation,
                }
            ),
            "accepted a scalar boundary result on a hosted settlement"
        );
    }
}
