//! Ordered guard facts require the current expression's builtin meaning.

use super::*;
use language_core::OperatorSpelling;

pub(super) fn builtin_ordering(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    let spelling = match comparison.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ],
    )
}

fn operand_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    // This is type lookup, not an immutable-value proof: mutable parameters
    // and locals keep ordinary evaluation-snapshot narrowing. Literal and
    // unresolved computed types remain wildcard candidates, never an assumed
    // copy of the other operand's carrier that could hide an overload.
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            crate::expression_types::named_value_type_reference(program, path)
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Call(_) => {
            declared_place_type_raw(program, machine, state, expression)
        }
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        // In particular, do not let declared_place_type_raw erase a Borrow
        // shell and incorrectly rule out a reference-typed operator candidate.
        _ => None,
    }?;
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}
