use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolTable};

use super::references::{
    assign_call_symbol, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
use crate::symbols::scope::MachineScope;

pub(in crate::symbols) fn assign_static_argument_type_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    argument: &symbol_resolved_trees::expression::StaticMachineArgument,
) {
    if argument.type_reference.is_valid() {
        crate::symbols::type_references::reassign_type_reference_symbols_and_values(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            argument.type_reference,
        );
    }
    if let Some(application) = &argument.application {
        for nested in &application.arguments {
            assign_static_argument_type_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                nested,
            );
        }
    }
}

pub(in crate::symbols) fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
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
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    expressions: HandleSpan<symbol_resolved_trees::expression::ExpressionHandle>,
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
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match expression_table.expression(expression).clone() {
        symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                dispatch.subject,
            );
            for offset in 0..dispatch.arms.count() {
                let arm = expression_table.match_arms(dispatch.arms)[offset as usize];
                if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                {
                    assign_expression_table_symbols(
                        symbols,
                        machine,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        pattern,
                    );
                }
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    arm.value,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            // The instruction-observed prior has its own destination expression,
            // distinct from references inside the arithmetic-shaped carrier.
            // Resolve it in the same state scope so downstream consumers bind
            // the declared local by identity, never by retained spelling.
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                atomic.result,
            );
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
        symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | symbol_resolved_trees::expression::ExpressionNode::UndeterminedNaN(_)
        | symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
        symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
            crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.target_type,
            );
            crate::symbols::type_references::each_type_reference_handle(
                cast.semantic_domain_arguments,
                |handle| {
                    crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                        symbols,
                        machine,
                        parameters,
                        state_symbol,
                        expression_table,
                        child_type_references,
                        handle,
                    );
                },
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            for argument in &call.machine_arguments {
                assign_static_argument_type_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    argument,
                );
            }
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
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
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
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            assign_name_symbol(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                &path,
                expression,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
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
        symbol_resolved_trees::expression::ExpressionNode::ZeroValue(type_reference) => {
            crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                type_reference,
            );
        }
    }
}
