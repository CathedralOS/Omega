use super::*;
use typed_trees::expression::TableBinaryExpression;

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
fn decimal_values_land_exactly_at_integer_destinations() {
    for (text, expected) in [("7.0", Some(7)), ("7.5", None)] {
        let mut program = TypedTrees::default();
        let decimal = program.expression_table.insert(ExpressionNode::Float(
            numerics::literals::FloatLiteral::parse(text).expect("decimal literal"),
        ));
        assert_eq!(
            land_anonymous_integer_expression(&program, decimal, PrimitiveType::I32, |_| true)
                .and_then(|literal| literal.value_i64()),
            expected,
            "{text}"
        );
    }
}

#[test]
fn anonymous_landing_rejects_stale_cycles_and_unselected_operations() {
    let mut program = TypedTrees::default();
    let first = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let second = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(4)));
    let root = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: first,
            operator: BinaryOperator::Add,
            right: second,
        }));
    assert_eq!(
        land_anonymous_integer_expression(&program, root, PrimitiveType::U8, |_| true)
            .unwrap()
            .value_u64(),
        Some(7)
    );
    assert!(
        land_anonymous_integer_expression(&program, root, PrimitiveType::U8, |_| false).is_none()
    );
    for invalid in [
        ExpressionHandle::invalid(),
        ExpressionHandle::from_parts(root.arena_index(), root.generation() + 1),
    ] {
        assert!(
            land_anonymous_integer_expression(&program, invalid, PrimitiveType::U8, |_| true)
                .is_none()
        );
    }
    *program.expression_table.expression_mut(root) =
        ExpressionNode::Binary(TableBinaryExpression {
            left: root,
            operator: BinaryOperator::Add,
            right: second,
        });
    assert!(
        land_anonymous_integer_expression(&program, root, PrimitiveType::U8, |_| true).is_none()
    );
}

#[test]
fn anonymous_landing_does_not_have_a_hidden_expression_depth_limit() {
    let mut program = TypedTrees::default();
    let zero = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let mut root = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
    for _ in 0..600 {
        root = program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: root,
                operator: BinaryOperator::Add,
                right: zero,
            }));
    }
    assert_eq!(
        land_anonymous_integer_expression(&program, root, PrimitiveType::U8, |_| true)
            .unwrap()
            .value_u64(),
        Some(7)
    );
}

#[test]
fn partial_division_and_target_width_are_not_guessed() {
    let mut program = TypedTrees::default();
    let first = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
    let second = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
    let root = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: first,
            operator: BinaryOperator::Divide,
            right: second,
        }));
    assert!(
        land_anonymous_integer_expression(&program, root, PrimitiveType::U8, |_| true).is_none()
    );
    assert!(
        land_anonymous_integer_expression(&program, first, PrimitiveType::Addr, |_| true).is_none()
    );
}
