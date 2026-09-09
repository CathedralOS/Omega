//! Whole readable primitive referents are storage reads, not scalar formals.

use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    signature::StateParameter,
    types::{PrimitiveType, TypeReferenceNode},
};

pub(super) fn lower(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    expected: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    if !program.expression_table.expression_is_valid(expression) || expected == PrimitiveType::Addr
    {
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
    if matching.next().is_some() || parameter.is_self || parameter.is_const {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    if !matches!(
        access,
        language_semantics::ReferenceAccess::Shared | language_semantics::ReferenceAccess::Mutable
    ) || !matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) || program.primitive_type_reference(*referee) != Some(expected)
    {
        return None;
    }
    Some(if expected == PrimitiveType::Bool {
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
            symbol: parameter.symbol,
        }))
    } else {
        CheckedScalarExpression::StorageRead {
            symbol: parameter.symbol,
            primitive_type: expected,
        }
    })
}
