use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, IntegerLiteral, PrimitiveType, TypedTrees,
    evaluate_anonymous_numeric_comparison, land_anonymous_integer_expression_with_warning,
};
use typed_trees::expression::TableBinaryExpression;

mod anonymous_dispatch;

#[test]
fn single_landing_preserves_fractional_warning_value_and_authored_span() {
    let mut program = TypedTrees::default();
    let three = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let two = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
    let quotient = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: three,
            operator: BinaryOperator::Divide,
            right: two,
        }));
    let source_span = source::SourceSpan::new(source::SourceId(7), source::Span::new(20, 25));
    program
        .expression_table
        .set_source_span(quotient, source_span);
    let product = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: quotient,
            operator: BinaryOperator::Multiply,
            right: two,
        }));
    let (literal, warning) = land_anonymous_integer_expression_with_warning(
        &program,
        product,
        PrimitiveType::U64,
        |_| true,
    )
    .expect("exact canceled fraction lands");
    assert_eq!(literal.value_u64(), Some(3));
    let warning = warning.expect("fractional intermediate remains observable");
    assert_eq!(warning.source_span, Some(source_span));
    assert!(warning.message.contains("3/2"));
    assert!(warning.message.contains("integer `3`"));
    assert!(
        land_anonymous_integer_expression_with_warning(
            &program,
            quotient,
            PrimitiveType::U64,
            |_| true,
        )
        .is_none()
    );
    let (_, warning) =
        land_anonymous_integer_expression_with_warning(&program, three, PrimitiveType::U64, |_| {
            true
        })
        .expect("integral leaf lands");
    assert!(warning.is_none());
}

#[test]
fn anonymous_comparison_requires_each_selection_and_rejects_malformed_trees() {
    let mut program = TypedTrees::default();
    let three = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let two = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
    let quotient = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: three,
            operator: BinaryOperator::Divide,
            right: two,
        }));
    let comparison =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: quotient,
                operator: BinaryOperator::Less,
                right: two,
            }));
    assert_eq!(
        evaluate_anonymous_numeric_comparison(&program, comparison, |_| true),
        Some(true)
    );
    for denied in [comparison, quotient] {
        assert_eq!(
            evaluate_anonymous_numeric_comparison(&program, comparison, |expression| expression
                != denied),
            None
        );
    }
    for invalid in [
        ExpressionHandle::invalid(),
        ExpressionHandle::from_parts(comparison.arena_index(), comparison.generation() + 1),
    ] {
        assert_eq!(
            evaluate_anonymous_numeric_comparison(&program, invalid, |_| true),
            None
        );
    }
    assert_eq!(
        evaluate_anonymous_numeric_comparison(&program, quotient, |_| true),
        None
    );
    for invalid_operand in [ExpressionHandle::invalid(), comparison] {
        *program.expression_table.expression_mut(comparison) =
            ExpressionNode::Binary(TableBinaryExpression {
                left: invalid_operand,
                operator: BinaryOperator::Less,
                right: two,
            });
        assert_eq!(
            evaluate_anonymous_numeric_comparison(&program, comparison, |_| true),
            None
        );
    }
}
