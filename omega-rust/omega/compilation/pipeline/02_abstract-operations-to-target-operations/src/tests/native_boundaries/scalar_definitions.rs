//! Ordered Unit definitions retain exact producer homes, not assigned stack slots.
use super::super::{
    AbstractOperation, AbstractOperationPlan, IntegerSign, IntegerType, NativeTarget, OperationId,
    ScalarType, ValueId,
};

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
    crate::lower_to_target_operations(
        plan,
        crate::TargetLoweringRequest {
            target,
            settlements: &[crate::AdmittedBoundarySettlement {
                boundary: plan.boundary_machines[0].id,
                execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                realization: target_operations::HostedWriteByteI32Realization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
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
