mod routing;

use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::SymbolTable;

use self::routing::assign_statement_symbols;
use super::scope::MachineScope;

pub(super) fn assign_statement_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let SymbolResolvedTrees {
        roots:
            psi_symbol_resolved_trees::SymbolResolvedRoots {
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
        let data_definition = machine.attached_data.as_ref().and_then(|attached_data| {
            data_definitions
                .iter()
                .find(|data_definition| data_definition.name == *attached_data)
        });
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let psi_symbol_resolved_trees::machine::MachineStorage {
            owned_data,
            satisfies: _,
            ranking_subjects: _,
            ranking_view: _,
            contracts: _,
            states,
            ..
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            type_parameters: data_type_parameters.span_or_empty(machine_type_parameters),
            attached_data: machine.attached_data.as_ref(),
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
            data_definitions,
            data_members,
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
                    type_constraints,
                    statement_path_members,
                    statement,
                    symbols,
                );
            }
        }
    });
}
