use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
};
use super::super::scoped_paths::{
    invalid_symbol_pair, resolve_state_scoped_table_path,
    resolve_state_scoped_table_path_with_indexed_last_member,
};

/// The member kinds a receiver path may resolve through — fields, states,
/// parameters and variants.
const RECEIVER_MEMBER_KINDS: &[SymbolKind] = &[
    SymbolKind::Field,
    SymbolKind::State,
    SymbolKind::Parameter,
    SymbolKind::Variant,
];

/// A structural head selects its common fields and this case's payload.
/// Sibling payload namespaces never participate, and duplicate spellings do
/// not become declaration-order selection.
pub(in crate::symbols) fn pattern_field_symbol(
    symbols: &SymbolTable,
    head: SymbolHandle,
    spelling: &str,
) -> SymbolHandle {
    if !head.is_valid() {
        return SymbolHandle::invalid();
    }
    let parent = if symbols.get(head).kind == SymbolKind::Variant {
        symbols.get(head).parent
    } else {
        SymbolHandle::invalid()
    };
    let mut fields = [head, parent]
        .into_iter()
        .filter(|owner| owner.is_valid())
        .flat_map(|owner| symbols.child_handles(owner).into_iter().flatten())
        .filter(|field| {
            symbols.get(*field).kind == SymbolKind::Field && symbols.name(*field) == spelling
        });
    let Some(field) = fields.next() else {
        return SymbolHandle::invalid();
    };
    if fields.next().is_some() {
        SymbolHandle::invalid()
    } else {
        field
    }
}

pub(in crate::symbols) fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
    member: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    if let symbol_resolved_trees::expression::ExpressionNode::SavedSubject(saved) =
        expression_table.expression(receiver)
    {
        let head = expression_table
            .name_path_node(saved.pattern_head)
            .map(|path| path.symbol)
            .unwrap_or_default();
        return pattern_field_symbol(symbols, head, member.as_str());
    }
    let receiver_symbol = resolve_expression_table_receiver_symbol(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
    );
    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    child_or_attached_data_child_symbol_by_kinds(
        symbols,
        receiver_symbol,
        RECEIVER_MEMBER_KINDS,
        member.as_str(),
    )
}

pub(in crate::symbols) fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(receiver) {
        symbol_resolved_trees::expression::ExpressionNode::SavedSubject(saved) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                saved.pattern_head,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = if matches!(
                expression_table.expression(member.receiver),
                symbol_resolved_trees::expression::ExpressionNode::SavedSubject(_)
            ) {
                pattern_field_symbol(symbols, receiver_symbol, member.member.as_str())
            } else {
                child_or_attached_data_child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    RECEIVER_MEMBER_KINDS,
                    member.member.as_str(),
                )
            };

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                inner.target,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            let symbol_resolved_trees::expression::ExpressionNode::Integer(index) =
                expression_table.expression(indexed.index)
            else {
                return invalid_symbol_pair();
            };
            // An index beyond i64 cannot name a real element.
            let Some(index) = index.value_i64() else {
                return invalid_symbol_pair();
            };

            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                indexed.collection,
                index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_indexed_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    collection: symbol_resolved_trees::expression::ExpressionHandle,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(collection) {
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path_with_indexed_last_member(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                index,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = child_indexed_symbol_by_kinds(
                symbols,
                receiver_symbol,
                RECEIVER_MEMBER_KINDS,
                member.member.as_str(),
                index,
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                inner.target,
                index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

pub(in crate::symbols::expression_paths) fn resolve_expression_table_receiver_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    match expression_table.expression(receiver) {
        symbol_resolved_trees::expression::ExpressionNode::SavedSubject(saved) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                saved.pattern_head,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (_, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            );
            symbol
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            resolve_expression_table_member_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                inner.target,
            )
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(_) => {
            let (_, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                receiver,
            );
            symbol
        }
        symbol_resolved_trees::expression::ExpressionNode::Range(_) => SymbolHandle::invalid(),
        _ => SymbolHandle::invalid(),
    }
}
