use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::projection::{RankProjection, unwrapped};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparison {
    Equal,
    Strict,
}

pub(super) fn argument_comparison(
    program: &TypedTrees,
    rank: &RankProjection,
    argument: ExpressionHandle,
    guard: ExpressionHandle,
) -> Option<Comparison> {
    if rank.is_subject(program, argument) {
        return Some(Comparison::Equal);
    }
    let ExpressionNode::StructLiteral(literal) = program
        .expression_table
        .expression(unwrapped(program, argument))
    else {
        return None;
    };
    if literal.type_symbol != rank.data
        || literal.case_name.is_some()
        || literal.case_symbol.is_some()
    {
        return None;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let mut strict = false;
    for field in &rank.fields {
        let mut matching = fields.iter().filter(|value| value.field_symbol == *field);
        let value = matching.next()?.value;
        if matching.next().is_some() {
            return None;
        }
        // A strict earlier component allows arbitrary later components, but
        // all fields still have exact, unique declaration associations.
        if strict || rank.is_field(program, value, *field) {
            continue;
        }
        let ExpressionNode::Binary(binary) = program
            .expression_table
            .expression(unwrapped(program, value))
        else {
            return None;
        };
        if binary.operator != BinaryOperator::Subtract
            || !rank.is_field(program, binary.left, *field)
        {
            return None;
        }
        let amount = integer(program, binary.right)?;
        if amount <= 0 || !guard_proves_lower_bound(program, rank, guard, *field, amount) {
            return None;
        }
        strict = true;
    }
    Some(if strict {
        Comparison::Strict
    } else {
        Comparison::Equal
    })
}

fn integer(program: &TypedTrees, expression: ExpressionHandle) -> Option<i64> {
    match program
        .expression_table
        .expression(unwrapped(program, expression))
    {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}

/// Only the current arm's live guard is used. There is no previous-arm name
/// cache and no assumption imported from another state or caller. Unsigned
/// component declarations plus this bound prove subtraction remains natural.
fn guard_proves_lower_bound(
    program: &TypedTrees,
    rank: &RankProjection,
    guard: ExpressionHandle,
    field: SymbolHandle,
    minimum: i64,
) -> bool {
    if !guard.is_valid() {
        return false;
    }
    let ExpressionNode::Binary(binary) = program
        .expression_table
        .expression(unwrapped(program, guard))
    else {
        return false;
    };
    if binary.operator == BinaryOperator::Equal {
        for (condition, truth) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if matches!(
                program
                    .expression_table
                    .expression(unwrapped(program, truth)),
                ExpressionNode::Boolean(true)
            ) {
                return guard_proves_lower_bound(program, rank, condition, field, minimum);
            }
        }
    }
    if binary.operator == BinaryOperator::And {
        return guard_proves_lower_bound(program, rank, binary.left, field, minimum)
            || guard_proves_lower_bound(program, rank, binary.right, field, minimum);
    }
    let (operator, bound) = if rank.is_field(program, binary.left, field) {
        (binary.operator, integer(program, binary.right))
    } else if rank.is_field(program, binary.right, field) {
        let operator = match binary.operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Equal => BinaryOperator::Equal,
            _ => return false,
        };
        (operator, integer(program, binary.left))
    } else {
        return false;
    };
    bound.is_some_and(|bound| match operator {
        BinaryOperator::Greater => bound >= minimum - 1,
        BinaryOperator::GreaterOrEqual | BinaryOperator::Equal => bound >= minimum,
        _ => false,
    })
}
