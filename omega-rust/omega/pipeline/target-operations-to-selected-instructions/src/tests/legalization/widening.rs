//! Ordered Unit scalar definitions retain their exact widening source and result.
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, ScalarType,
    ValueId,
};
use target::NativeTarget;
use target_operations::{
    CompilerBuiltinExecution, TargetIntegerExpression, TargetOperationPlan, TargetScalarExpression,
    TargetUnitOperation,
};

use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};

fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
}

fn fixture(
    native: NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = super::byte_output::fixture(native);
    let input_type = integer(IntegerSign::Unsigned, 8);
    let output_type = integer(IntegerSign::Signed, 32);
    source.functions[0].parameters[0].scalar_type = ScalarType::Integer(input_type);
    let AbstractOperation::BoundaryCall { arguments, .. } = &mut source.functions[0].operations[0]
    else {
        panic!("byte-output source");
    };
    arguments[0] = ValueId::new(9).unwrap();
    source.functions[0].operations.insert(
        0,
        AbstractOperation::IntegerWiden {
            psi_operation: OperationId::new(8).unwrap(),
            result: ValueId::new(9).unwrap(),
            source_type: input_type,
            target_type: output_type,
            operand: ValueId::new(5).unwrap(),
        },
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &source, native, &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::HostedWriteByteI32),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    ).unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn unit_u8_to_i32_widening_selects_one_definition_before_byte_output() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let rows = &legalized.plan().scalar_functions[0].blocks[0].instructions;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].operation, OperationId::new(8).unwrap());
        assert_eq!(
            rows[0].result.as_ref().unwrap().value,
            ValueId::new(9).unwrap()
        );
        assert!(
            matches!(rows[0].kind, legalized_operations::LegalizedScalarInstructionKind::IntegerWiden { operand, source_type }
            if operand == ValueId::new(5).unwrap() && source_type == integer(IntegerSign::Unsigned, 8))
        );
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let instructions = &selected.plan().functions[0].blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .filter(
                    |row| row.kind == selected_instructions::SelectedInstructionKind::ZeroExtendU8
                )
                .count(),
            1
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|row| matches!(
                    row.kind,
                    selected_instructions::SelectedInstructionKind::HostedWriteByteI32 { .. }
                ))
                .count(),
            1
        );
        let mut changed = selected.plan().clone();
        let widening = changed.functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|row| row.kind == selected_instructions::SelectedInstructionKind::ZeroExtendU8)
            .unwrap();
        widening.kind = selected_instructions::SelectedInstructionKind::ZeroExtendU32;
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                changed
            )
            .is_err()
        );
    }
}

#[test]
fn target_widening_rejects_home_type_identity_and_definition_order_substitution() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        for mutation in 0..10 {
            let mut changed = target.clone();
            let body = &mut changed.functions[0].graph;
            if mutation == 8 {
                body.blocks[0].operations.remove(0);
            } else if mutation == 9 {
                body.blocks[0].operations.swap(0, 1);
            } else {
                let TargetUnitOperation::ScalarDefinition {
                    result_home,
                    expression,
                } = &mut body.blocks[0].operations[0]
                else {
                    panic!("scalar definition");
                };
                let TargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                } = expression
                else {
                    panic!("integer definition");
                };
                let TargetIntegerExpression::IntegerWiden {
                    source_type,
                    operand,
                    ..
                } = expression
                else {
                    panic!("widening");
                };
                match mutation {
                    0 => result_home.defining_operation = OperationId::new(99).unwrap(),
                    1 => result_home.source_value = ValueId::new(99).unwrap(),
                    2 => result_home.shape = calling_conventions::ValueShape::integer(8, 8),
                    3 => {
                        result_home.scalar_type =
                            ScalarType::Integer(integer(IntegerSign::Unsigned, 32))
                    }
                    4 => *scalar_type = integer(IntegerSign::Signed, 8),
                    5 => *source_type = integer(IntegerSign::Signed, 8),
                    6 => *source_type = integer(IntegerSign::Unsigned, 64),
                    _ => **operand = TargetIntegerExpression::ScalarHome(*result_home),
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}"
            );
        }
    }
}

#[test]
fn widening_replay_rejects_signed_narrowing_source_and_future_value_forgery() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..7 {
            let mut changed = legalized.plan().clone();
            let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
            if mutation == 5 {
                rows.remove(0);
            } else if mutation == 6 {
                rows.swap(0, 1);
            } else {
                let row = &mut rows[0];
                let legalized_operations::LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } = &mut row.kind
                else {
                    panic!("widening");
                };
                match mutation {
                    0 => *source_type = integer(IntegerSign::Signed, 8),
                    1 => *source_type = integer(IntegerSign::Unsigned, 64),
                    2 => {
                        row.result.as_mut().unwrap().scalar_type =
                            ScalarType::Integer(integer(IntegerSign::Signed, 8))
                    }
                    3 => *operand = ValueId::new(9).unwrap(),
                    _ => row.result.as_mut().unwrap().value = ValueId::new(99).unwrap(),
                }
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "replay mutation {mutation}"
            );
        }
    }
}
