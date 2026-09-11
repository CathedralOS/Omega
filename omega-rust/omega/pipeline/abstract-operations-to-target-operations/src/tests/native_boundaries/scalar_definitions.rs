//! Ordered Unit definitions retain exact producer homes, not assigned stack slots.
use super::*;
use target_operations::TargetOperationPlan;

fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
}

fn fixture() -> AbstractOperationPlan {
    let mut plan = super::returning_byte_parameter::fixture();
    let source_type = integer(IntegerSign::Unsigned, 8);
    let target_type = integer(IntegerSign::Signed, 32);
    let function = &mut plan.functions[0];
    function.parameters[0].scalar_type = ScalarType::Integer(source_type);
    let operand = function.parameters[0].value;
    let result = ValueId::new(910).unwrap();
    let AbstractOperation::BoundaryCall { arguments, .. } = &mut function.operations[0] else {
        panic!("boundary");
    };
    arguments[0] = result;
    function.operations.insert(
        0,
        AbstractOperation::IntegerWiden {
            psi_operation: OperationId::new(910).unwrap(),
            result,
            source_type,
            target_type,
            operand,
        },
    );
    plan
}

fn lower(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, crate::LoweringError> {
    crate::lower_to_target_operations_with_provider_executions(
        plan,
        target,
        &[crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[0].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    )
}

#[test]
fn unit_widening_has_exact_definition_and_boundary_source() {
    let plan = fixture();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower(&plan, target).unwrap();
        let body = &lowered.functions[0].graph;
        let TargetUnitOperation::ScalarDefinition {
            result_home,
            expression,
        } = &body.blocks[0].operations[0]
        else {
            panic!("definition");
        };
        assert_eq!(result_home.source_value, ValueId::new(910).unwrap());
        assert_eq!(
            result_home.defining_operation,
            OperationId::new(910).unwrap()
        );
        assert_eq!(result_home.shape, ValueShape::integer(4, 4));
        assert_eq!(expression.scalar_type(), result_home.scalar_type);
        let TargetUnitOperation::BoundarySettlement {
            runtime_scalar_arguments,
            ..
        } = &body.blocks[0].operations[1]
        else {
            panic!("boundary");
        };
        assert_eq!(
            runtime_scalar_arguments[0].source,
            target_operations::TargetUnitScalarArgumentSource::Home(*result_home)
        );
    }
}

#[test]
fn unit_widening_rejects_wrong_source_type_narrowing_and_unknown_values() {
    for mutation in 0..4 {
        let mut plan = fixture();
        let AbstractOperation::IntegerWiden {
            operand,
            source_type,
            target_type,
            ..
        } = &mut plan.functions[0].operations[0]
        else {
            panic!("widen");
        };
        match mutation {
            0 => *source_type = integer(IntegerSign::Signed, 8),
            1 => *target_type = integer(IntegerSign::Signed, 8),
            2 => *operand = ValueId::new(999).unwrap(),
            _ => plan.functions[0].operations.swap(0, 1),
        }
        assert!(
            lower(&plan, NativeTarget::linux_x64()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn successive_unit_definitions_reference_the_prior_home_once() {
    let mut plan = fixture();
    let function = &mut plan.functions[0];
    let AbstractOperation::IntegerWiden { target_type, .. } = &mut function.operations[0] else {
        panic!("widen");
    };
    *target_type = integer(IntegerSign::Unsigned, 16);
    function.operations.insert(
        1,
        AbstractOperation::IntegerWiden {
            psi_operation: OperationId::new(911).unwrap(),
            result: ValueId::new(911).unwrap(),
            source_type: integer(IntegerSign::Unsigned, 16),
            target_type: integer(IntegerSign::Signed, 32),
            operand: ValueId::new(910).unwrap(),
        },
    );
    let AbstractOperation::BoundaryCall { arguments, .. } = &mut function.operations[2] else {
        panic!("boundary");
    };
    arguments[0] = ValueId::new(911).unwrap();
    let lowered = lower(&plan, NativeTarget::linux_x64()).unwrap();
    let body = &lowered.functions[0].graph;
    let TargetUnitOperation::ScalarDefinition { result_home, .. } = &body.blocks[0].operations[0]
    else {
        panic!("first");
    };
    let TargetUnitOperation::ScalarDefinition {
        expression:
            target_operations::TargetScalarExpression::Integer {
                expression: target_operations::TargetIntegerExpression::IntegerWiden { operand, .. },
                ..
            },
        ..
    } = &body.blocks[0].operations[1]
    else {
        panic!("second");
    };
    assert_eq!(
        operand.as_ref(),
        &target_operations::TargetIntegerExpression::ScalarHome(*result_home)
    );
}

#[test]
fn unit_widening_retains_all_total_fixed_native_integer_shapes() {
    for source_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for target_sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for source_bits in [8, 16, 32] {
                for target_bits in [16, 32, 64] {
                    let source = integer(source_sign, source_bits);
                    let target = integer(target_sign, target_bits);
                    if !source.can_widen_to(target) {
                        continue;
                    }
                    let mut plan = fixture();
                    plan.functions[0].parameters[0].scalar_type = ScalarType::Integer(source);
                    let AbstractOperation::IntegerWiden {
                        source_type,
                        target_type,
                        ..
                    } = &mut plan.functions[0].operations[0]
                    else {
                        panic!("widen");
                    };
                    *source_type = source;
                    *target_type = target;
                    plan.functions[0].operations.remove(1);
                    crate::lower_to_target_operations(&plan, NativeTarget::linux_x64())
                        .unwrap_or_else(|error| panic!("{source:?} -> {target:?}: {error:?}"));
                }
            }
        }
    }
}
