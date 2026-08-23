use super::{
    BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
    TableStructLiteral, TableStructLiteralField,
};
use crate::name::DiagnosticName;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn expression_table_stores_nested_expressions_as_handles() {
    let mut table = ExpressionTable::new();
    let one = table.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(1),
    ));
    let two = table.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(2),
    ));
    let three = table.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(3),
    ));
    let right = table.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: two,
        operator: BinaryOperator::Add,
        right: three,
    }));
    let root = table.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: one,
        operator: BinaryOperator::Add,
        right,
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
fn expression_table_stores_name_paths_as_member_spans() {
    let mut table = ExpressionTable::new();
    let members = table.reserve_name_path_members(2);
    table.set_name_path_member_at_offset(members, 0, DiagnosticName::generated("player"));
    table.set_name_path_member_at_offset(members, 1, DiagnosticName::generated("inventory"));
    let root = table.insert(ExpressionNode::Name(TableNamePath {
        members,
        is_self_value: false,
        head_symbol: SymbolHandle::from_arena_index(1),
        symbol: SymbolHandle::from_arena_index(2),
    }));
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

    let mut source = ExpressionTable::new();
    let room_members = source.reserve_name_path_members(1);
    source.set_name_path_member_at_offset(room_members, 0, DiagnosticName::generated("room"));
    let room = source.insert(ExpressionNode::Name(TableNamePath {
        members: room_members,
        is_self_value: false,
        head_symbol: room_symbol,
        symbol: room_symbol,
    }));

    let field_members = source.reserve_name_path_members(2);
    source.set_name_path_member_at_offset(field_members, 0, DiagnosticName::generated("room"));
    source.set_name_path_member_at_offset(field_members, 1, DiagnosticName::generated("field"));
    let field = source.insert(ExpressionNode::Name(TableNamePath {
        members: field_members,
        is_self_value: false,
        head_symbol: room_symbol,
        symbol: field_symbol,
    }));

    let hall = source.insert(ExpressionNode::String(Arc::from(&b"Hall"[..])));
    let open = source.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: room,
        operator: BinaryOperator::Equal,
        right: field,
    }));
    let fields = source.insert_struct_fields([
        TableStructLiteralField {
            name: DiagnosticName::generated("name"),
            value: hall,
        },
        TableStructLiteralField {
            name: DiagnosticName::generated("open"),
            value: open,
        },
    ]);
    let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: DiagnosticName::generated("Room"),
        case_name: None,
        fields,
    }));

    let mut copied = ExpressionTable::new();
    let copied_root = copied.copy_from(&source, root);

    assert_eq!(source.display_name(root), copied.display_name(copied_root));

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
