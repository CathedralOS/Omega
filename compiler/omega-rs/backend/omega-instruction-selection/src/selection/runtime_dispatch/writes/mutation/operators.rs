use crate::InstructionSelectionInput;
use omega_abstract_operations::StateGuardOperator;
use omega_core::symbols::BuiltinFunction;
use psi_checked_trees::expression::{BinaryOperator, TableCallExpression};

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

fn builtin_runtime_call_operator_by_symbol(
    input: &InstructionSelectionInput<'_>,
    target_symbol: omega_core::symbols::SymbolHandle,
) -> Option<StateGuardOperator> {
    let symbols = &input.program.symbols;
    if Some(target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Max) {
        return Some(StateGuardOperator::Max);
    }
    if Some(target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Min) {
        return Some(StateGuardOperator::Min);
    }

    None
}
