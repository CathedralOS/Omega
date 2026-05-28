use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::lookup::{child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds};
use super::scope::MachineScope;
use super::scoped_paths::{
    invalid_symbol_pair, resolve_state_scoped_table_path,
    resolve_state_scoped_table_path_with_indexed_last_member,
};
use super::targets::resolve_call_target_symbol;

pub(super) fn stamp_receiver_path_symbols_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_symbols_in_table(expression_table, inner, head_symbol, symbol);
        }
        _ => {}
    }
}

fn stamp_receiver_path_head_symbol_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = head_symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_head_symbol_in_table(expression_table, inner, head_symbol);
        }
        _ => {}
    }
}

pub(super) fn resolve_expression_table_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    call: &omega_symbol_resolved_trees::expression::TableCallExpression,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if call.receiver.is_valid() {
        let receiver_symbol = resolve_expression_table_receiver_symbol(
            symbols,
            machine.symbol,
            state_symbol,
            expression_table,
            call.receiver,
        );
        return resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_symbol,
            &call.target,
            child_type_references,
            symbols,
        );
    }

    resolve_call_target_symbol(
        machine,
        parameters,
        false,
        SymbolHandle::invalid(),
        &call.target,
        child_type_references,
        symbols,
    )
}

pub(super) fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
    member: &omega_symbol_resolved_trees::name::DiagnosticName,
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
    let member_symbol = child_or_attached_data_child_symbol_by_kinds(
        symbols,
        receiver_symbol,
        &[
            SymbolKind::Field,
            SymbolKind::Object,
            SymbolKind::State,
            SymbolKind::Parameter,
            SymbolKind::Variant,
        ],
        member.as_str(),
    );

    member_symbol
}

pub(super) fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
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
                    SymbolKind::Object,
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
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            let omega_symbol_resolved_trees::expression::ExpressionNode::Integer(index) =
                expression_table.expression(indexed.index)
            else {
                return invalid_symbol_pair();
            };

            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                indexed.collection,
                *index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_indexed_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    collection: omega_symbol_resolved_trees::expression::ExpressionHandle,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(collection) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path_with_indexed_last_member(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                index,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
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
                    SymbolKind::Object,
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
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
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

fn resolve_expression_table_receiver_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (_, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            );
            symbol
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            resolve_expression_table_member_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(_) => {
            let (_, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                receiver,
            );
            symbol
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(_) => {
            SymbolHandle::invalid()
        }
        _ => SymbolHandle::invalid(),
    }
}
