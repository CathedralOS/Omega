use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::projection::{RankOrder, RankProjection, unwrapped};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparison {
    Equal,
    Strict,
}

pub(super) fn argument_comparison(
    program: &TypedTrees,
    rank: &RankProjection,
    argument: ExpressionHandle,
    guards: &[(ExpressionHandle, bool)],
) -> Option<Comparison> {
    if rank.is_subject(program, argument) {
        return Some(Comparison::Equal);
    }
    let RankOrder::Lexicographic {
        data,
        fields: components,
        ..
    } = &rank.order
    else {
        return component_comparison(program, rank, argument, SymbolHandle::invalid(), guards);
    };
    let ExpressionNode::StructLiteral(literal) = program
        .expression_table
        .expression(unwrapped(program, argument))
    else {
        return None;
    };
    if literal.type_symbol != *data || literal.case_name.is_some() || literal.case_symbol.is_some()
    {
        return None;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let mut strict = false;
    for field in components {
        let mut matching = fields.iter().filter(|value| value.field_symbol == *field);
        let value = matching.next()?.value;
        if matching.next().is_some() {
            return None;
        }
        // A strict earlier component allows arbitrary later components, but
        // all fields still have exact, unique declaration associations.
        if strict {
            continue;
        }
        strict = component_comparison(program, rank, value, *field, guards)? == Comparison::Strict;
    }
    Some(if strict {
        Comparison::Strict
    } else {
        Comparison::Equal
    })
}

fn component_comparison(
    program: &TypedTrees,
    rank: &RankProjection,
    value: ExpressionHandle,
    field: SymbolHandle,
    guards: &[(ExpressionHandle, bool)],
) -> Option<Comparison> {
    if rank.is_component(program, value, field) {
        return Some(Comparison::Equal);
    }
    let ExpressionNode::Binary(binary) = program
        .expression_table
        .expression(unwrapped(program, value))
    else {
        return None;
    };
    if binary.operator != BinaryOperator::Subtract
        || !rank.is_component(program, binary.left, field)
    {
        return None;
    }
    let amount = integer(program, binary.right)?;
    (amount > 0
        && guards.iter().any(|(guard, truth)| {
            guard_proves_lower_bound(program, rank, *guard, *truth, field, amount)
        }))
    .then_some(Comparison::Strict)
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

/// Facts come from the current arm or an earlier failed dispatch guard, all
/// over the same unchanged entry binding. No rendered-name cache or facts
/// from another state/caller contribute to this lower-bound proof.
fn guard_proves_lower_bound(
    program: &TypedTrees,
    rank: &RankProjection,
    guard: ExpressionHandle,
    truth: bool,
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
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        for (condition, boolean) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if let ExpressionNode::Boolean(value) = program
                .expression_table
                .expression(unwrapped(program, boolean))
            {
                return guard_proves_lower_bound(
                    program,
                    rank,
                    condition,
                    truth == (*value == (binary.operator == BinaryOperator::Equal)),
                    field,
                    minimum,
                );
            }
        }
    }
    if (binary.operator == BinaryOperator::And && truth)
        || (binary.operator == BinaryOperator::Or && !truth)
    {
        return guard_proves_lower_bound(program, rank, binary.left, truth, field, minimum)
            || guard_proves_lower_bound(program, rank, binary.right, truth, field, minimum);
    }
    let operator = if truth {
        binary.operator
    } else {
        match binary.operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::Equal => BinaryOperator::NotEqual,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            _ => return false,
        }
    };
    let (operator, bound) = if rank.is_component(program, binary.left, field) {
        (operator, integer(program, binary.right))
    } else if rank.is_component(program, binary.right, field) {
        let operator = match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            BinaryOperator::Equal => BinaryOperator::Equal,
            BinaryOperator::NotEqual => BinaryOperator::NotEqual,
            _ => return false,
        };
        (operator, integer(program, binary.left))
    } else {
        return false;
    };
    bound.is_some_and(|bound| match operator {
        BinaryOperator::Greater => bound >= minimum - 1,
        BinaryOperator::GreaterOrEqual | BinaryOperator::Equal => bound >= minimum,
        BinaryOperator::NotEqual => bound == 0 && minimum == 1,
        _ => false,
    })
}
