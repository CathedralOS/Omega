//! Scalar Boolean structure shared by domain and exit proofs. Leaf custody is
//! supplied by the caller; this module does not replay initializers or choose
//! an arithmetic domain for runtime operations.

use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

pub use numerics::scalar_predicates::ScalarValue;
pub(super) use numerics::scalar_predicates::{PredicateNode, PredicateOperator, evaluate_nodes};

/// Closed Boolean facts need no state-entry premise. Resolve no names or
/// runtime leaves, and use only the independently selected builtin meanings.
pub fn closed_boolean_value(
    program: &typed_trees::TypedTrees,
    operators: &crate::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> Option<bool> {
    if !has_builtin_operators(program, operators, expression) {
        return None;
    }
    let ScalarValue::Boolean(value) = evaluate(program, expression, &mut |_| None, &|_, _| true)?
    else {
        return None;
    };
    Some(value)
}

pub fn has_builtin_operators(
    program: &typed_trees::TypedTrees,
    operators: &crate::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    if operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.status != crate::CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            has_builtin_operators(program, operators, binary.left)
                && has_builtin_operators(program, operators, binary.right)
        }
        ExpressionNode::Unary(unary) => has_builtin_operators(program, operators, unary.operand),
        ExpressionNode::Borrow(borrow) => has_builtin_operators(program, operators, borrow.target),
        _ => true,
    }
}

/// Comparison meaning is supplied by the consumer independently of leaf
/// values. In particular, the domain prover has no selected-operator evidence
/// for Boolean equality merely because both operands have Boolean values.
pub fn evaluate(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
) -> Option<ScalarValue> {
    evaluate_for_atoms(
        program,
        expression,
        resolve_leaf,
        comparison_is_admitted,
        &mut |_| None,
    )
}

/// Exact semantic observations can supply Boolean atoms before their retained
/// syntax is decomposed. The observation owns its meaning and subject custody;
/// ordinary scalar comparisons keep their independent admission check.
pub fn evaluate_for_atoms(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
    observe_boolean: &mut impl FnMut(ExpressionHandle) -> Option<bool>,
) -> Option<ScalarValue> {
    evaluate_nodes(
        expression,
        &mut |expression| source_node(program, expression, observe_boolean),
        resolve_leaf,
        comparison_is_admitted,
    )
}

pub(super) fn source_node(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    observe_boolean: &mut impl FnMut(ExpressionHandle) -> Option<bool>,
) -> Option<PredicateNode<ExpressionHandle, ExpressionHandle>> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let Some(value) = program.static_boolean_value(expression) {
        return Some(PredicateNode::Value(ScalarValue::Boolean(value)));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => {
            return value
                .value_bignum()
                .map(|value| PredicateNode::Value(ScalarValue::Integer(value)));
        }
        ExpressionNode::Boolean(value) => {
            return Some(PredicateNode::Value(ScalarValue::Boolean(*value)));
        }
        _ => {}
    }
    if let Some(value) = observe_boolean(expression) {
        return Some(PredicateNode::Value(ScalarValue::Boolean(value)));
    }
    Some(match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => PredicateNode::Forward(borrow.target),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            PredicateNode::Not(unary.operand)
        }
        ExpressionNode::Binary(binary) if predicate_operator(binary.operator).is_some() => {
            PredicateNode::Binary {
                operator: predicate_operator(binary.operator)?,
                left: binary.left,
                right: binary.right,
            }
        }
        // Arithmetic, calls, casts, and projections need the caller's exact
        // value evidence. Literal operands do not grant a runtime arithmetic
        // interpretation here (e.g. wrapping u8 addition is not i64 addition).
        _ => PredicateNode::Leaf(expression),
    })
}

fn predicate_operator(operator: BinaryOperator) -> Option<PredicateOperator> {
    Some(match operator {
        BinaryOperator::And => PredicateOperator::And,
        BinaryOperator::Or => PredicateOperator::Or,
        BinaryOperator::Equal => PredicateOperator::Equal,
        BinaryOperator::NotEqual => PredicateOperator::NotEqual,
        BinaryOperator::Less => PredicateOperator::Less,
        BinaryOperator::LessOrEqual => PredicateOperator::LessOrEqual,
        BinaryOperator::Greater => PredicateOperator::Greater,
        BinaryOperator::GreaterOrEqual => PredicateOperator::GreaterOrEqual,
        _ => return None,
    })
}
