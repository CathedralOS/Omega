//! Exact hosted-exit admission, using the existing admitted scalar boundary fixture.
use crate::{legalize_target_operations, validate_legalized_operations};
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerValue, OperationId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetOperation,
    TargetOperationPlan, TargetUnitOperation,
};

fn lower(
    source: &AbstractOperationPlan,
    native: NativeTarget,
    execution: CompilerBuiltinExecution,
) -> TargetOperationPlan {
    let realization = match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => {
            target_operations::HostedExitProcessI32Realization.into()
        }
        CompilerBuiltinExecution::HostedWriteByteI32 => {
            target_operations::HostedWriteByteI32Realization.into()
        }
        _ => panic!("fixture hosted scalar role"),
    };
    abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        source,
        native,
        &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(execution),
            realization,
        }],
    )
    .unwrap()
}

fn seed(source: &AbstractOperationPlan) -> optimization_unit::PsiOptimizationUnit {
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn process_exit_admits_runtime_and_constant_i32_and_replays_exact_role() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for constant in [false, true] {
            let (mut source, _, _) = super::byte_output::fixture(native);
            if constant {
                let parameter = source.functions[0].parameters.remove(0);
                source.functions[0].operations.insert(
                    0,
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(6).unwrap(),
                        result: parameter.value,
                        scalar_type: parameter.scalar_type,
                        value: IntegerValue::Signed(-1),
                    },
                );
            }
            let target = lower(
                &source,
                native,
                CompilerBuiltinExecution::HostedExitProcessI32,
            );
            let unit = seed(&source);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let mut wrong_return = legal.plan().clone();
            let legalized_operations::LegalizedScalarTerminator::Return(returned) =
                &mut wrong_return.scalar_functions[0].blocks[0].terminator
            else {
                panic!("return");
            };
            returned.value = legalized_operations::LegalizedScalarReturnValue::Value {
                value: ValueId::new(5).unwrap(),
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .unwrap(),
            };
            assert!(validate_legalized_operations(&target, &source, &unit, wrong_return).is_err());
            let position = usize::from(constant);
            for mutation in 0..4 {
                let mut candidate = legal.plan().clone();
                let row = &mut candidate.scalar_functions[0].blocks[0].instructions[position];
                row.kind = match mutation {
                    0 => legalized_operations::LegalizedScalarInstructionKind::HostedWriteByteI32 {
                        boundary: BoundaryMachineId::new(1).unwrap(),
                        source: ValueId::new(5).unwrap(),
                    },
                    1 => {
                        legalized_operations::LegalizedScalarInstructionKind::HostedExitProcessI32 {
                            boundary: BoundaryMachineId::new(2).unwrap(),
                            source: ValueId::new(5).unwrap(),
                        }
                    }
                    2 => {
                        legalized_operations::LegalizedScalarInstructionKind::HostedExitProcessI32 {
                            boundary: BoundaryMachineId::new(1).unwrap(),
                            source: ValueId::new(99).unwrap(),
                        }
                    }
                    _ => {
                        row.operation = OperationId::new(99).unwrap();
                        row.kind.clone()
                    }
                };
                assert!(validate_legalized_operations(&target, &source, &unit, candidate).is_err());
            }
            let mut changed = target.clone();
            changed.target.pointer_size = 4;
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
        }
    }
}

#[test]
fn exit_admission_rejects_an_operation_after_the_boundary() {
    let native = NativeTarget::macos_arm64();
    let (mut source, _, _) = super::byte_output::fixture(native);
    let parameter = source.functions[0].parameters[0];
    source.functions[0].operations.insert(
        1,
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(8).unwrap(),
            result: ValueId::new(8).unwrap(),
            scalar_type: parameter.scalar_type,
            value: IntegerValue::Signed(1),
        },
    );
    // A returning write legitimately admits this tail. Substituting an exit must not.
    let mut target = lower(
        &source,
        native,
        CompilerBuiltinExecution::HostedWriteByteI32,
    );
    let unit = seed(&source);
    legalize_target_operations(&target, &source, &unit).unwrap();
    let TargetOperation::UnitBody(body) = &mut target.functions[0].operation else {
        panic!("Unit body");
    };
    let TargetUnitOperation::BoundarySettlement {
        execution,
        realization,
        ..
    } = &mut body.operations[0]
    else {
        panic!("boundary");
    };
    *execution =
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::HostedExitProcessI32);
    *realization = BoundaryRealization::HostedExitProcessI32(Default::default());
    assert!(legalize_target_operations(&target, &source, &unit).is_err());
}

#[test]
fn process_exit_retains_provider_specialization_and_service_custody_without_storage() {
    use semantic_vocabulary::{
        PlaceId, ServiceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    };
    let native = NativeTarget::macos_arm64();
    let (mut source, _, _) = super::byte_output::fixture(native);
    let attachment = StructuralTypeId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    let service = ServiceId::new(1).unwrap();
    source
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: attachment,
            identity: "exit::Owner".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: field,
                    identity: "console".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Erased {
                        type_identity: "exit::Console".into(),
                    },
                }],
            },
        });
    source.functions[0].attachment = Some(attachment);
    source.functions[0].published_service_ceiling = vec![service];
    source.boundary_machines[0].published_service_ceiling = vec![service];
    let target = lower(
        &source,
        native,
        CompilerBuiltinExecution::HostedExitProcessI32,
    );
    let mut unit = seed(&source);
    unit.services = vec![terminal_psi::ServiceDeclaration {
        id: service,
        identity: "exit::Console".into(),
        parents: Vec::new(),
    }]
    .into();
    // The seed has no service catalog/reach payload. This fixture's reachable
    // concrete boundary contributes its declared service to the entry reach.
    unit.root_service_reach.concrete = vec![service];
    unit.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary: BoundaryMachineId::new(1).unwrap(),
            },
        });
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    assert!(legal.plan().scalar_functions[0].structural.is_none());
    for mutation in 0..8 {
        let mut changed = unit.clone();
        match mutation {
            0 => changed.functions[0].published_service_ceiling.clear(),
            1 => changed.services = Vec::new().into(),
            2 => changed.functions[0].structural_places.clear(),
            3 => {
                changed.functions[0]
                    .declared_places
                    .insert(PlaceId::new(1).unwrap());
            }
            7 => changed.boundary_machines[0]
                .published_service_ceiling
                .clear(),
            _ => {
                let StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field,
                    boundary,
                } = &mut changed.functions[0].structural_places[0].kind
                else {
                    panic!("provider root");
                };
                match mutation {
                    4 => *attachment = StructuralTypeId::new(2).unwrap(),
                    5 => *field = StructuralFieldId::new(2).unwrap(),
                    _ => *boundary = BoundaryMachineId::new(2).unwrap(),
                }
            }
        }
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(
            legalize_target_operations(&target, &source, &changed).is_err(),
            "metadata mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&target, &source, &changed, legal.plan().clone())
                .is_err()
        );
    }
    let mut changed_source = source.clone();
    changed_source.functions[0]
        .published_service_ceiling
        .clear();
    assert!(legalize_target_operations(&target, &changed_source, &unit).is_err());
    let mut changed_source = source.clone();
    changed_source.boundary_machines[0]
        .published_service_ceiling
        .clear();
    assert!(legalize_target_operations(&target, &changed_source, &unit).is_err());
    // Matching declarations cannot erase a boundary's required caller permission.
    let mut changed_source = source.clone();
    changed_source.functions[0]
        .published_service_ceiling
        .clear();
    let mut changed = unit.clone();
    changed.functions[0].published_service_ceiling.clear();
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(legalize_target_operations(&target, &changed_source, &changed).is_err());
}
