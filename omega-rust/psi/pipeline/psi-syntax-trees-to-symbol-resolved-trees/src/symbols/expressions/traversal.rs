use psi_arena::HandleSpan;
use psi_symbols::{SymbolHandle, SymbolTable};

use super::references::{
    assign_call_symbol, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
use crate::symbols::scope::MachineScope;

pub(in crate::symbols) fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
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

pub(in crate::symbols) fn assign_expression_span_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    expressions: HandleSpan<psi_symbol_resolved_trees::expression::ExpressionHandle>,
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

pub(in crate::symbols) fn assign_expression_table_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match expression_table.expression(expression).clone() {
        psi_symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                atomic.value,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
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
        psi_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
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
        psi_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | psi_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
        psi_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
            let mut target_type = child_type_references.get(cast.target_type).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                machine.type_parameters,
                machine.symbol,
                &mut target_type,
            );
            *child_type_references.get_mut(cast.target_type) = target_type;
            let target_type = child_type_references.get(cast.target_type).clone();
            crate::symbols::type_references::assign_type_value_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                &target_type,
            );
            for offset in 0..cast.semantic_domain_arguments.count() {
                let start = cast.semantic_domain_arguments.start();
                let handle =
                    psi_arena::Handle::from_parts(start.arena_index() + offset, start.generation());
                let mut argument = child_type_references.get(handle).clone();
                crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                    symbols,
                    child_type_references,
                    machine.type_parameters,
                    machine.symbol,
                    &mut argument,
                );
                *child_type_references.get_mut(handle) = argument;
            }
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
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
            assign_call_symbol(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.receiver,
                &call,
                expression,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
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
        psi_symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
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
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let receiver = member.receiver;
            let member_name = member.member.clone();
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                receiver,
            );
            assign_member_symbol(
                symbols,
                machine,
                state_symbol,
                expression_table,
                receiver,
                &member_name,
                expression,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                membership.value,
            );
            assign_membership_symbol(symbols, expression_table, membership.domain, expression);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                inner.target,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                unary.operand,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            assign_name_symbol(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                &path,
                expression,
            );
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
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
            assign_struct_literal_symbols(symbols, expression_table, expression);
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::ZeroValue(type_reference) => {
            let mut target_type = child_type_references.get(type_reference).clone();
            crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type(
                symbols,
                child_type_references,
                machine.type_parameters,
                machine.symbol,
                &mut target_type,
            );
            *child_type_references.get_mut(type_reference) = target_type;
            let target_type = child_type_references.get(type_reference).clone();
            crate::symbols::type_references::assign_type_value_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                &target_type,
            );
        }
    }
}
