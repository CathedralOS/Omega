use super::*;

pub(super) fn operation_definition(operation: &AbstractOperation) -> Option<(ValueId, ScalarType)> {
    use AbstractOperation as O;
    match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::IeeeFloatConstant { result, value, .. } => {
            Some((*result, ScalarType::IeeeFloat(value.format())))
        }
        O::NearestIeeeFloatFusedMultiplyAdd { result, format, .. } => {
            Some((*result, ScalarType::IeeeFloat(*format)))
        }
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    }
}

pub(super) fn operation_uses(operation: &AbstractOperation) -> Vec<ValueId> {
    use AbstractOperation as O;
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::WriteOnlyPrimitiveStore { value, .. } => vec![value.value],
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
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
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => vec![*left, *right, *addend],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    }
}
