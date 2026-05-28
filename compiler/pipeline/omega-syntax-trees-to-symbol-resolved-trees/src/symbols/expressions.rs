use omega_core::arena::HandleSpan;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::expression_paths::{
    resolve_expression_table_call_target_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols, stamp_receiver_path_symbols_in_table,
};
use super::scope::MachineScope;
use super::scoped_paths::resolve_state_scoped_table_path;

pub(super) fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    assign_expression_table_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        expression,
    );
}

pub(super) fn assign_expression_span_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expressions: HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let count = expressions.count();
    for offset in 0..count {
        let expression = expression_table.expression_handles(expressions)[offset as usize];
        assign_expression_table_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            expression,
        );
    }
}

pub(super) fn assign_expression_table_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                values,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.left,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.right,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
        omega_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    call.receiver,
                );
            }
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                call.receiver,
            );
            if symbol.is_valid() {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    call.receiver,
                    head_symbol,
                    symbol,
                );
            }
            let target_symbol = resolve_expression_table_call_target_symbol(
                machine,
                parameters,
                state_symbol,
                &call,
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
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.collection,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.index,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    range.end,
                );
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                member.receiver,
            );
            let member_symbol = resolve_expression_table_member_symbol(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            );
            if let (symbol, omega_symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
                (member_symbol, expression_table.expression_mut(expression))
                && symbol.is_valid()
            {
                member.member_symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                membership.value,
            );
            let name = expression_table
                .name_path_members(membership.domain)
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
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                inner,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (head_symbol, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                &path,
            );
            if symbol.is_valid() {
                if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    expression_table.expression_mut(expression)
                {
                    path.head_symbol = head_symbol;
                    path.symbol = symbol;
                }
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let count = struct_literal.fields.count();
            for offset in 0..count {
                let field = &expression_table.struct_fields(struct_literal.fields)[offset as usize];
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    field.value,
                );
            }
        }
    }
}
