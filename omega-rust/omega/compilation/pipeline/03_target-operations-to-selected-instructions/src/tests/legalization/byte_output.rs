//! Native byte-output admission binds the source argument and exact receiving builtin.
use crate::{legalize_target_operations, validate_legalized_operations};
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractParameter};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, ScalarType,
    ValueId,
};
use target::NativeTarget;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetUnitOperation,
};

pub(super) fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let value = ValueId::new(5).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
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
        });
    source.functions[0].parameters = vec![AbstractParameter { value, scalar_type }];
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(7).unwrap(),
            result: AbstractBoundaryResult::Unit,
            boundary,
            arguments: vec![value],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[AdmittedBoundarySettlement {
                boundary,
                execution: AdmittedBoundaryExecution::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                realization: target_operations::HostedWriteByteI32Realization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn byte_output_replays_exact_builtin_argument_and_occurrence() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..4 {
            let mut changed = legal.plan().clone();
            let row = &mut changed.scalar_functions[0].blocks[0].instructions[0];
            match mutation {
                0 => {
                    row.kind =
                        legalized_operations::LegalizedScalarInstructionKind::HostedWriteByteI32 {
                            boundary: BoundaryMachineId::new(2).unwrap(),
                            source: ValueId::new(5).unwrap(),
                        }
                }
                1 => {
                    row.kind =
                        legalized_operations::LegalizedScalarInstructionKind::HostedWriteByteI32 {
                            boundary: BoundaryMachineId::new(1).unwrap(),
                            source: ValueId::new(6).unwrap(),
                        }
                }
                2 => row.operation = OperationId::new(8).unwrap(),
                _ => row.fuel.clear(),
            }
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }
        for mutation in 0..5 {
            let mut changed = target.clone();
            let body = &mut changed.functions[0].graph;
            let TargetUnitOperation::BoundarySettlement {
                execution,
                realization,
                runtime_scalar_arguments,
                boundary,
                ..
            } = &mut body.blocks[0].operations[0]
            else {
                panic!("byte output");
            };
            match mutation {
                0 => {
                    *execution = BoundaryExecutionBinding::CompilerBuiltin(
                        CompilerBuiltinExecution::HostedExitProcessI32,
                    )
                }
                1 => *realization = BoundaryRealization::HostedExitProcessI32(Default::default()),
                2 => *boundary = BoundaryMachineId::new(2).unwrap(),
                3 => runtime_scalar_arguments[0].parameter_index = 1,
                _ => {
                    runtime_scalar_arguments[0].placement.shape =
                        calling_conventions::ValueShape::integer(8, 8)
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn scalar_return_cannot_hide_an_unwitnessed_byte_output_boundary() {
    use abstract_operations::{AbstractFunctionResult, AbstractResult};
    use semantic_vocabulary::{EdgeId, IntegerValue};
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (mut source, valid_unit_target, valid_unit) = fixture(native);
        legalize_target_operations(&valid_unit_target, &source, &valid_unit).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let result = ValueId::new(11).unwrap();
        let value = ValueId::new(12).unwrap();
        source.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
            value: result,
            scalar_type,
        });
        source.functions[0].operations.pop();
        source.functions[0].operations.extend([
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(12).unwrap(),
                result: value,
                scalar_type,
                value: IntegerValue::Unsigned(17),
            },
            AbstractOperation::Return {
                psi_edge: EdgeId::new(1).unwrap(),
                result,
                value,
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ]);
        let mut pure = source.clone();
        pure.functions[0].operations.remove(0);
        let mut forged_target =
            abstract_operations_to_target_operations::lower_to_target_operations(
                &pure,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
        let pure_unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &pure,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        legalize_target_operations(&forged_target, &pure, &pure_unit).unwrap();
        // A raw target can claim the source occurrence in provenance without carrying
        // any boundary execution witness. Provenance alone must not select a syscall.
        forged_target.functions[0]
            .provenance
            .operations
            .insert(0, OperationId::new(7).unwrap());
        let full_unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let error = match legalize_target_operations(&forged_target, &source, &full_unit) {
            Err(error) => error,
            Ok(_) => panic!("scalar target invented a Linux write without a builtin witness"),
        };
        assert!(
            matches!(
                error,
                crate::LegalizationError::SourceCustodyMismatch { .. }
            ),
            "{error:?}"
        );
    }
}

#[test]
fn provider_byte_output_retains_execution_custody_and_normal_return() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, mut target, unit) = fixture(native);
        let provider = target_operations::ProviderExecutionBinding::from_execution_record(
            target_operations::ProviderPlanReportIdentity::new(71).unwrap(),
            72,
            73,
            74,
            75,
        )
        .unwrap();
        let body = &mut target.functions[0].graph;
        let TargetUnitOperation::BoundarySettlement { execution, .. } =
            &mut body.blocks[0].operations[0]
        else {
            panic!("output settlement")
        };
        *execution = BoundaryExecutionBinding::AdmittedProvider(provider);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        for mutation in 0..3 {
            let mut changed = target.clone();
            let body = &mut changed.functions[0].graph;
            let TargetUnitOperation::BoundarySettlement {
                execution,
                realization,
                runtime_scalar_arguments,
                ..
            } = &mut body.blocks[0].operations[0]
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    *execution = BoundaryExecutionBinding::CompilerBuiltin(
                        CompilerBuiltinExecution::HostedExitProcessI32,
                    )
                }
                1 => *realization = BoundaryRealization::HostedExitProcessI32(Default::default()),
                _ => {
                    runtime_scalar_arguments[0].placement.shape =
                        calling_conventions::ValueShape::integer(8, 8)
                }
            }
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                    .is_err()
            );
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
        }
    }
}

fn scalar_result_fixture(
    native: NativeTarget,
    borrowed: bool,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    use abstract_operations::{AbstractFunctionResult, AbstractResult};
    use semantic_vocabulary::{EdgeId, IntegerValue, PlaceId, ServiceId, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };

    let (mut source, _, _) = fixture(native);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let result = ValueId::new(11).unwrap();
    let value = ValueId::new(12).unwrap();
    let service = ServiceId::new(1).unwrap();
    source.boundary_machines[0].published_service_ceiling = vec![service];
    source.boundary_machines[0].fixed_service_reach = vec![service];
    let function = &mut source.functions[0];
    function.published_service_ceiling = vec![service];
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type,
    });
    function.operations.pop();
    function.operations.extend([
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(12).unwrap(),
            result: value,
            scalar_type,
            value: IntegerValue::Unsigned(17),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result,
            value,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ]);
    if borrowed {
        let structural_type = StructuralTypeId::new(1).unwrap();
        source
            .structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: structural_type,
                identity: "scalar-result::Bytes".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                }),
            });
        function
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
    }
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[AdmittedBoundarySettlement {
                boundary: BoundaryMachineId::new(1).unwrap(),
                execution: AdmittedBoundaryExecution::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                realization: target_operations::HostedWriteByteI32Realization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    unit.services = vec![terminal_psi::ServiceDeclaration {
        id: service,
        identity: "test::Console".into(),
        parents: Vec::new(),
    }]
    .into();
    unit.root_service_reach.concrete = vec![service];
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

#[test]
fn scalar_results_replay_ordered_boundary_settlements_and_return_value() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for borrowed in [false, true] {
            let (source, target, unit) = scalar_result_fixture(native, borrowed);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            assert!(
                matches!(legal.plan().scalar_functions[0].blocks[0].instructions[0].kind,
                legalized_operations::LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source }
                    if boundary == BoundaryMachineId::new(1).unwrap() && source == ValueId::new(5).unwrap())
            );
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert!(legal.plan().scalar_functions[0].call_plan.result.is_some());
            assert_eq!(selected.plan().functions[0].boundary_settlements.len(), 1);
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            for mutation in [
                "execution",
                "realization",
                "boundary",
                "operation",
                "argument",
                "placement",
                "missing",
                "duplicate",
                "order",
            ] {
                let mut changed = target.clone();
                let rows = &mut changed.functions[0].graph.blocks[0].operations;
                match mutation {
                    "missing" => {
                        rows.remove(0);
                    }
                    "duplicate" => {
                        rows.insert(0, rows[0].clone());
                    }
                    "order" => {
                        rows.swap(0, 1);
                    }
                    _ => {
                        let TargetUnitOperation::BoundarySettlement {
                            execution,
                            realization,
                            boundary,
                            psi_operation,
                            runtime_scalar_arguments,
                            ..
                        } = &mut rows[0]
                        else {
                            panic!("settlement")
                        };
                        match mutation {
                            "execution" => {
                                *execution = BoundaryExecutionBinding::CompilerBuiltin(
                                    CompilerBuiltinExecution::HostedExitProcessI32,
                                )
                            }
                            "realization" => {
                                *realization =
                                    BoundaryRealization::HostedExitProcessI32(Default::default())
                            }
                            "boundary" => *boundary = BoundaryMachineId::new(99).unwrap(),
                            "operation" => *psi_operation = OperationId::new(99).unwrap(),
                            "argument" => runtime_scalar_arguments[0].parameter_index = 1,
                            "placement" => {
                                runtime_scalar_arguments[0].placement.shape =
                                    calling_conventions::ValueShape::integer(8, 8)
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{mutation}"
                );
                assert!(
                    validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                        .is_err(),
                    "{mutation}"
                );
            }
            // This publication grants no permission to weaken boundary reach.
            let mut changed = unit.clone();
            changed.functions[0].published_service_ceiling.clear();
            changed.identity =
                optimization_unit::recompute_psi_optimization_unit_identity(&changed);
            assert!(optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err());
            assert!(legalize_target_operations(&target, &source, &changed).is_err());
        }
    }
}
