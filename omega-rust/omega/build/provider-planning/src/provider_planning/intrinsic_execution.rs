use super::{CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation};
use typed_trees::{TypedTrees, expression::BinaryOperator};

/// Encode the shared target-neutral Float meaning as the existing sealed
/// compiler execution child. The selected plan separately binds its provider.
pub fn primitive_float_binary_intrinsic_execution_identity(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let requirement = crate::IntrinsicRequirement::from_operator(typed, operator)?;
    primitive_float_binary_intrinsic_execution_identity_for(typed, &requirement)
}

/// The spelled `Float::+`-family execution child. Only the operator species
/// carries a fixed token, so a top-level requirement never selects it.
pub fn primitive_float_binary_intrinsic_execution_identity_for(
    typed: &TypedTrees,
    requirement: &crate::IntrinsicRequirement<'_>,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let operator = requirement.as_operator()?;
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
