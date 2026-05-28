use omega_core::arena::Arena;
use omega_core::symbols::{SymbolHandle, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use super::scope::MachineScope;
use super::scoped_paths::resolve_state_scoped_members;
use super::type_references::assign_type_reference_symbol_with_self_type;
use super::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
    assign_transition_target_symbols, resolve_call_target_symbol,
};

pub(super) fn assign_statement_call_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let SymbolResolvedTrees {
        roots:
            omega_symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let machine_contained_objects = &tables.declarations.machine_contained_objects;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &mut tables.declarations.machine_states;
    let state_parameters = &tables.declarations.state_parameters;
    let statement_path_members = &tables.declarations.statement_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let state_statements = &mut tables.declarations.state_statements;
    let child_type_references = &mut tables.declarations.child_type_references;
    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let data_definition = machine.attached_data.as_ref().and_then(|attached_data| {
            data_definitions
                .iter()
                .find(|data_definition| data_definition.name == *attached_data)
        });
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let omega_symbol_resolved_trees::machine::MachineStorage {
            contains,
            owned_data,
            satisfies: _,
            terminates: _,
            decreases: _,
            decrease_order: _,
            effects: _,
            contracts: _,
            states,
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            attached_data: machine.attached_data.as_ref(),
            contains: machine_contained_objects.span_or_empty(*contains),
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
        };
        for state in machine_state_handles.span_or_empty(*states).iter().copied() {
            let state = machine_states.get_mut(state);
            let state_symbol = state.symbol;
            let parameters = state_parameters.span_or_empty(state.parameters);
            for statement in state_statements.span_mut_or_empty(state.statements) {
                assign_statement_symbols(
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    statement,
                    symbols,
                );
            }
        }
    });
}

fn assign_statement_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
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
