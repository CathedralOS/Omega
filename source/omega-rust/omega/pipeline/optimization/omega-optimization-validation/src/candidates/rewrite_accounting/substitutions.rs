use super::*;

pub(crate) fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
    let replace = |value: &mut ValueId| {
        if *value == from {
            *value = to;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings {
            replace(&mut binding.argument);
        }
    };
    match operation {
        O::WriteOnlyPrimitiveStore { value, .. } => replace(&mut value.value),
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump { bindings, .. } => rewrite_bindings(bindings),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            rewrite_bindings(&mut when_true.bindings);
            rewrite_bindings(&mut when_false.bindings);
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}

pub(crate) fn rewrite_successor_operation(
    operation: &mut O,
    edge: EdgeId,
    target: BlockId,
    bindings: &[omega_abstract_operations::ValueBinding],
) -> bool {
    match operation {
        O::Jump {
            psi_edge,
            target: operation_target,
            bindings: operation_bindings,
            ..
        } if *psi_edge == edge => {
            *operation_target = target;
            *operation_bindings = bindings.to_vec();
            true
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let successor = if when_true.psi_edge == edge {
                when_true
            } else if when_false.psi_edge == edge {
                when_false
            } else {
                return false;
            };
            successor.target = target;
            successor.bindings = bindings.to_vec();
            true
        }
        _ => false,
    }
}
