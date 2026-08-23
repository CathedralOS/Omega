use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::{
    child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
};
use super::super::scoped_paths::{
    invalid_symbol_pair, resolve_state_scoped_table_path,
    resolve_state_scoped_table_path_with_indexed_last_member,
};

pub(in crate::symbols) fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
    member: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
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
        &[
            SymbolKind::Field,
            SymbolKind::State,
            SymbolKind::Parameter,
            SymbolKind::Variant,
        ],
        member.as_str(),
    )
}

pub(in crate::symbols) fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(receiver) {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            )
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
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
            let member_symbol = child_or_attached_data_child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            let psi_symbol_resolved_trees::expression::ExpressionNode::Integer(index) =
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
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    collection: psi_symbol_resolved_trees::expression::ExpressionHandle,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(collection) {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path_with_indexed_last_member(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                index,
            )
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
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
                &[
                    SymbolKind::Field,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
                index,
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
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
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    match expression_table.expression(receiver) {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (_, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            );
            symbol
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            resolve_expression_table_member_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            )
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(_) => {
            let (_, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                receiver,
            );
            symbol
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Range(_) => SymbolHandle::invalid(),
        _ => SymbolHandle::invalid(),
    }
}
