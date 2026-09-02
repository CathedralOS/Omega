//! Exhaustive scalar-use and incoming-binding rewrite mechanics.

use super::*;

pub(crate) fn rewrite_block_parameter_operation(
    operation: &mut omega_abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use omega_abstract_operations::AbstractOperation as O;

    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings.iter_mut() {
            if binding.argument == patch.parameter {
                binding.argument = patch.replacement;
            }
        }
    };
    match operation {
        O::WriteOnlyPrimitiveStore { value, .. } | O::StructuralScalarFieldStore { value, .. } => {
            replace(&mut value.value)
        }
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
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => {
            replace(left);
            replace(right);
            replace(addend);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => {
            rewrite_bindings(bindings);
            if *target == patch.block {
                bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
            }
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            for successor in [when_true, when_false] {
                rewrite_bindings(&mut successor.bindings);
                if successor.target == patch.block {
                    successor
                        .bindings
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}
