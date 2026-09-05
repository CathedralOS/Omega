use psi_arena::Arena;
use psi_symbols::{SymbolHandle, SymbolTable};

use crate::symbols::expressions::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
};
use crate::symbols::scope::MachineScope;
use crate::symbols::scoped_paths::resolve_state_scoped_members;
use crate::symbols::targets::{
    assign_provider_selection_argument_symbol, assign_representation_selection_argument_symbol,
    assign_static_argument_symbols, assign_transition_target_symbols, resolve_call_target_symbol,
};
use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type_and_constraints;

pub(super) fn assign_statement_symbols(
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &psi_arena::Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    statement_path_members: &mut Arena<psi_symbol_resolved_trees::name::DiagnosticName>,
    statement: &mut psi_symbol_resolved_trees::statement::Statement,
    symbols: &SymbolTable,
) {
    match statement {
        psi_symbol_resolved_trees::statement::Statement::AssemblyFact(fact) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                fact.expression,
            );
        }
        psi_symbol_resolved_trees::statement::Statement::Assignment(assignment) => {
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
        psi_symbol_resolved_trees::statement::Statement::Call(call) => {
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
                let members = statement_path_members.span_or_empty(call.receiver);
                // A projected member may await type-aware resolution. Its
                // lexical root is already exact and must survive independently.
                let (root, _) = resolve_state_scoped_members(
                    symbols,
                    machine.symbol,
                    state_symbol,
                    members.get(..1).unwrap_or_default(),
                    call.receiver_starts_at_self,
                );
                call.receiver_root_symbol = root;
                let (_, symbol) = resolve_state_scoped_members(
                    symbols,
                    machine.symbol,
                    state_symbol,
                    members,
                    call.receiver_starts_at_self,
                );
                if symbol.is_valid() {
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
            let provider_selection = call.target.as_str() == "select_provider";
            let representation_selection = call.target.as_str() == "select_representation";
            for (index, argument) in call.machine_arguments.iter_mut().enumerate() {
                if provider_selection {
                    assign_provider_selection_argument_symbol(symbols, argument, index == 0);
                } else if representation_selection {
                    assign_representation_selection_argument_symbol(symbols, argument, index == 0);
                } else {
                    assign_static_argument_symbols(symbols, machine.symbol, argument, false);
                }
            }
        }
        psi_symbol_resolved_trees::statement::Statement::ProofOutputBindingStatement(binding) => {
            binding.machine_symbol = machine.symbol;
            binding.state_symbol = state_symbol;
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binding.call,
            );
        }
        psi_symbol_resolved_trees::statement::Statement::Expression(expression) => {
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
        psi_symbol_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                machine.type_parameters,
                machine.symbol,
                &mut local_data.type_reference,
            );
            crate::symbols::type_references::assign_type_value_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                &local_data.type_reference,
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
        psi_symbol_resolved_trees::statement::Statement::Transition(transition) => {
            if let psi_symbol_resolved_trees::statement::TransitionGuard::When(expression) =
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
