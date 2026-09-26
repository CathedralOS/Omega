use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode,
};

use super::super::patterns;

use patterns::comparison;

fn reverse(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn exact_parameter(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter,
) -> bool {
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Name(name) if name.symbol == parameter.symbol)
}

fn positive(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    guards: &[patterns::GuardFact],
    nonnegative: bool,
    matches: impl Fn(ExpressionHandle) -> bool,
) -> bool {
    guards
        .iter()
        .any(|guard| positive_guard(program, state, guard, nonnegative, &matches))
}

/// One guard fact read for a positive-parameter test. A `when` conjunction
/// that holds supplies each conjunct as a fact, so a guard spelled
/// `remaining > 0 && acc < 1000` carries the same positivity as `remaining >
/// 0` alone.
fn positive_guard(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    guard: &patterns::GuardFact,
    nonnegative: bool,
    matches: &impl Fn(ExpressionHandle) -> bool,
) -> bool {
    if !super::has_builtin_meaning(program, state, guard.expression) {
        return false;
    }
    let Some((left, operator, right)) = comparison(program, *guard) else {
        return false;
    };
    // `comparison` unwraps the Boolean equality a dispatch wraps its guard in;
    // an `And` it surfaces therefore already holds, and both conjuncts are
    // facts. A FAILED conjunction stays inside the same `And` operator's
    // operand pair only when the fact can decompose, which it cannot, so
    // `comparison` returns `None` for it and no conjunct is read.
    if operator == BinaryOperator::And {
        return [left, right].iter().any(|&operand| {
            positive_guard(
                program,
                state,
                &patterns::GuardFact {
                    expression: operand,
                    holds: true,
                },
                nonnegative,
                matches,
            )
        });
    }
    let (operator, bound) = if matches(left) {
        (operator, right)
    } else if matches(right) {
        (reverse(operator), left)
    } else {
        return false;
    };
    let ExpressionNode::Integer(literal) = program.expression_table.expression(bound) else {
        return false;
    };
    // Any literal bound that places the parameter at one or above: `x > k`
    // for k >= 0, `x >= k` for k >= 1, and `x != 0` over an unsigned carrier.
    let Some(bound) = literal.value_i64() else {
        return false;
    };
    match operator {
        BinaryOperator::Greater => bound >= 0,
        BinaryOperator::NotEqual => nonnegative && bound == 0,
        BinaryOperator::GreaterOrEqual => bound >= 1,
        _ => false,
    }
}

pub(super) fn guard_is_positive_parameter(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    guards: &[patterns::GuardFact],
    parameter: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter,
) -> bool {
    use symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType;
    let nonnegative = matches!(
        program.primitive_type_reference(parameter.type_reference),
        Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
    );
    positive(program, state, guards, nonnegative, |expression| {
        exact_parameter(program, expression, parameter)
    })
}

pub(super) fn guard_is_positive_parameter_member(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    guards: &[patterns::GuardFact],
    parameter: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    positive(program, state, guards, false, |expression| {
        matches!(program.expression_table.expression(expression),
            ExpressionNode::Member(member) if member.member.as_str() == member_name
                && exact_parameter(program, member.receiver, parameter))
    })
}

pub(super) fn guard_is_index_below_limit(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    guards: &[patterns::GuardFact],
    index_parameter: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter,
    limit_parameter: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter,
) -> bool {
    guards.iter().any(|guard| {
        if !super::has_builtin_meaning(program, state, guard.expression) {
            return false;
        }
        let Some((left, operator, right)) = comparison(program, *guard) else {
            return false;
        };
        let (left, right) = match operator {
            BinaryOperator::Less => (left, right),
            BinaryOperator::Greater => (right, left),
            _ => return false,
        };
        exact_parameter(program, left, index_parameter)
            && patterns::expression_matches_parameter(program, right, limit_parameter)
    })
}
