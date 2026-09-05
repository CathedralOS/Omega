use super::*;
use typed_trees::expression::TableBinaryExpression;

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
