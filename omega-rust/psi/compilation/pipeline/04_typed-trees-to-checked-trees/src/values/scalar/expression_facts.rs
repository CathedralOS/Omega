//! Facts about scalar expressions the lowerings share: parameter and local
//! positions, expression types, builtin operators and arithmetic domains.

use crate::values::scalar::expression_plans::ScalarLocal;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Whether an authored parameter occupies a position in the dense scalar
/// namespace that `CheckedScalarExpression::Parameter` indexes: a primitive
/// carrier that is not an `[erased]` binding occurrence. An erased primitive
/// parameter owns no position (contracts.md#explicit-erased-bindings), so the
/// retained parameters after it shift down, matching the stripped signature
/// plan in `execution::unit::calls::signatures`; a runtime read of it then
/// finds no position and fails closed here as well as in validation.
pub(crate) fn occupies_scalar_position(
    program: &dyn ::validation::CheckingInput,
    parameter: &StateParameter,
) -> bool {
    !parameter.relevance.is_erased()
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
}

pub(crate) fn parameter_position(
    program: &dyn ::validation::CheckingInput,
    path: &typed_trees::expression::TableNamePath,
    parameters: &[StateParameter],
) -> Option<usize> {
    let members = program.expression_table.name_path_members(path.members);
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (members.len() == 1)
        .then(|| {
            parameters.iter().position(|parameter| {
                if path.symbol.is_valid() {
                    parameter.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    parameter.symbol == path.head_symbol
                } else {
                    parameter.name.as_str() == members[0].as_str()
                }
            })
        })
        .flatten()
}

pub(crate) fn local_position(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
    locals: &[ScalarLocal],
) -> Option<usize> {
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            locals.iter().rposition(|local| {
                if local.is_mutable {
                    // Bare storage reads require both retained resolved identities.
                    // Spelling cannot repair a missing or stale storage handle.
                    local.symbol.is_valid()
                        && local.symbol == path.symbol
                        && local.symbol == path.head_symbol
                } else if path.symbol.is_valid() {
                    local.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    local.symbol == path.head_symbol
                } else {
                    local.name == program.expression_table.display_name(expression)
                }
            })
        })
        .flatten()
}

pub(crate) fn scalar_expression_type(
    expression: &CheckedScalarExpression,
) -> Option<PrimitiveType> {
    expression.primitive_type()
}

/// Plain primitive declarations establish zero without an authored
/// initializer. Keep this tied to the declaration: a missing expression at
/// another site or a reference has no such meaning. Arithmetic policy alone
/// permits zero; other qualifications still need their establishment evidence.
pub(crate) fn zeroed_scalar_local_initializer(
    program: &dyn ::validation::CheckingInput,
    local: &typed_trees::statement::TableLocalData,
) -> Option<CheckedScalarExpression> {
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};
    if local.initial_value.is_valid()
        || !validation::is_plain_scalar_storage(program, local.type_reference)
    {
        return None;
    }
    let primitive = program.primitive_type_reference(local.type_reference)?;
    let landed_type = match primitive {
        PrimitiveType::Bool => {
            return Some(CheckedScalarExpression::Boolean(Box::new(
                checked_trees::CheckedBooleanExpression::Constant(false),
            )));
        }
        PrimitiveType::F32 => {
            return Some(CheckedScalarExpression::IeeeFloatLiteral {
                value: semantic_vocabulary::IeeeFloatValue::Binary32(0),
            });
        }
        PrimitiveType::F64 => {
            return Some(CheckedScalarExpression::IeeeFloatLiteral {
                value: semantic_vocabulary::IeeeFloatValue::Binary64(0),
            });
        }
        // A selector has no zero value: only a named implementation makes one.
        primitive => primitive.landed_landing()?,
    };
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "0")
            .ok()?
            .with_landing(IntegerLanding {
                landed_type,
                domain: ArithmeticDomain::Exact,
            }),
    })
}

pub(crate) fn is_integer(primitive: PrimitiveType) -> bool {
    !matches!(
        primitive,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
    )
}

pub(crate) fn operator_is_builtin(
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    operators
        .expression_use(expression)
        .is_none_or(|operator_use| {
            operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
        })
}

pub(crate) fn combine_arithmetic_domains(
    left: ArithmeticDomain,
    right: ArithmeticDomain,
) -> Option<ArithmeticDomain> {
    match (left, right) {
        (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => Some(domain),
        (left, right) if left == right => Some(left),
        _ => None,
    }
}

pub(crate) use validation::checked_integer_binary_kind;
