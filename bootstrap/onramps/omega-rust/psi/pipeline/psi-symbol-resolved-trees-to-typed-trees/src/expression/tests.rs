use super::lower_expression_handle_from_table;
use psi_arena::HandleSpan;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

#[test]
fn lowers_binary_expression_directly_into_typed_table() {
    let mut source = resolved::expression::ExpressionTable::new();
    let left = source.insert(resolved::expression::ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(1),
    ));
    let right = source.insert(resolved::expression::ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(2),
    ));
    let expression = source.insert(resolved::expression::ExpressionNode::Binary(
        resolved::expression::TableBinaryExpression {
            left,
            operator: resolved::expression::BinaryOperator::Add,
            right,
        },
    ));

    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("direct lowering should succeed");

    assert_eq!(target.expression_table.display_name(lowered), "1 + 2");
    assert_eq!(target.expression_table.expression_count(), 3);
}

#[test]
fn lowers_expression_spans_directly_into_typed_table() {
    let mut source = resolved::expression::ExpressionTable::new();
    let mut values = HandleSpan::empty();
    let one = source.insert(resolved::expression::ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(1),
    ));
    let two = source.insert(resolved::expression::ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(2),
    ));
    source.push_expression_handle(&mut values, one);
    source.push_expression_handle(&mut values, two);
    let expression = source.insert(resolved::expression::ExpressionNode::ArrayLiteral(values));

    let mut target = typed::TypedTrees::default();
    let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
        .expect("direct lowering should succeed");

    let typed::expression::ExpressionNode::ArrayLiteral(values) =
        target.expression_table.expression(lowered)
    else {
        panic!("root should lower to array literal");
    };

    assert_eq!(values.count(), 2);
    assert_eq!(target.expression_table.display_name(lowered), "[1, 2]");
    assert_eq!(target.expression_table.expression_count(), 3);
}
