use super::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
use typed_trees::types::PrimitiveType;
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

/// The exact integer comparison a boundary operator declaration spells.
/// Unlike the float catalog there is no canonical requirement namespace for
/// integer comparisons: the authored token is the operation identity, so the
/// classification follows the declared spelling, the shared integer operand
/// primitive, and the Boolean result -- the same admission the checked
/// `selected_integer_comparison` classifier applies to the use. An operand
/// primitive outside the fixed-width integer roster (Boolean, a float
/// format, or the address carrier) has no closed comparison identity.
fn primitive_integer_comparison_semantics(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<(CompilerPrimitiveIntegerComparisonOperation, PrimitiveType)> {
    use CompilerPrimitiveIntegerComparisonOperation as Operation;
    use language_core::operator_spelling::OperatorSpelling;
    let operation = match operator.spelling {
        Some(OperatorSpelling::Equal) => Operation::Equal,
        Some(OperatorSpelling::NotEqual) => Operation::NotEqual,
        Some(OperatorSpelling::Less) => Operation::Less,
        Some(OperatorSpelling::LessEqual) => Operation::LessOrEqual,
        Some(OperatorSpelling::Greater) => Operation::Greater,
        Some(OperatorSpelling::GreaterEqual) => Operation::GreaterOrEqual,
        _ => return None,
    };
    let [left, right] = typed.operator_parameters(operator) else {
        return None;
    };
    let primitive = typed.primitive_type_reference(left.type_reference)?;
    if typed.primitive_type_reference(right.type_reference) != Some(primitive)
        || matches!(
            primitive,
            PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
        )
        || typed.primitive_type_reference(operator.return_type) != Some(PrimitiveType::Bool)
    {
        return None;
    }
    Some((operation, primitive))
}

/// Encode the selected integer comparison as the sealed compiler execution
/// child. The selected plan separately binds its provider; this child commits
/// to the authored comparison meaning and exact operand type.
pub fn primitive_integer_comparison_intrinsic_execution_identity(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let requirement = crate::IntrinsicRequirement::from_operator(typed, operator)?;
    primitive_integer_comparison_intrinsic_execution_identity_for(typed, &requirement)
}

/// The spelled integer-comparison execution child. Only the operator species
/// carries a fixed token, so a top-level requirement never selects it. The
/// address carrier has no `CompilerNumericType`, so an `addr` operand fails
/// closed rather than widening into the integer cohort.
pub fn primitive_integer_comparison_intrinsic_execution_identity_for(
    typed: &TypedTrees,
    requirement: &crate::IntrinsicRequirement<'_>,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let operator = requirement.as_operator()?;
    let (operation, primitive) = primitive_integer_comparison_semantics(typed, operator)?;
    let integer_type = CompilerNumericType::from_primitive(primitive)?;
    if integer_type.is_float() {
        return None;
    }
    Some(
        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
            operation,
            integer_type,
        },
    )
}
