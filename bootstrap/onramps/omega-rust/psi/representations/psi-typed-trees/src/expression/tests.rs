use super::{
    BinaryExpression, BinaryOperator, Expression, ExpressionNode, ExpressionTable, NamePath,
    StructLiteral, StructLiteralField, TableBinaryExpression, TableNamePath,
};
use crate::name::Identifier;
use crate::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelections,
};
use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

fn authored_selection_occurrences() -> [AuthoredDeclarationSelectionOccurrenceId; 2] {
    let mut selections = AuthoredDeclarationSelections::default();
    let first = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::StaticPathSegment,
            SymbolHandle::from_arena_index(41),
        )
        .expect("valid selected symbol");
    let second = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(42),
        )
        .expect("valid selected symbol");
    [first, second]
}

#[test]
fn expression_occurrences_survive_typed_clone_and_symbol_remap() {
    let occurrences = authored_selection_occurrences();
    let original_symbol = SymbolHandle::from_arena_index(50);
    let remapped_symbol = SymbolHandle::from_arena_index(51);

    let mut source = ExpressionTable::new();
    let members = source.reserve_name_path_members(1);
    source.set_name_path_member_at_offset(members, 0, Identifier::generated("value"));
    let member_symbols = source.reserve_name_path_member_symbols(1);
    source.set_name_path_member_symbol_at_offset(member_symbols, 0, original_symbol);
    let expression = source.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        head_symbol: original_symbol,
        symbol: original_symbol,
    }));
    source.attach_authored_selection_occurrences(expression, occurrences);

    let mut cloned = ExpressionTable::with_capacities(source.copy_capacity());
    let cloned_expression = cloned.copy_from(&source, expression);
    cloned.remap_symbols_in(cloned_expression, &[(original_symbol, remapped_symbol)]);

    assert_eq!(
        cloned
            .authored_selection_occurrences(cloned_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    let ExpressionNode::Name(path) = cloned.expression(cloned_expression) else {
        panic!("cloned expression should remain a name path");
    };
    assert_eq!(path.head_symbol, remapped_symbol);
    assert_eq!(path.symbol, remapped_symbol);
    assert_eq!(
        cloned.name_path_member_symbols(path.member_symbols),
        [remapped_symbol]
    );

    let self_copied_expression = source.insert_copy(expression);
    source.remap_symbols_in(
        self_copied_expression,
        &[(original_symbol, remapped_symbol)],
    );
    assert_eq!(
        source
            .authored_selection_occurrences(self_copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
}

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
