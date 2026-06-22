use omega_core::arena::Arena;
use omega_core::symbols::{SymbolHandle, SymbolTable};

use crate::symbols::expressions::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
};
use crate::symbols::scope::MachineScope;
use crate::symbols::scoped_paths::resolve_state_scoped_members;
use crate::symbols::targets::{assign_transition_target_symbols, resolve_call_target_symbol};
use crate::symbols::type_references::assign_type_reference_symbol_with_self_type;

pub(super) fn assign_statement_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &mut Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    statement: &mut omega_symbol_resolved_trees::statement::Statement,
    symbols: &SymbolTable,
) {
    match statement {
        omega_symbol_resolved_trees::statement::Statement::Assignment(assignment) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.target,
            );
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.value,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Call(call) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            if !call.receiver.is_empty() {
                let (head_symbol, symbol) = resolve_state_scoped_members(
                    symbols,
                    machine.symbol,
                    state_symbol,
                    statement_path_members.span_or_empty(call.receiver),
                    call.receiver_starts_at_self,
                );
                if symbol.is_valid() {
                    let _ = head_symbol;
                    call.receiver_symbol = symbol;
                }
            }

            call.target_symbol = resolve_call_target_symbol(
                machine,
                parameters,
                !call.receiver.is_empty(),
                call.receiver_symbol,
                &call.target,
                child_type_references,
                symbols,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Expression(expression) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol_with_self_type(
                symbols,
                child_type_references,
                machine.symbol,
                &mut local_data.type_reference,
            );
            if local_data.initial_value.is_valid() {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    local_data.initial_value,
                );
            }
        }
        omega_symbol_resolved_trees::statement::Statement::Transition(transition) => {
            if let omega_symbol_resolved_trees::statement::TransitionGuard::When(expression) =
                &mut transition.guard
            {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    *expression,
                );
            }
            assign_transition_target_symbols(
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                statement_path_members,
                &mut transition.target,
                symbols,
            );
            if let Some(continuation) = &mut transition.continuation {
                assign_transition_target_symbols(
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    continuation,
                    symbols,
                );
            }
        }
    }
}
