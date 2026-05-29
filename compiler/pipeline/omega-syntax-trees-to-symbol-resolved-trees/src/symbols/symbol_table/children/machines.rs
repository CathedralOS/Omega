use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::symbol_table::names::{SymbolSeed, symbol_seed};

pub(in crate::symbols::symbol_table) fn insert_machine_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    machine_symbol: SymbolHandle,
    machine: &omega_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) {
    let inherited_field_count = inherited_data_field_symbols(program, machine, has_sources).count();
    let machine_children = builder.insert_children(
        machine_symbol,
        inherited_data_field_symbols(program, machine, has_sources)
            .chain(
                program
                    .machine_contained_objects(machine.contains)
                    .iter()
                    .map(|contained_object| {
                        symbol_seed(SymbolKind::Object, &contained_object.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_owned_data(machine.owned_data)
                    .iter()
                    .map(|owned_data| {
                        symbol_seed(SymbolKind::Field, &owned_data.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|state| program.machine_state(*state))
                    .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
            ),
    );
    let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

    for _ in 0..inherited_field_count {
        let _ = machine_children.next();
    }
    for _ in program.machine_contained_objects(machine.contains) {
        let _ = machine_children.next();
    }
    for _ in program.machine_owned_data(machine.owned_data) {
        let _ = machine_children.next();
    }
    for state in program.machine_state_handles(machine.states) {
        if let Some(state_symbol) = machine_children.next() {
            let state = program.machine_state(*state);
            insert_state_symbol_children(builder, program, state_symbol, state, has_sources);
        }
    }
}

fn insert_state_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    state_symbol: SymbolHandle,
    state: &omega_symbol_resolved_trees::state::State,
    has_sources: bool,
) {
    builder.insert_children(
        state_symbol,
        program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources))
            .chain(local_symbol_seeds(
                program.state_statements(state.statements),
                has_sources,
            )),
    );
}

fn inherited_data_field_symbols<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &'program omega_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    program
        .data_definitions
        .iter()
        .find(|data_definition| Some(&data_definition.name) == machine.attached_data.as_ref())
        .into_iter()
        .flat_map(|data_definition| program.data_members(data_definition.members).iter())
        .filter_map(move |member| match member {
            omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                Some(symbol_seed(SymbolKind::Field, &field.name, has_sources))
            }
            omega_symbol_resolved_trees::data::DataMember::Variant(_) => None,
        })
}

fn local_symbol_seeds<'program>(
    statements: &'program [omega_symbol_resolved_trees::statement::Statement],
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    statements
        .iter()
        .filter_map(move |statement| match statement {
            omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) => Some(
                symbol_seed(SymbolKind::Local, &local_data.name, has_sources),
            ),
            _ => None,
        })
}
