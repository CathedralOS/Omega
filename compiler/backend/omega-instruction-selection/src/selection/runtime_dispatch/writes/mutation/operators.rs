use crate::InstructionSelectionInput;
use omega_abstract_operations::StateGuardOperator;
use omega_checked_trees::expression::{BinaryOperator, TableCallExpression};
use omega_core::symbols::BuiltinFunction;

pub(super) fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

pub(super) fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
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
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => None,
    }
}

pub(super) fn builtin_runtime_call_operator_in_table(
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
    call: &omega_checked_trees::expression::CallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_some() || call.arguments.len() != 2 {
        return None;
    }

    builtin_runtime_call_operator_by_symbol(input, call.target_symbol)
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
