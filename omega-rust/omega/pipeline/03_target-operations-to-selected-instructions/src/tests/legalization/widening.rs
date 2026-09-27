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

use crate::legalize_target_operations;

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
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
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
