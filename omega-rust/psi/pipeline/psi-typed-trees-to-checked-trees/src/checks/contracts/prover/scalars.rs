//! Scalar Boolean structure shared by domain and exit proofs. Leaf custody is
//! supplied by the caller; this module does not replay initializers or choose
//! an arithmetic domain for runtime operations.

#[cfg(test)]
use psi_numerics::bignum::BigInt;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};

pub(in crate::checks::contracts) use psi_facts::ScalarValue;

/// Closed Boolean facts need no state-entry premise. Resolve no names or
/// runtime leaves, and use only the independently selected builtin meanings.
pub(in crate::checks::contracts) fn closed_boolean_value(
    program: &TypedTrees,
    operators: &psi_checked_trees::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> Option<bool> {
    if !has_builtin_operators(program, operators, expression) {
        return None;
    }
    let ScalarValue::Boolean(value) = evaluate(program, expression, &mut |_| None)? else {
        return None;
    };
    Some(value)
}

pub(in crate::checks::contracts) fn has_builtin_operators(
    program: &TypedTrees,
    operators: &psi_checked_trees::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    if operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.status
                != psi_checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
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

pub(super) fn literal(program: &TypedTrees, expression: ExpressionHandle) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
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
    if !program.expression_table.expression_is_valid(expression) {
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
            // An unevaluated Boolean operand supplies neither a value premise
            // nor a comparison obligation. Leaf lookup must follow the same
            // selective schedule as the expression it is proving.
            match (&left, binary.operator) {
                (ScalarValue::Boolean(false), BinaryOperator::And) => {
                    return Some(ScalarValue::Boolean(false));
                }
                (ScalarValue::Boolean(true), BinaryOperator::Or) => {
                    return Some(ScalarValue::Boolean(true));
                }
                _ => {}
            }
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
    use psi_checked_trees::{
        CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    };
    use psi_typed_trees::expression::{TableBinaryExpression, TableUnaryExpression};

    #[test]
    fn closed_boolean_values_preserve_false_and_selected_operator_identity() {
        let mut program = TypedTrees::default();
        for left in [false, true] {
            for right in [false, true] {
                let left_expression = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(left));
                let right_expression = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(right));
                for (operator, expected) in [
                    (BinaryOperator::Equal, left == right),
                    (BinaryOperator::NotEqual, left != right),
                ] {
                    let expression = program.expression_table.insert(ExpressionNode::Binary(
                        TableBinaryExpression {
                            operator,
                            left: left_expression,
                            right: right_expression,
                        },
                    ));
                    let mut operators = CheckedOperatorFacts::default();
                    assert_eq!(
                        closed_boolean_value(&program, &operators, expression),
                        Some(expected)
                    );
                    operators.uses.append(CheckedOperatorUseFact {
                        expression,
                        status: CheckedOperatorResolutionStatus::BuiltinFallback,
                        ..Default::default()
                    });
                    assert_eq!(
                        closed_boolean_value(&program, &operators, expression),
                        Some(expected)
                    );
                    // Even a conflicting later row must not be hidden by the
                    // first matching builtin row or by an enclosing negation.
                    operators.uses.append(CheckedOperatorUseFact {
                        expression,
                        status: CheckedOperatorResolutionStatus::Resolved,
                        ..Default::default()
                    });
                    assert_eq!(closed_boolean_value(&program, &operators, expression), None);
                    let negated = program.expression_table.insert(ExpressionNode::Unary(
                        TableUnaryExpression {
                            operator: UnaryOperator::LogicalNot,
                            operand: expression,
                        },
                    ));
                    assert_eq!(closed_boolean_value(&program, &operators, negated), None);
                }
            }
        }
    }

    #[test]
    fn short_circuit_proofs_do_not_query_the_unevaluated_operand() {
        let mut program = TypedTrees::default();
        let unknown = program.expression_table.insert(ExpressionNode::Name(
            psi_typed_trees::expression::TableNamePath::default(),
        ));
        for (operator, left_value) in [(BinaryOperator::And, false), (BinaryOperator::Or, true)] {
            let left = program
                .expression_table
                .insert(ExpressionNode::Boolean(left_value));
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left,
                        operator,
                        right: unknown,
                    }));
            assert_eq!(
                evaluate(&program, expression, &mut |_| panic!(
                    "queried skipped operand"
                )),
                Some(ScalarValue::Boolean(left_value)),
            );
            let evaluated_left = program
                .expression_table
                .insert(ExpressionNode::Boolean(!left_value));
            let evaluated =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: evaluated_left,
                        operator,
                        right: unknown,
                    }));
            let mut lookups = 0;
            assert_eq!(
                evaluate(&program, evaluated, &mut |leaf| {
                    assert_eq!(leaf, unknown);
                    lookups += 1;
                    None
                }),
                None
            );
            assert_eq!(lookups, 1);
        }
    }

    #[test]
    fn absent_and_stale_expressions_never_supply_dummy_zero_evidence() {
        let mut program = TypedTrees::default();
        let zero = program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::zero(),
        ));
        assert_eq!(
            literal(&program, zero),
            Some(ScalarValue::Integer(BigInt::zero()))
        );
        for missing in [
            ExpressionHandle::invalid(),
            ExpressionHandle::from_parts(zero.arena_index(), zero.generation() + 1),
            ExpressionHandle::from_parts(u32::MAX, zero.generation()),
        ] {
            assert_eq!(literal(&program, missing), None);
            assert_eq!(
                evaluate(&program, missing, &mut |_| panic!(
                    "missing source reached leaf lookup"
                )),
                None
            );
        }
    }

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
