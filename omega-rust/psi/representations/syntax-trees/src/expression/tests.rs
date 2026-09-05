use super::{BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression};
use crate::identifier::Identifier;
use arena::HandleSpan;

#[test]
fn expression_table_stores_recursive_expressions_as_handles() {
    let mut table = ExpressionTable::new();
    let one = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let two = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let three = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(3),
    ));
    let nested = table.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: two,
        operator: BinaryOperator::Add,
        right: three,
    }));
    let root = table.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: one,
        operator: BinaryOperator::Add,
        right: nested,
    }));

    assert_eq!(table.expression_count(), 5);
    assert_eq!(table.display_name(root), "1 + 2 + 3");

    let ExpressionNode::Binary(TableBinaryExpression { left, right, .. }) = table.expression(root)
    else {
        panic!("root expression should be binary");
    };

    assert!(left.is_valid());
    assert!(right.is_valid());
}

#[test]
fn expression_table_stores_array_children_as_handle_spans() {
    let mut table = ExpressionTable::new();
    let one = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let two = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let three = table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(3),
    ));
    let values = table.insert_expression_handles([one, two, three]);
    let root = table.insert(ExpressionNode::ArrayLiteral(values));
    let ExpressionNode::ArrayLiteral(values) = table.expression(root) else {
        panic!("root expression should be array literal");
    };

    assert_eq!(values.count(), 3);
    assert_eq!(table.display_name(root), "[1, 2, 3]");
}

#[test]
fn expression_table_stores_name_paths_as_member_spans() {
    let mut table = ExpressionTable::new();
    let first = table.append_identifier_path_member(Identifier::generated("player"));
    let _second = table.append_identifier_path_member(Identifier::generated("inventory"));
    let root = table.insert(ExpressionNode::Name(HandleSpan::from_parts(first, 2)));
    let ExpressionNode::Name(path) = table.expression(root) else {
        panic!("root expression should be a name path");
    };

    assert_eq!(path.count(), 2);
    assert_eq!(table.display_name(root), "player::inventory");
}
