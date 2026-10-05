//! Whole readable primitive referents are storage reads, not scalar formals.

use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};

pub(super) fn lower(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    expected: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let (value, _) = declared(program, parameters, expression)?;
    (super::scalar_expression_type(&value) == Some(expected)).then_some(value)
}

/// Primitive reference reads are ordinary leaves, including below operators.
/// Their declared referee supplies both carrier and arithmetic semantics; an
/// enclosing expression's expected result must not invent either property.
pub(super) fn declared(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<(
    CheckedScalarExpression,
    numerics::arithmetic::ArithmeticDomain,
)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || program
            .expression_table
            .name_path_member_symbols(name.member_symbols)
            .first()
            .is_some_and(|symbol| *symbol != name.symbol)
    {
        return None;
    }
    let mut matching = parameters
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let parameter = matching.next()?;
    if (matching.next().is_some()) || parameter.is_const {
        return None;
    }
    let (primitive_type, arithmetic_domain) = readable_type(program, parameter.type_reference)?;
    let value = if primitive_type == PrimitiveType::Bool {
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
            symbol: parameter.symbol,
        }))
    } else {
        CheckedScalarExpression::StorageRead {
            symbol: parameter.symbol,
            primitive_type,
        }
    };
    Some((value, arithmetic_domain))
}

/// Observation depends on the declared referee and current carrier, never on
/// whether its producer was a borrow, recast, call, or transferred reference.
/// Access and live custody are independently checked when that read is lowered.
pub(super) fn local(
    program: &TypedTrees,
    local: &typed_trees::statement::TableLocalData,
) -> Option<super::expression_plans::ScalarLocal> {
    let (primitive_type, arithmetic_domain) = readable_type(program, local.type_reference)?;
    Some(super::expression_plans::ScalarLocal {
        is_mutable: local.is_mutable,
        symbol: local.symbol,
        name: local.name.as_str().to_owned(),
        primitive_type,
        arithmetic_domain,
        is_reference: true,
    })
}

fn readable_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, numerics::arithmetic::ArithmeticDomain)> {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !matches!(
        access,
        language_semantics::ReferenceAccess::Shared | language_semantics::ReferenceAccess::Mutable
    ) {
        return None;
    }
    // Reading an established value does not assert a new membership fact.
    // Formation retains its qualifications; the referee still selects the
    // arithmetic policy used by the observation and its enclosing operators.
    let primitive_type = program.primitive_type_reference(*referee)?;
    (primitive_type != PrimitiveType::Addr).then(|| {
        (
            primitive_type,
            program.arithmetic_domain_for_type_reference(*referee),
        )
    })
}
