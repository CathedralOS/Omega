//! Bounds valid for every evaluation of immutable, builtin integer expressions.

use super::*;
use language_core::OperatorSpelling;

/// Bound a literal or builtin arithmetic tree over exact immutable primitive
/// parameters. No initializer, caller flow fact, callee body, or mutable place
/// is read: the interval is valid independently of the evaluation snapshot.
pub fn immutable_integer_expression_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let value = bounds(program, machine, state, expression)?;
    Some((value.interval.low?, value.interval.high?))
}

/// Retain one-sided carrier bounds when projecting an exact builtin guard.
/// An unrestricted u64 has a useful zero floor even though its ceiling does
/// not fit the interval engine's i64 endpoint representation.
pub(super) fn builtin_comparison_intervals(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Interval, Interval)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let left = bounds(program, machine, state, binary.left)?;
    let right = bounds(program, machine, state, binary.right)?;
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[left.type_reference, right.type_reference],
    )
    .then_some((left.interval, right.interval))
}

struct Bounds {
    interval: Interval,
    primitive: Option<PrimitiveType>,
    type_reference: Option<TypeReferenceHandle>,
}

fn bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Bounds> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => Some(Bounds {
            interval: literal_interval(literal),
            primitive: None,
            type_reference: None,
        }),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            if parameter.is_self
                || parameter.is_mutable
                || parameter.is_const
                || program.arithmetic_domain_for_type_reference(parameter.type_reference)
                    != ArithmeticDomain::Exact
            {
                return None;
            }
            let primitive = program.primitive_type_reference(parameter.type_reference)?;
            let carrier = primitive_range(primitive)?;
            Some(Bounds {
                interval: enforced_declared_range(program, parameter.type_reference)
                    .map_or(carrier, |range| range.intersect(carrier)),
                primitive: Some(primitive),
                type_reference: Some(parameter.type_reference),
            })
        }
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return None,
            };
            let left = bounds(program, machine, state, binary.left)?;
            let right = bounds(program, machine, state, binary.right)?;
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &[left.type_reference, right.type_reference],
            ) {
                return None;
            }
            if left
                .primitive
                .zip(right.primitive)
                .is_some_and(|(left, right)| left != right)
            {
                return None;
            }
            let primitive = left.primitive.or(right.primitive)?;
            let carrier = primitive_range(primitive)?;
            // The shared i64 interval engine cannot represent abs(i64::MIN).
            // Do not use its saturating divisor magnitude as an exact bound.
            if binary.operator == BinaryOperator::Modulo && right.interval.low == Some(i64::MIN) {
                return None;
            }
            let interval = match binary.operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
                BinaryOperator::Divide => left.interval.divide(right.interval),
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                _ => return None,
            };
            carrier.contains(interval).then_some(Bounds {
                interval,
                primitive: Some(primitive),
                type_reference: None,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
