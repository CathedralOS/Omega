//! Scalar Boolean structure shared by domain and exit proofs. Leaf custody is
//! supplied by the caller; this module does not replay initializers or choose
//! an arithmetic domain for runtime operations.

use psi_numerics::bignum::BigInt;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::checks::contracts) enum ScalarValue {
    Integer(BigInt),
    Boolean(bool),
}

pub(super) fn literal(program: &TypedTrees, expression: ExpressionHandle) -> Option<ScalarValue> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
        ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
        _ => None,
    }
}

pub(in crate::checks::contracts) fn evaluate(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
) -> Option<ScalarValue> {
    evaluate_with_comparisons(program, expression, resolve_leaf, &|_, _| true)
}

/// Comparison meaning is supplied by the consumer independently of leaf
/// values. In particular, the domain prover has no selected-operator evidence
/// for Boolean equality merely because both operands have Boolean values.
pub(super) fn evaluate_with_comparisons(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
) -> Option<ScalarValue> {
    if !expression.is_valid() {
        return None;
    }
    if let Some(value) = literal(program, expression) {
        return Some(value);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            evaluate_with_comparisons(program, borrow.target, resolve_leaf, comparison_is_admitted)
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let ScalarValue::Boolean(value) = evaluate_with_comparisons(
                program,
                unary.operand,
                resolve_leaf,
                comparison_is_admitted,
            )?
            else {
                return None;
            };
            Some(ScalarValue::Boolean(!value))
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
            let left = evaluate_with_comparisons(
                program,
                binary.left,
                resolve_leaf,
                comparison_is_admitted,
            )?;
            let right = evaluate_with_comparisons(
                program,
                binary.right,
                resolve_leaf,
                comparison_is_admitted,
            )?;
            if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && !comparison_is_admitted(&left, &right)
            {
                return None;
            }
            let value = match (left, right) {
                (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => {
                    match binary.operator {
                        BinaryOperator::And => left && right,
                        BinaryOperator::Or => left || right,
                        BinaryOperator::Equal => left == right,
                        BinaryOperator::NotEqual => left != right,
                        _ => return None,
                    }
                }
                (ScalarValue::Integer(left), ScalarValue::Integer(right)) => {
                    match binary.operator {
                        BinaryOperator::Equal => left == right,
                        BinaryOperator::NotEqual => left != right,
                        BinaryOperator::Less => left < right,
                        BinaryOperator::LessOrEqual => left <= right,
                        BinaryOperator::Greater => left > right,
                        BinaryOperator::GreaterOrEqual => left >= right,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            Some(ScalarValue::Boolean(value))
        }
        // Arithmetic, calls, casts, and projections need the caller's exact
        // value evidence. Literal operands do not grant a runtime arithmetic
        // interpretation here (e.g. wrapping u8 addition is not i64 addition).
        _ => resolve_leaf(expression),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_typed_trees::expression::{TableBinaryExpression, TableUnaryExpression};

    #[test]
    fn unadmitted_boolean_equality_cannot_become_domain_evidence() {
        let mut program = TypedTrees::default();
        let yes = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let equal =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: yes,
                    operator: BinaryOperator::Equal,
                    right: yes,
                }));
        let conjunction =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: yes,
                    operator: BinaryOperator::And,
                    right: equal,
                }));
        for expression in [equal, conjunction] {
            assert_eq!(
                evaluate_with_comparisons(
                    &program,
                    expression,
                    &mut |_| None,
                    &|left, right| matches!(
                        (left, right),
                        (ScalarValue::Integer(_), ScalarValue::Integer(_))
                    )
                ),
                None
            );
            assert_eq!(
                evaluate(&program, expression, &mut |_| None),
                Some(ScalarValue::Boolean(true))
            );
        }
    }

    #[test]
    fn scalar_boolean_structure_preserves_type_and_polarity() {
        let mut program = TypedTrees::default();
        let yes = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let no = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        for (operator, expected) in [
            (BinaryOperator::Equal, false),
            (BinaryOperator::NotEqual, true),
            (BinaryOperator::And, false),
            (BinaryOperator::Or, true),
        ] {
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: yes,
                        operator,
                        right: no,
                    }));
            assert_eq!(
                evaluate(&program, expression, &mut |_| None),
                Some(ScalarValue::Boolean(expected))
            );
        }
        let negated =
            program
                .expression_table
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::LogicalNot,
                    operand: yes,
                }));
        assert_eq!(
            evaluate(&program, negated, &mut |_| None),
            Some(ScalarValue::Boolean(false))
        );
        assert_eq!(
            evaluate(&program, ExpressionHandle::invalid(), &mut |_| Some(
                ScalarValue::Integer(BigInt::from_i64(0))
            )),
            None
        );
    }

    #[test]
    fn scalar_arithmetic_needs_explicit_leaf_evidence() {
        let mut program = TypedTrees::default();
        let left = program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(255),
        ));
        let right = program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(1),
        ));
        let sum = program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Add,
                right,
            }));
        assert_eq!(evaluate(&program, sum, &mut |_| None), None);
        assert_eq!(
            evaluate(&program, sum, &mut |expression| (expression == sum)
                .then_some(ScalarValue::Integer(BigInt::from_i64(0)))),
            Some(ScalarValue::Integer(BigInt::from_i64(0)))
        );
        let less = program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Less,
                right,
            }));
        assert_eq!(
            evaluate(&program, less, &mut |_| None),
            Some(ScalarValue::Boolean(false))
        );
        let boolean = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let mixed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right: boolean,
                }));
        assert_eq!(evaluate(&program, mixed, &mut |_| None), None);
    }

    #[test]
    fn scalar_comparisons_preserve_full_width_and_radix_independence() {
        use psi_numerics::literals::{IntegerLiteral, IntegerRadix};
        let mut program = TypedTrees::default();
        let unsigned_maximum = program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615")
                .expect("u64 maximum"),
        ));
        let signed_maximum =
            program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(
                    i64::MAX,
                )));
        let hexadecimal_maximum = program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "ffffffffffffffff")
                .expect("hex u64 maximum"),
        ));
        for (operator, right) in [
            (BinaryOperator::Greater, signed_maximum),
            (BinaryOperator::Equal, hexadecimal_maximum),
        ] {
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: unsigned_maximum,
                        operator,
                        right,
                    }));
            assert_eq!(
                evaluate(&program, expression, &mut |_| None),
                Some(ScalarValue::Boolean(true))
            );
        }
    }
}
