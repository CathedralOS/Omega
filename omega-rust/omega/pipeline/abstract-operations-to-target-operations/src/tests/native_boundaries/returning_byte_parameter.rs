//! Machine parameters do not depend on the body matching a special entry shape.

use super::super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, BlockId, BoundaryMachineDeclaration,
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, NativeTarget, OperationId,
    ScalarType, TargetUnitOperation, ValueId, identity,
};

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
    // x86-64 on Mach-O is not a canonical profile. Since 5213d05f76 the native
    // callable matrix fails closed with a panic on that undeclared
    // (architecture, object-format) pair, and `derive_fixed_scalar_function_abi`
    // consults it before any realization check, so only the realization-level
    // rejection is observable here; the fail-closed arm is pinned in
    // calling-conventions. The lowering rows below all use declared pairs.
    assert!(
        !target_operations::HostedWriteByteI32Realization::supports_target(NativeTarget {
            architecture: target::Architecture::X86_64,
            ..NativeTarget::macos_arm64()
        })
    );
    for target in [
        NativeTarget::windows_x64(),
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
            crate::lower_to_target_operations(
                &plan,
                crate::TargetLoweringRequest {
                    target,
                    settlements: std::slice::from_ref(&binding),
                    installation: None,
                    ieee_float_fma: &[]
                }
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
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "Console::write_byte(i32)->Unit".into(),
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
            crate::lower_to_target_operations(
                plan,
                crate::TargetLoweringRequest {
                    target,
                    settlements: std::slice::from_ref(&binding),
                    installation: None,
                    ieee_float_fma: &[],
                },
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

#[test]
fn hosted_write_settlement_replays_and_rejects_forged_rows() {
    let plan = fixture();
    let binding = crate::AdmittedBoundarySettlement {
        boundary: plan.boundary_machines[0].id,
        execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
        ),
        realization: target_operations::HostedWriteByteI32Realization.into(),
    };
    let value = plan.functions[0].parameters[0].value;
    let scalar_type = plan.functions[0].parameters[0].scalar_type;
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = crate::lower_to_target_operations(
            &plan,
            crate::TargetLoweringRequest {
                target: native,
                settlements: std::slice::from_ref(&binding),
                installation: None,
                ieee_float_fma: &[],
            },
        )
        .unwrap();
        crate::validate_abstract_to_target_translation(&plan, native, &lowered)
            .expect("honest hosted write settlement replays");

        for mutation in 0..15 {
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
                    completion_claim_sources,
                    completion_receipts,
                    ..
                } = &mut changed.functions[0].graph.blocks[0].operations[0]
                else {
                    panic!("boundary settlement row")
                };
                match mutation {
                    0 => *psi_operation = OperationId::new(999).unwrap(),
                    1 => *boundary = BoundaryMachineId::new(999).unwrap(),
                    2 => {
                        *execution = target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedReadByte,
                        )
                    }
                    3 => {
                        *realization = target_operations::BoundaryRealization::HostedReadByte(
                            target_operations::HostedReadByteRealization,
                        )
                    }
                    4 => {
                        *realization = target_operations::BoundaryRealization::ClaimCompletionOnly(
                            target_operations::ClaimCompletionOnlyRealization,
                        )
                    }
                    5 => {
                        *result = target_operations::TargetBoundaryResult::Structural(
                            target_operations::TargetStructuralHomeRequirement {
                                origin:
                                    target_operations::TargetStructuralHomeOrigin::OperationResult {
                                        operation: *psi_operation,
                                        result: terminal_psi::StructuralOperationResult {
                                            place: semantic_vocabulary::PlaceId::new(999).unwrap(),
                                            structural_type:
                                                semantic_vocabulary::StructuralTypeId::new(999)
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
                    6 => runtime_scalar_arguments[0].parameter_index = 7,
                    7 => {
                        runtime_scalar_arguments[0].placement.shape =
                            calling_conventions::ValueShape::integer(8, 8)
                    }
                    8 => {
                        let target_operations::TargetUnitScalarArgumentSource::Parameter {
                            parameter_index,
                            ..
                        } = &mut runtime_scalar_arguments[0].source
                        else {
                            panic!("parameter source")
                        };
                        *parameter_index = 7;
                    }
                    9 => {
                        let target_operations::TargetUnitScalarArgumentSource::Parameter {
                            source_value,
                            ..
                        } = &mut runtime_scalar_arguments[0].source
                        else {
                            panic!("parameter source")
                        };
                        *source_value = ValueId::new(999).unwrap();
                    }
                    10 => {
                        runtime_scalar_arguments[0].source =
                            target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                                defining_operation: OperationId::new(999).unwrap(),
                                source_value: value,
                                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                                value: semantic_vocabulary::IntegerValue::Signed(37),
                            }
                    }
                    11 => scalar_arguments.push(target_operations::BoundaryScalarArgument {
                        source_value: value,
                        scalar_type,
                        immediate: semantic_vocabulary::IntegerValue::Signed(37),
                        destination: calling_conventions::MachineRegister::X86Rax,
                    }),
                    _ => {
                        if mutation == 12 {
                            arguments.push(terminal_psi::StructuralArgument {
                                place: semantic_vocabulary::PlaceId::new(999).unwrap(),
                                path: Vec::new(),
                                access: terminal_psi::StructuralAccess::SharedBorrow,
                            });
                        } else if mutation == 13 {
                            completion_claim_sources.push(
                                abstract_operations::CompletionClaimSource {
                                    claim: semantic_vocabulary::ClaimId::new(999).unwrap(),
                                    entry: None,
                                    content: None,
                                },
                            );
                        } else {
                            completion_receipts.push(terminal_psi::CompletionReceipt {
                                claim: semantic_vocabulary::ClaimId::new(999).unwrap(),
                                argument_index: 0,
                            });
                        }
                    }
                }
            }
            let forged_key = (mutation == 0).then_some(OperationId::new(999).unwrap());
            assert_eq!(
                crate::validate_abstract_to_target_translation(&plan, native, &changed),
                Err(
                    crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                        machine: plan.entry,
                        operation: forged_key.unwrap_or(OperationId::new(902).unwrap()),
                    }
                ),
                "accepted forged write settlement mutation {mutation} on {native:?}"
            );
        }

        // A duplicated retained row under the same source operation key is
        // forged by construction.
        let mut changed = lowered.clone();
        let duplicate = changed.functions[0].graph.blocks[0].operations[0].clone();
        changed.functions[0].graph.blocks[0]
            .operations
            .push(duplicate);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&plan, native, &changed),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: plan.entry,
                    operation: OperationId::new(902).unwrap(),
                }
            ),
            "accepted a duplicated write settlement row"
        );
    }
}
