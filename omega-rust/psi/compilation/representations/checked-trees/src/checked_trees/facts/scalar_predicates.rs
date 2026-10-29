//! Scalar Boolean structure shared by domain and exit proofs. Leaf custody is
//! supplied by the caller; this module does not replay initializers or choose
//! an arithmetic domain for runtime operations.

use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

pub use facts::ScalarValue;

/// Selected Boolean structure, independent of the representation supplying its
/// children and scalar observations. Arithmetic meaning stays with the owner
/// that supplies a leaf value, not this predicate interpreter.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub(super) enum PredicateNode<Reference, Leaf> {
    #[default]
    Missing,
    Value(ScalarValue),
    Leaf(Leaf),
    Forward(Reference),
    Not(Reference),
    Binary {
        operator: BinaryOperator,
        left: Reference,
        right: Reference,
    },
}

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
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) =>
        {
            PredicateNode::Binary {
                operator: binary.operator,
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

/// One selective evaluator serves live source proofs and retained predicates.
/// The worklist owns traversal; a captured graph needs no source-tree access.
pub(super) fn evaluate_nodes<Reference: Copy + PartialEq, Leaf>(
    root: Reference,
    node: &mut impl FnMut(Reference) -> Option<PredicateNode<Reference, Leaf>>,
    resolve_leaf: &mut impl FnMut(Leaf) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
) -> Option<ScalarValue> {
    enum Step<Reference> {
        Enter(Reference),
        Leave,
        Not,
        Left(BinaryOperator, Reference),
        Right(BinaryOperator, ScalarValue),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut active = Vec::new();
    let mut value = None;
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(reference) => {
                if active.contains(&reference) {
                    return None;
                }
                active.push(reference);
                pending.push(Step::Leave);
                match node(reference)? {
                    PredicateNode::Missing => return None,
                    PredicateNode::Value(result) => value = Some(result),
                    PredicateNode::Leaf(leaf) => value = Some(resolve_leaf(leaf)?),
                    PredicateNode::Forward(child) => pending.push(Step::Enter(child)),
                    PredicateNode::Not(child) => {
                        pending.push(Step::Not);
                        pending.push(Step::Enter(child));
                    }
                    PredicateNode::Binary {
                        operator,
                        left,
                        right,
                    } => {
                        pending.push(Step::Left(operator, right));
                        pending.push(Step::Enter(left));
                    }
                }
            }
            Step::Leave => {
                active.pop()?;
            }
            Step::Not => {
                let ScalarValue::Boolean(operand) = value.take()? else {
                    return None;
                };
                value = Some(ScalarValue::Boolean(!operand));
            }
            Step::Left(operator, right) => {
                let left = value.take()?;
                // A skipped operand supplies no value premise or comparison
                // authority. Preserve source left-to-right short circuiting.
                match (&left, operator) {
                    (ScalarValue::Boolean(false), BinaryOperator::And)
                    | (ScalarValue::Boolean(true), BinaryOperator::Or) => value = Some(left),
                    _ => {
                        pending.push(Step::Right(operator, left));
                        pending.push(Step::Enter(right));
                    }
                }
            }
            Step::Right(operator, left) => {
                let right = value.take()?;
                if !matches!(operator, BinaryOperator::And | BinaryOperator::Or)
                    && !comparison_is_admitted(&left, &right)
                {
                    return None;
                }
                let result = match (left, right) {
                    (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => match operator {
                        BinaryOperator::And => left && right,
                        BinaryOperator::Or => left || right,
                        BinaryOperator::Equal => left == right,
                        BinaryOperator::NotEqual => left != right,
                        _ => return None,
                    },
                    (ScalarValue::Integer(left), ScalarValue::Integer(right)) => {
                        operator.evaluate_comparison(&left, &right)?
                    }
                    _ => return None,
                };
                value = Some(ScalarValue::Boolean(result));
            }
        }
    }
    value
}
