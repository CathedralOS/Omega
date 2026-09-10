use super::{CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation};
use typed_trees::{TypedTrees, expression::BinaryOperator};

/// Encode the shared target-neutral Float meaning as the existing sealed
/// compiler execution child. The selected plan separately binds its provider.
pub fn primitive_float_binary_intrinsic_execution_identity(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let (operation, format) =
        typed_trees::operator::primitive_float_binary_semantics(typed, operator)?;
    let operation = match operation {
        BinaryOperator::Add => CompilerPrimitiveFloatBinaryOperation::Add,
        BinaryOperator::Subtract => CompilerPrimitiveFloatBinaryOperation::Subtract,
        BinaryOperator::Multiply => CompilerPrimitiveFloatBinaryOperation::Multiply,
        BinaryOperator::Divide => CompilerPrimitiveFloatBinaryOperation::Divide,
        BinaryOperator::Equal => CompilerPrimitiveFloatBinaryOperation::Equal,
        BinaryOperator::NotEqual => CompilerPrimitiveFloatBinaryOperation::NotEqual,
        BinaryOperator::Less => CompilerPrimitiveFloatBinaryOperation::Less,
        BinaryOperator::LessOrEqual => CompilerPrimitiveFloatBinaryOperation::LessOrEqual,
        BinaryOperator::Greater => CompilerPrimitiveFloatBinaryOperation::Greater,
        BinaryOperator::GreaterOrEqual => CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual,
        _ => return None,
    };
    Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format })
}
