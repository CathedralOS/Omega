//! Closed numeric observations under simultaneous application telescopes.
//! This supplies local correspondence only; ordinary and portable commitment
//! encodings keep their structural expression grammar.

use super::{IndexIdentityArgument, identity_context::TypeIdentityContext};
use crate::{
    TypedTrees,
    data::TypeParameterKind,
    expression::{ExpressionHandle, ExpressionNode},
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use numerics::bignum::BigInt;

pub(super) fn integer_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> Option<BigInt> {
    Some(value(program, expression, context)?.value)
}

fn value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> Option<crate::closed_numeric::ClosedIntegerValue> {
    program.closed_integer_value_with_bindings(
        expression,
        context.application_owner,
        &mut |expression| {
            let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
                return None;
            };
            let parameter = program.first_data_type_parameter_by_symbol(path.symbol)?;
            if !matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                return None;
            }
            // The returned context excludes this telescope: every actual
            // belongs to its supplier, not another slot in the same binding.
            let (argument, outer) = context.applied_argument(path.symbol)?;
            let reference = match argument {
                IndexIdentityArgument::TypeReference(reference) => reference,
                IndexIdentityArgument::StaticArgument(argument)
                    if argument.application.is_none()
                        && argument.evidence_projection.is_none()
                        && argument.type_reference.is_valid() =>
                {
                    argument.type_reference
                }
                IndexIdentityArgument::Expression(expression) => {
                    let observed = value(program, expression, &outer)?;
                    return Some((observed.value, observed.type_reference?));
                }
                _ => return None,
            };
            argument_value(program, reference, &outer)
        },
    )
}

fn argument_value(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> Option<(BigInt, TypeReferenceHandle)> {
    if !program.contains_type_reference(reference) {
        return None;
    }
    let context = context.enter_type_operand(program, reference);
    match program.type_reference_node(reference) {
        TypeReferenceNode::ConstExpression(expression) => {
            let observed = value(program, *expression, &context)?;
            Some((observed.value, observed.type_reference?))
        }
        TypeReferenceNode::Named { symbol, .. } => {
            let declaration = program.const_declaration_by_symbol(*symbol)?;
            let observed = value(program, declaration.materialized_initializer, &context)?;
            // The evaluator rechecks fit in this actual declaration's carrier.
            // No formal or destination type supplies missing operand custody.
            Some((observed.value, declaration.declared_type))
        }
        _ => None,
    }
}
