use super::{CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation};
use typed_trees::TypedTrees;

/// One compiler-owned primitive floating-point binary execution.
///
/// This is deliberately narrower than an authored boundary-operator identity:
/// the selected provider row already retains the exact requirement and
/// realization declarations. This atom identifies only the sealed compiler
/// execution child and therefore commits the irreducible operation and
/// permanent floating-point format.
const fn operation_returns_boolean(operation: CompilerPrimitiveFloatBinaryOperation) -> bool {
    matches!(
        operation,
        CompilerPrimitiveFloatBinaryOperation::Equal
            | CompilerPrimitiveFloatBinaryOperation::NotEqual
            | CompilerPrimitiveFloatBinaryOperation::Less
            | CompilerPrimitiveFloatBinaryOperation::LessOrEqual
            | CompilerPrimitiveFloatBinaryOperation::Greater
            | CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual
    )
}

const fn operation_spelling(
    operation: CompilerPrimitiveFloatBinaryOperation,
) -> language_core::operator_spelling::OperatorSpelling {
    use language_core::operator_spelling::OperatorSpelling;

    match operation {
        CompilerPrimitiveFloatBinaryOperation::Add => OperatorSpelling::Add,
        CompilerPrimitiveFloatBinaryOperation::Subtract => OperatorSpelling::Subtract,
        CompilerPrimitiveFloatBinaryOperation::Multiply => OperatorSpelling::Multiply,
        CompilerPrimitiveFloatBinaryOperation::Divide => OperatorSpelling::Divide,
        CompilerPrimitiveFloatBinaryOperation::Equal => OperatorSpelling::Equal,
        CompilerPrimitiveFloatBinaryOperation::NotEqual => OperatorSpelling::NotEqual,
        CompilerPrimitiveFloatBinaryOperation::Less => OperatorSpelling::Less,
        CompilerPrimitiveFloatBinaryOperation::LessOrEqual => OperatorSpelling::LessEqual,
        CompilerPrimitiveFloatBinaryOperation::Greater => OperatorSpelling::Greater,
        CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual => OperatorSpelling::GreaterEqual,
    }
}

/// Classify the primitive floating-point execution child from the exact typed
/// boundary operator. The authored realization-machine name is not consulted.
pub fn primitive_float_binary_intrinsic_execution_identity(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<CompilerIntrinsicExecutionIdentity> {
    let [namespace, requirement] = typed.operator_path_members(operator.name) else {
        return None;
    };
    if namespace.as_str() != "Float" {
        return None;
    }
    let operation = match requirement.as_str() {
        "add" => CompilerPrimitiveFloatBinaryOperation::Add,
        "subtract" => CompilerPrimitiveFloatBinaryOperation::Subtract,
        "multiply" => CompilerPrimitiveFloatBinaryOperation::Multiply,
        "divide" => CompilerPrimitiveFloatBinaryOperation::Divide,
        "equal" => CompilerPrimitiveFloatBinaryOperation::Equal,
        "not_equal" => CompilerPrimitiveFloatBinaryOperation::NotEqual,
        "less" => CompilerPrimitiveFloatBinaryOperation::Less,
        "less_or_equal" => CompilerPrimitiveFloatBinaryOperation::LessOrEqual,
        "greater" => CompilerPrimitiveFloatBinaryOperation::Greater,
        "greater_or_equal" => CompilerPrimitiveFloatBinaryOperation::GreaterOrEqual,
        _ => return None,
    };
    if operator.spelling != Some(operation_spelling(operation)) {
        return None;
    }
    let [left, right] = typed.operator_parameters(operator) else {
        return None;
    };
    let primitive = typed.primitive_type_reference(left.type_reference)?;
    if typed.primitive_type_reference(right.type_reference) != Some(primitive) {
        return None;
    }
    let format = match primitive {
        typed_trees::types::PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
        typed_trees::types::PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
        _ => return None,
    };
    let expected_result = if operation_returns_boolean(operation) {
        typed_trees::types::PrimitiveType::Bool
    } else {
        primitive
    };
    if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
        return None;
    }
    Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format })
}
