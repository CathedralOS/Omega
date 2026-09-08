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
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetOperation,
    TargetUnitOperation,
};

#[test]
fn hosted_byte_output_replay_rejects_substituted_native_targets() {
    let (source, target, unit) = fixture(NativeTarget::macos_arm64());
    for native in [
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
        let mut changed = target.clone();
        changed.target = native;
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "target substitution {native:?}"
        );
    }
}

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
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &source, native, &[AdmittedBoundarySettlement { boundary,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }]).unwrap();
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
            let TargetOperation::UnitBody(body) = &mut changed.functions[0].operation else {
                panic!("Unit body");
            };
            let TargetUnitOperation::BoundarySettlement {
                execution,
                realization,
                runtime_scalar_arguments,
                boundary,
                ..
            } = &mut body.operations[0]
            else {
                panic!("byte output");
            };
            match mutation {
                0 => {
                    *execution = BoundaryExecutionBinding::CompilerBuiltin(
                        CompilerBuiltinExecution::LinuxExitGroupI32,
                    )
                }
                1 => *realization = BoundaryRealization::LinuxExitGroupI32(Default::default()),
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
            abstract_operations_to_target_operations::lower_to_target_operations(&pure, native)
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
        assert_eq!(
            error,
            crate::LegalizationError::UnsupportedSourceShape { function: 0 }
        );
    }
}
