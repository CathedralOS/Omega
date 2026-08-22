use super::{
    BinaryExpression, BinaryOperator, Expression, ExpressionNode, ExpressionTable, NamePath,
    StructLiteral, StructLiteralField, TableBinaryExpression, TableNamePath,
};
use crate::name::Identifier;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn expression_table_stores_recursive_typed_expressions_as_handles() {
    let expression = Expression::Binary(Box::new(BinaryExpression {
        left: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(1)),
        operator: BinaryOperator::Add,
        right: Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(2)),
            operator: BinaryOperator::Add,
            right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(3)),
        })),
    }));

    let mut table = ExpressionTable::new();
    let root = table.insert_tree(&expression);

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
fn expression_table_stores_name_paths_as_member_spans() {
    let expression = Expression::Name(NamePath::resolved(
        vec![
            Identifier::generated("player"),
            Identifier::generated("inventory"),
        ],
        SymbolHandle::from_arena_index(1),
        SymbolHandle::from_arena_index(2),
    ));

    let mut table = ExpressionTable::new();
    let root = table.insert_tree(&expression);
    let ExpressionNode::Name(path) = table.expression(root) else {
        panic!("root expression should be a name path");
    };

    assert_eq!(path.members.count(), 2);
    assert_eq!(path.head_symbol, SymbolHandle::from_arena_index(1));
    assert_eq!(path.symbol, SymbolHandle::from_arena_index(2));
    assert_eq!(table.display_name(root), "player::inventory");
}

#[test]
fn expression_table_copies_table_payloads_without_tree_roundtrip() {
    let room_symbol = SymbolHandle::from_arena_index(3);
    let field_symbol = SymbolHandle::from_arena_index(4);
    let expression = Expression::StructLiteral(StructLiteral {
        type_name: Identifier::generated("Room"),
        case_name: None,
        fields: Arc::from(
            vec![
                StructLiteralField {
                    name: Identifier::generated("name"),
                    value: Expression::String(Arc::from(&b"Hall"[..])),
                },
                StructLiteralField {
                    name: Identifier::generated("open"),
                    value: Expression::Binary(Box::new(BinaryExpression {
                        left: Expression::Name(NamePath::resolved(
                            vec![Identifier::generated("room")],
                            room_symbol,
                            room_symbol,
                        )),
                        operator: BinaryOperator::Equal,
                        right: Expression::Name(NamePath::resolved(
                            vec![
                                Identifier::generated("room"),
                                Identifier::generated("field"),
                            ],
                            room_symbol,
                            field_symbol,
                        )),
                    })),
                },
            ]
            .into_boxed_slice(),
        ),
    });

    let mut source = ExpressionTable::new();
    let root = source.insert_tree(&expression);

    let mut copied = ExpressionTable::new();
    let copied_root = copied.copy_from(&source, root);

    assert_eq!(source.display_name(root), copied.display_name(copied_root));
    assert_eq!(
        expression.display_name(),
        copied.to_tree(copied_root).display_name()
    );

    let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root) else {
        panic!("copied root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);

    let open_field = &copied.struct_fields(struct_literal.fields)[1];
    let ExpressionNode::Binary(binary) = copied.expression(open_field.value) else {
        panic!("copied field should keep its binary expression");
    };
    let ExpressionNode::Name(TableNamePath {
        members,
        head_symbol,
        symbol,
        ..
    }) = copied.expression(binary.right)
    else {
        panic!("copied binary rhs should keep its name path");
    };

    assert_eq!(*head_symbol, room_symbol);
    assert_eq!(*symbol, field_symbol);
    assert_eq!(copied.name_path_members(*members).len(), 2);
}

#[test]
fn filtered_copy_never_inserts_rejected_field_subtrees() {
    let expression = Expression::StructLiteral(StructLiteral {
        type_name: Identifier::generated("Certified"),
        case_name: None,
        fields: Arc::from(
            vec![
                StructLiteralField {
                    name: Identifier::generated("value"),
                    value: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        7,
                    )),
                },
                StructLiteralField {
                    name: Identifier::generated("proof"),
                    value: Expression::Binary(Box::new(BinaryExpression {
                        left: Expression::Integer(
                            psi_numerics::literals::IntegerLiteral::from_value(11),
                        ),
                        operator: BinaryOperator::Add,
                        right: Expression::Integer(
                            psi_numerics::literals::IntegerLiteral::from_value(13),
                        ),
                    })),
                },
            ]
            .into_boxed_slice(),
        ),
    });
    let mut source = ExpressionTable::new();
    let root = source.insert_tree(&expression);
    assert_eq!(source.expression_count(), 5);

    let mut copied = ExpressionTable::new();
    let copied_root =
        copied.copy_from_filtering_struct_literal_fields(&source, root, &|_, field| {
            field.name.as_str() != "proof"
        });

    let ExpressionNode::StructLiteral(literal) = copied.expression(copied_root) else {
        panic!("root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(literal.fields).len(), 1);
    assert_eq!(copied.expression_count(), 2);
    assert!(
        copied
            .iter_expressions()
            .all(|(_, node)| !matches!(node, ExpressionNode::Binary(_))),
        "rejected initializer subtree must not exist even as unreachable arena nodes"
    );
}
