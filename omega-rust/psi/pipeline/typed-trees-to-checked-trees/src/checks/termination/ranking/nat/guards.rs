use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

use super::super::patterns;

fn comparison(
    program: &typed_trees::TypedTrees,
    fact: patterns::GuardFact,
) -> Option<(ExpressionHandle, BinaryOperator, ExpressionHandle)> {
    let mut expression = fact.expression;
    let mut holds = fact.holds;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                holds = !holds;
                expression = unary.operand;
            }
            ExpressionNode::Binary(binary) => {
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    let wrapped = match (
                        program.expression_table.expression(binary.left),
                        program.expression_table.expression(binary.right),
                    ) {
                        (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                        (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                        _ => None,
                    };
                    if let Some((operand, value)) = wrapped {
                        holds = holds == (value == (binary.operator == BinaryOperator::Equal));
                        expression = operand;
                        continue;
                    }
                }
                let operator = if holds {
                    binary.operator
                } else {
                    match binary.operator {
                        BinaryOperator::Equal => BinaryOperator::NotEqual,
                        BinaryOperator::NotEqual => BinaryOperator::Equal,
                        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
                        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
                        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
                        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
                        _ => return None,
                    }
                };
                return Some((binary.left, operator, binary.right));
            }
            _ => return None,
        }
    }
}

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
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Name(name) if name.symbol == parameter.symbol)
}

fn positive(
    program: &typed_trees::TypedTrees,
    guards: &[patterns::GuardFact],
    nonnegative: bool,
    matches: impl Fn(ExpressionHandle) -> bool,
) -> bool {
    guards.iter().any(|guard| {
        let Some((left, operator, right)) = comparison(program, *guard) else {
            return false;
        };
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
        match operator {
            BinaryOperator::Greater => literal.value_i64() == Some(0),
            BinaryOperator::NotEqual => nonnegative && literal.value_i64() == Some(0),
            BinaryOperator::GreaterOrEqual => literal.value_i64() == Some(1),
            _ => false,
        }
    })
}

pub(super) fn guard_is_positive_parameter(
    program: &typed_trees::TypedTrees,
    guards: &[patterns::GuardFact],
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::types::PrimitiveType;
    let nonnegative = matches!(
        program.primitive_type_reference(parameter.type_reference),
        Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
    );
    positive(program, guards, nonnegative, |expression| {
        exact_parameter(program, expression, parameter)
    })
}

pub(super) fn guard_is_positive_parameter_member(
    program: &typed_trees::TypedTrees,
    guards: &[patterns::GuardFact],
    parameter: &typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    positive(program, guards, false, |expression| {
        matches!(program.expression_table.expression(expression),
            ExpressionNode::Member(member) if member.member.as_str() == member_name
                && exact_parameter(program, member.receiver, parameter))
    })
}

pub(super) fn guard_is_index_below_limit(
    program: &typed_trees::TypedTrees,
    guards: &[patterns::GuardFact],
    index_parameter: &typed_trees::signature::StateParameter,
    limit_parameter: &typed_trees::signature::StateParameter,
) -> bool {
    guards.iter().any(|guard| {
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
