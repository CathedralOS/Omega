mod routing;

use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolTable;

use self::routing::assign_statement_symbols;
use super::expressions::{assign_expression_span_symbols, assign_statement_expression_symbols};
use super::scope::MachineScope;

pub(super) fn assign_statement_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let SymbolResolvedTrees {
        roots:
            symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let data_type_parameters = &tables.declarations.data_type_parameters;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &mut tables.declarations.machine_states;
    let state_parameters = &tables.declarations.state_parameters;
    let statement_path_members = &mut tables.declarations.statement_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let state_statements = &mut tables.declarations.state_statements;
    let child_type_references = &mut tables.declarations.child_type_references;
    let type_constraints = &tables.types.constraints;
    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let machine_type_parameters = machine.type_parameters;
        let data_definition = data_definitions
            .iter()
            .find(|data_definition| data_definition.symbol == machine.attached_data_symbol);
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let symbol_resolved_trees::machine::MachineStorage {
            owned_data,
            satisfies: _,
            ranking_subjects,
            ranking_view_arguments,
            ranking_range,
            ranking_view: _,
            contracts: _,
            states,
            ..
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            type_parameters: data_type_parameters.span_or_empty(machine_type_parameters),
            attached_data: machine.attached_data.as_ref(),
            attached_data_symbol: machine.attached_data_symbol,
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
            prior_statements: &[],
            data_definitions,
            data_members,
            data_payload_fields: &tables.declarations.data_payload_fields,
            type_constraints,
        };
        // Ranking expressions belong to the entry signature, not a later
        // statement or a same-spelled parameter in another state. Resolve
        // their retained handles before proof/termination consumers use them.
        if let Some(entry) = machine_state_handles.span_or_empty(*states).first() {
            let entry = machine_states.get(*entry);
            let parameters = state_parameters.span_or_empty(entry.parameters);
            for expressions in [*ranking_subjects, *ranking_view_arguments] {
                assign_expression_span_symbols(
                    symbols,
                    &machine_scope,
                    parameters,
                    entry.symbol,
                    expression_table,
                    child_type_references,
                    expressions,
                );
            }
            assign_statement_expression_symbols(
                symbols,
                &machine_scope,
                parameters,
                entry.symbol,
                expression_table,
                child_type_references,
                *ranking_range,
            );
        }
        for state in machine_state_handles.span_or_empty(*states).iter().copied() {
            let state = machine_states.get_mut(state);
            let state_symbol = state.symbol;
            let parameters = state_parameters.span_or_empty(state.parameters);
            for type_reference in parameters
                .iter()
                .map(|parameter| &parameter.type_reference)
                .chain(state.return_type.iter())
            {
                super::type_references::assign_type_value_expression_symbols(
                    symbols,
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_reference,
                );
            }
            let statements = state_statements.span_mut_or_empty(state.statements);
            for index in 0..statements.len() {
                let (prior_statements, remaining) = statements.split_at_mut(index);
                let scope = MachineScope {
                    prior_statements,
                    ..machine_scope
                };
                assign_statement_symbols(
                    &scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    type_constraints,
                    statement_path_members,
                    &mut remaining[0],
                    symbols,
                );
            }
        }
    });
}
