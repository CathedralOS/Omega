use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::expression_paths::{
    resolve_expression_table_call_target_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols, stamp_receiver_path_symbols_in_table,
};
use super::super::scope::MachineScope;
use super::super::scoped_paths::resolve_state_scoped_table_path;

pub(super) fn assign_call_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
    call: &omega_symbol_resolved_trees::expression::TableCallExpression,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
        symbols,
        machine.symbol,
        state_symbol,
        expression_table,
        receiver,
    );
    if symbol.is_valid() {
        stamp_receiver_path_symbols_in_table(expression_table, receiver, head_symbol, symbol);
    }

    let target_symbol = resolve_expression_table_call_target_symbol(
        machine,
        parameters,
        state_symbol,
        call,
        expression_table,
        child_type_references,
        symbols,
    );
    if let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expression_table.expression_mut(expression)
    {
        call.target_symbol = target_symbol;
    }
}

pub(super) fn assign_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
    member_name: &omega_symbol_resolved_trees::name::DiagnosticName,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let member_symbol = resolve_expression_table_member_symbol(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
        member_name,
    );
    if let (symbol, omega_symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
        (member_symbol, expression_table.expression_mut(expression))
        && symbol.is_valid()
    {
        member.member_symbol = symbol;
    }
}

pub(super) fn assign_membership_symbol(
    symbols: &SymbolTable,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    domain: omega_core::arena::HandleSpan<omega_symbol_resolved_trees::name::DiagnosticName>,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let name = expression_table
        .name_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    if let omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
        expression_table.expression_mut(expression)
    {
        membership.domain_symbol = symbols
            .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
            .unwrap_or_else(SymbolHandle::invalid);
    }
}

pub(super) fn assign_name_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    path: &omega_symbol_resolved_trees::expression::TableNamePath,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let (head_symbol, symbol) = resolve_state_scoped_table_path(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        path,
    );
    if symbol.is_valid()
        && let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            expression_table.expression_mut(expression)
    {
        path.head_symbol = head_symbol;
        path.symbol = symbol;
    }
}
