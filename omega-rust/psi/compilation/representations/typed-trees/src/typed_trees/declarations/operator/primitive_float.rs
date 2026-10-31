//! Shared classification of the sealed primitive Float semantic child.
//! This derives meaning from the exact typed requirement; provider selection
//! and realization authority remain separate obligations owned by Omega.
use crate::{TypedTrees, expression::BinaryOperator};

const fn operation_returns_boolean(operation: BinaryOperator) -> bool {
    matches!(
        operation,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

const fn operation_spelling(
    operation: BinaryOperator,
) -> Option<language_core::operator_spelling::OperatorSpelling> {
    use language_core::operator_spelling::OperatorSpelling;

    Some(match operation {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        _ => return super::spellings::comparison_operator_spelling(operation),
    })
}

pub fn primitive_float_binary_semantics(
    typed: &TypedTrees,
    operator: &super::OperatorDefinition,
) -> Option<(BinaryOperator, numerics::literals::FloatFormat)> {
    let [namespace, requirement] = typed.operator_path_members(operator.name) else {
        return None;
    };
    if namespace.as_str() != "Float" {
        return None;
    }
    let operation = match requirement.as_str() {
        "add" => BinaryOperator::Add,
        "subtract" => BinaryOperator::Subtract,
        "multiply" => BinaryOperator::Multiply,
        "divide" => BinaryOperator::Divide,
        "equal" => BinaryOperator::Equal,
        "not_equal" => BinaryOperator::NotEqual,
        "less" => BinaryOperator::Less,
        "less_or_equal" => BinaryOperator::LessOrEqual,
        "greater" => BinaryOperator::Greater,
        "greater_or_equal" => BinaryOperator::GreaterOrEqual,
        _ => return None,
    };
    if operator.spelling != operation_spelling(operation) {
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
        crate::types::PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
        crate::types::PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
        _ => return None,
    };
    let expected_result = if operation_returns_boolean(operation) {
        crate::types::PrimitiveType::Bool
    } else {
        primitive
    };
    if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
        return None;
    }
    Some((operation, format))
}

/// Exact declaration custody shared by primitive selection and semantic proof.
/// A matching Float name/signature in a user source supplies no sealed law.
pub fn is_sealed_toolchain_float_operator(
    typed: &TypedTrees,
    operator: &super::OperatorDefinition,
) -> bool {
    let [namespace, name] = typed.operator_path_members(operator.name) else {
        return false;
    };
    typed
        .normalized_hermetic_symbol_identity(operator.symbol)
        .is_ok_and(|identity| {
            identity == format!("toolchain::{}::{}", namespace.as_str(), name.as_str())
        })
        && typed
            .symbol_toolchain_relative_path(operator.symbol)
            .is_some_and(|relative| {
                relative
                    == std::path::Path::new(
                        numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                    )
            })
}

/// Primitive token requirements are the language's default numeric surface.
/// Only this sealed root cohort is available without an authored import;
/// ordinary helpers and domain-owned meanings retain source exposure rules.
/// Selection still uses the same complete operand tuple and ambiguity checks,
/// and grants no target-provider realization authority.
pub(super) fn is_default_primitive_operator(
    typed: &TypedTrees,
    operator: &super::OperatorDefinition,
) -> bool {
    operator.is_boundary
        && operator.is_public
        && !operator.home_domain.is_valid()
        && is_sealed_toolchain_float_operator(typed, operator)
        && primitive_float_binary_semantics(typed, operator).is_some()
}
