use super::{
    BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
    TableStructLiteral, TableStructLiteralField,
};
use crate::name::DiagnosticName;
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
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(31),
        )
        .expect("valid selected symbol");
    let second = selections
        .record_resolved(
            SourceSpan::default(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(32),
        )
        .expect("valid selected symbol");
    [first, second]
}

#[test]
fn expression_occurrences_support_multiple_ids_and_survive_resolved_copies() {
    let occurrences = authored_selection_occurrences();
    assert_eq!(occurrences[0].ordinal(), 0);

    let mut source = ExpressionTable::new();
    let expression = source.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(7),
    ));
    source.set_authored_expression_exposure(
        expression,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    );
    source.attach_authored_selection_occurrences(expression, occurrences);

    assert_eq!(
        source
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>(),
        occurrences
    );

    let mut copied = ExpressionTable::new();
    let copied_expression = copied.copy_from(&source, expression);
    assert_eq!(
        copied
            .authored_selection_occurrences(copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    assert_eq!(
        copied.authored_expression_exposure(copied_expression),
        Some(AuthoredDeclarationSelectionExposure::PublicInterface)
    );

    let self_copied_expression = source.copy_from_self(expression);
    assert_eq!(
        source
            .authored_selection_occurrences(self_copied_expression)
            .collect::<Vec<_>>(),
        occurrences
    );
    assert_eq!(
        source.authored_expression_exposure(self_copied_expression),
        Some(AuthoredDeclarationSelectionExposure::PublicInterface)
    );
}

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
    let member_symbols = table.reserve_name_path_member_symbols(2);
    table.set_name_path_member_symbol_at_offset(
        member_symbols,
        0,
        SymbolHandle::from_arena_index(1),
    );
    table.set_name_path_member_symbol_at_offset(
        member_symbols,
        1,
        SymbolHandle::from_arena_index(2),
    );
    let root = table.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
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
    assert_eq!(
        table.name_path_member_symbols(path.member_symbols),
        [
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2)
        ]
    );
    assert_eq!(table.display_name(root), "player::inventory");
}

#[test]
fn expression_table_copies_table_payloads_without_tree_roundtrip() {
    let room_symbol = SymbolHandle::from_arena_index(3);
    let field_symbol = SymbolHandle::from_arena_index(4);

    let mut source = ExpressionTable::new();
    let room_members = source.reserve_name_path_members(1);
    source.set_name_path_member_at_offset(room_members, 0, DiagnosticName::generated("room"));
    let room_member_symbols = source.reserve_name_path_member_symbols(1);
    source.set_name_path_member_symbol_at_offset(room_member_symbols, 0, room_symbol);
    let room = source.insert(ExpressionNode::Name(TableNamePath {
        members: room_members,
        member_symbols: room_member_symbols,
        is_self_value: false,
        head_symbol: room_symbol,
        symbol: room_symbol,
    }));

    let field_members = source.reserve_name_path_members(2);
    source.set_name_path_member_at_offset(field_members, 0, DiagnosticName::generated("room"));
    source.set_name_path_member_at_offset(field_members, 1, DiagnosticName::generated("field"));
    let field_member_symbols = source.reserve_name_path_member_symbols(2);
    source.set_name_path_member_symbol_at_offset(field_member_symbols, 0, room_symbol);
    source.set_name_path_member_symbol_at_offset(field_member_symbols, 1, field_symbol);
    let field = source.insert(ExpressionNode::Name(TableNamePath {
        members: field_members,
        member_symbols: field_member_symbols,
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
            field_symbol: SymbolHandle::from_arena_index(6),
            value: hall,
        },
        TableStructLiteralField {
            name: DiagnosticName::generated("open"),
            field_symbol: SymbolHandle::from_arena_index(7),
            value: open,
        },
    ]);
    let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
        type_name: DiagnosticName::generated("Room"),
        type_symbol: SymbolHandle::from_arena_index(5),
        case_name: None,
        case_symbol: None,
        fields,
    }));

    let mut copied = ExpressionTable::new();
    let copied_root = copied.copy_from(&source, root);

    assert_eq!(source.display_name(root), copied.display_name(copied_root));

    let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root) else {
        panic!("copied root should remain a struct literal");
    };
    assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);
    assert_eq!(
        struct_literal.type_symbol,
        SymbolHandle::from_arena_index(5)
    );
    assert_eq!(
        copied.struct_fields(struct_literal.fields)[0].field_symbol,
        SymbolHandle::from_arena_index(6)
    );

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
    assert_eq!(
        copied.name_path_member_symbols(match copied.expression(binary.right) {
            ExpressionNode::Name(path) => path.member_symbols,
            _ => unreachable!(),
        }),
        [room_symbol, field_symbol]
    );
}
