use crate::InstructionSelectionInput;
use omega_abstract_operations::StateGuardOperator;
use psi_checked_trees::expression::{BinaryOperator, TableCallExpression};
use psi_symbols::BuiltinFunction;

pub(super) fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 2 | 4 | 8)
}

pub(super) fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 2 | 4 | 8)
}

pub(super) fn is_float_classification_predicate(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::IsNan
            | StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
            | StateGuardOperator::FloatClassify
    )
}

pub(in crate::selection) fn float_unary_result_is_bool(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::IsNan
            | StateGuardOperator::IsFinite
            | StateGuardOperator::IsInfinite
            | StateGuardOperator::IsNormal
            | StateGuardOperator::IsSubnormal
    )
}

pub(super) fn runtime_binary_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::And => Some(StateGuardOperator::And),
        BinaryOperator::Equal => Some(StateGuardOperator::Equal),
        BinaryOperator::Greater => Some(StateGuardOperator::Greater),
        BinaryOperator::GreaterOrEqual => Some(StateGuardOperator::GreaterOrEqual),
        BinaryOperator::Less => Some(StateGuardOperator::Less),
        BinaryOperator::LessOrEqual => Some(StateGuardOperator::LessOrEqual),
        BinaryOperator::NotEqual => Some(StateGuardOperator::NotEqual),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Divide => Some(StateGuardOperator::Divide),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Or => Some(StateGuardOperator::Or),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::ShiftLeft => Some(StateGuardOperator::ShiftLeft),
        BinaryOperator::ShiftRight => Some(StateGuardOperator::ShiftRight),
        BinaryOperator::BitwiseAnd => Some(StateGuardOperator::BitwiseAnd),
        BinaryOperator::BitwiseOr => Some(StateGuardOperator::BitwiseOr),
        BinaryOperator::BitwiseXor => Some(StateGuardOperator::BitwiseXor),
    }
}

pub(in crate::selection) fn builtin_runtime_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_valid() || call.arguments.count() != 2 {
        return None;
    }

    builtin_runtime_call_operator_by_symbol(input, call.target_symbol)
}

pub(in crate::selection) fn builtin_runtime_binary_float_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_valid() || call.arguments.count() != 2 {
        return None;
    }
    builtin_runtime_binary_float_operator_by_symbol(input, call.target_symbol)
}

pub(super) fn builtin_runtime_call_operator(
    input: &InstructionSelectionInput<'_>,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_some() || call.arguments.len() != 2 {
        return None;
    }

    builtin_runtime_call_operator_by_symbol(input, call.target_symbol)
}

/// A single-argument float builtin that lowers on the binary value-write path.
/// `sqrt(x)` carries both expression positions as `x`; float classification
/// predicates later replace the ignored right runtime operand with zero so
/// `x` executes once.
pub(in crate::selection) fn builtin_runtime_unary_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_valid() || call.arguments.count() != 1 {
        return None;
    }
    if Some(call.target_symbol)
        == input
            .program
            .symbols
            .builtin_function_symbol(BuiltinFunction::Sqrt)
    {
        return Some(StateGuardOperator::Sqrt);
    }
    for (builtin, operator) in [
        (
            BuiltinFunction::FloatSqrtTowardZeroF32,
            StateGuardOperator::SqrtTowardZero,
        ),
        (
            BuiltinFunction::FloatSqrtTowardZeroF64,
            StateGuardOperator::SqrtTowardZero,
        ),
        (
            BuiltinFunction::FloatSqrtTowardPositiveF32,
            StateGuardOperator::SqrtTowardPositive,
        ),
        (
            BuiltinFunction::FloatSqrtTowardPositiveF64,
            StateGuardOperator::SqrtTowardPositive,
        ),
        (
            BuiltinFunction::FloatSqrtTowardNegativeF32,
            StateGuardOperator::SqrtTowardNegative,
        ),
        (
            BuiltinFunction::FloatSqrtTowardNegativeF64,
            StateGuardOperator::SqrtTowardNegative,
        ),
        (BuiltinFunction::FloatIsNan, StateGuardOperator::IsNan),
        (BuiltinFunction::FloatIsFinite, StateGuardOperator::IsFinite),
        (
            BuiltinFunction::FloatIsInfinite,
            StateGuardOperator::IsInfinite,
        ),
        (BuiltinFunction::FloatIsNormal, StateGuardOperator::IsNormal),
        (
            BuiltinFunction::FloatIsSubnormal,
            StateGuardOperator::IsSubnormal,
        ),
        (
            BuiltinFunction::FloatClassifyF32,
            StateGuardOperator::FloatClassify,
        ),
        (
            BuiltinFunction::FloatClassifyF64,
            StateGuardOperator::FloatClassify,
        ),
    ] {
        if Some(call.target_symbol) == input.program.symbols.builtin_function_symbol(builtin) {
            return Some(operator);
        }
    }
    None
}

pub(in crate::selection) fn builtin_runtime_ternary_float_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<(usize, StateGuardOperator)> {
    if call.receiver.is_valid() || call.arguments.count() != 3 {
        return None;
    }
    let symbols = &input.program.symbols;
    let builtin = [
        BuiltinFunction::FloatMultiplyThenAddF32,
        BuiltinFunction::FloatMultiplyThenAddF64,
        BuiltinFunction::FloatFusedMultiplyAddF32,
        BuiltinFunction::FloatFusedMultiplyAddF64,
        BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64,
        BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64,
        BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32,
        BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64,
    ]
    .into_iter()
    .find(|builtin| Some(call.target_symbol) == symbols.builtin_function_symbol(*builtin))?;
    builtin_ternary_float_operator(builtin)
}

fn builtin_ternary_float_operator(builtin: BuiltinFunction) -> Option<(usize, StateGuardOperator)> {
    for (candidate, byte_width, operator) in [
        (
            BuiltinFunction::FloatMultiplyThenAddF32,
            4,
            StateGuardOperator::MultiplyThenAdd,
        ),
        (
            BuiltinFunction::FloatMultiplyThenAddF64,
            8,
            StateGuardOperator::MultiplyThenAdd,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddF32,
            4,
            StateGuardOperator::FusedMultiplyAdd,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddF64,
            8,
            StateGuardOperator::FusedMultiplyAdd,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardZeroF32,
            4,
            StateGuardOperator::FusedMultiplyAddTowardZero,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardZeroF64,
            8,
            StateGuardOperator::FusedMultiplyAddTowardZero,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF32,
            4,
            StateGuardOperator::FusedMultiplyAddTowardPositive,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardPositiveF64,
            8,
            StateGuardOperator::FusedMultiplyAddTowardPositive,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF32,
            4,
            StateGuardOperator::FusedMultiplyAddTowardNegative,
        ),
        (
            BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64,
            8,
            StateGuardOperator::FusedMultiplyAddTowardNegative,
        ),
    ] {
        if builtin == candidate {
            return Some((byte_width, operator));
        }
    }
    None
}

fn builtin_runtime_call_operator_by_symbol(
    input: &InstructionSelectionInput<'_>,
    target_symbol: psi_symbols::SymbolHandle,
) -> Option<StateGuardOperator> {
    let symbols = &input.program.symbols;
    if Some(target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Max) {
        return Some(StateGuardOperator::Max);
    }
    if Some(target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Min) {
        return Some(StateGuardOperator::Min);
    }
    builtin_runtime_binary_float_operator_by_symbol(input, target_symbol)
}

fn builtin_runtime_binary_float_operator_by_symbol(
    input: &InstructionSelectionInput<'_>,
    target_symbol: psi_symbols::SymbolHandle,
) -> Option<StateGuardOperator> {
    let symbols = &input.program.symbols;
    for (builtin, operator) in [
        (
            BuiltinFunction::FloatAddTowardZeroF32,
            StateGuardOperator::AddTowardZero,
        ),
        (
            BuiltinFunction::FloatAddTowardZeroF64,
            StateGuardOperator::AddTowardZero,
        ),
        (
            BuiltinFunction::FloatAddTowardPositiveF32,
            StateGuardOperator::AddTowardPositive,
        ),
        (
            BuiltinFunction::FloatAddTowardPositiveF64,
            StateGuardOperator::AddTowardPositive,
        ),
        (
            BuiltinFunction::FloatAddTowardNegativeF32,
            StateGuardOperator::AddTowardNegative,
        ),
        (
            BuiltinFunction::FloatAddTowardNegativeF64,
            StateGuardOperator::AddTowardNegative,
        ),
        (
            BuiltinFunction::FloatSubtractTowardZeroF32,
            StateGuardOperator::SubtractTowardZero,
        ),
        (
            BuiltinFunction::FloatSubtractTowardZeroF64,
            StateGuardOperator::SubtractTowardZero,
        ),
        (
            BuiltinFunction::FloatSubtractTowardPositiveF32,
            StateGuardOperator::SubtractTowardPositive,
        ),
        (
            BuiltinFunction::FloatSubtractTowardPositiveF64,
            StateGuardOperator::SubtractTowardPositive,
        ),
        (
            BuiltinFunction::FloatSubtractTowardNegativeF32,
            StateGuardOperator::SubtractTowardNegative,
        ),
        (
            BuiltinFunction::FloatSubtractTowardNegativeF64,
            StateGuardOperator::SubtractTowardNegative,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardZeroF32,
            StateGuardOperator::MultiplyTowardZero,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardZeroF64,
            StateGuardOperator::MultiplyTowardZero,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardPositiveF32,
            StateGuardOperator::MultiplyTowardPositive,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardPositiveF64,
            StateGuardOperator::MultiplyTowardPositive,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardNegativeF32,
            StateGuardOperator::MultiplyTowardNegative,
        ),
        (
            BuiltinFunction::FloatMultiplyTowardNegativeF64,
            StateGuardOperator::MultiplyTowardNegative,
        ),
        (
            BuiltinFunction::FloatDivideTowardZeroF32,
            StateGuardOperator::DivideTowardZero,
        ),
        (
            BuiltinFunction::FloatDivideTowardZeroF64,
            StateGuardOperator::DivideTowardZero,
        ),
        (
            BuiltinFunction::FloatDivideTowardPositiveF32,
            StateGuardOperator::DivideTowardPositive,
        ),
        (
            BuiltinFunction::FloatDivideTowardPositiveF64,
            StateGuardOperator::DivideTowardPositive,
        ),
        (
            BuiltinFunction::FloatDivideTowardNegativeF32,
            StateGuardOperator::DivideTowardNegative,
        ),
        (
            BuiltinFunction::FloatDivideTowardNegativeF64,
            StateGuardOperator::DivideTowardNegative,
        ),
    ] {
        if Some(target_symbol) == symbols.builtin_function_symbol(builtin) {
            return Some(operator);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use omega_abstract_operations::StateGuardOperator;

    use super::{builtin_ternary_float_operator, float_unary_result_is_bool};

    #[test]
    fn boolean_float_predicates_exclude_enum_classification() {
        for operator in [
            StateGuardOperator::IsNan,
            StateGuardOperator::IsFinite,
            StateGuardOperator::IsInfinite,
            StateGuardOperator::IsNormal,
            StateGuardOperator::IsSubnormal,
        ] {
            assert!(float_unary_result_is_bool(operator), "{operator:?}");
        }
        assert!(!float_unary_result_is_bool(
            StateGuardOperator::FloatClassify
        ));
        assert!(!float_unary_result_is_bool(StateGuardOperator::Sqrt));
    }

    #[test]
    fn ternary_float_builtins_retain_format_and_operation() {
        assert_eq!(
            builtin_ternary_float_operator(psi_symbols::BuiltinFunction::FloatMultiplyThenAddF32,),
            Some((4, StateGuardOperator::MultiplyThenAdd))
        );
        assert_eq!(
            builtin_ternary_float_operator(
                psi_symbols::BuiltinFunction::FloatFusedMultiplyAddTowardNegativeF64,
            ),
            Some((8, StateGuardOperator::FusedMultiplyAddTowardNegative))
        );
        assert_eq!(
            builtin_ternary_float_operator(psi_symbols::BuiltinFunction::FloatIsFinite),
            None
        );
    }
}
