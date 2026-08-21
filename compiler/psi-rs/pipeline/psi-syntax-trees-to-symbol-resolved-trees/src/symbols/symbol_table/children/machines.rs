use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};

use super::insert_machine_parameter_signature_children;
use crate::symbols::symbol_table::names::{SymbolSeed, symbol_seed};

pub(in crate::symbols::symbol_table) fn insert_machine_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    machine_symbol: SymbolHandle,
    machine: &psi_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) {
    let inherited_field_count = inherited_data_field_symbols(program, machine, has_sources).count();
    let machine_children = builder.insert_children(
        machine_symbol,
        program
            .machine_type_parameters(machine)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            })
            .chain(machine.conformance_bounds.iter().filter_map(|bound| {
                bound.binder_name.as_ref().map(|binder| {
                    symbol_seed(SymbolKind::ConformanceParameter, binder, has_sources)
                })
            }))
            .chain(inherited_data_field_symbols(program, machine, has_sources))
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

    for parameter in program.machine_type_parameters(machine) {
        let parameter_symbol = machine_children.next();
        if let (
            Some(parameter_symbol),
            psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract },
        ) = (parameter_symbol, &parameter.kind)
            && let Some(contract) = contract.structural()
        {
            insert_machine_parameter_signature_children(
                builder,
                program,
                parameter_symbol,
                contract,
                has_sources,
            );
        }
    }
    for bound in machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder_name.is_some())
    {
        let Some(binder_symbol) = machine_children.next() else {
            break;
        };
        if let Some(trait_definition) = program
            .traits
            .iter()
            .find(|trait_definition| trait_definition.name == bound.carrier_name)
        {
            let mut requirements = Vec::new();
            collect_evidence_requirement_closure(
                program,
                trait_definition,
                &mut Vec::new(),
                &mut requirements,
            );
            builder.insert_children(
                binder_symbol,
                requirements.into_iter().map(|requirement| {
                    symbol_seed(SymbolKind::State, &requirement.name, has_sources)
                }),
            );
        }
    }
    for _ in 0..inherited_field_count {
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

fn collect_evidence_requirement_closure<'program>(
    program: &'program SymbolResolvedTrees,
    trait_definition: &'program psi_symbol_resolved_trees::trait_definition::TraitDefinition,
    visited: &mut Vec<String>,
    output: &mut Vec<&'program psi_symbol_resolved_trees::signature::StateSignature>,
) {
    if visited
        .iter()
        .any(|name| name == trait_definition.name.as_str())
    {
        return;
    }
    visited.push(trait_definition.name.as_str().to_owned());
    output.extend(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    );
    for parent in program.trait_requirements(trait_definition.requires) {
        let Some(parent_trait) = program
            .traits
            .iter()
            .find(|candidate| candidate.name == parent.name)
        else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, visited, output);
    }
}

fn insert_state_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    state_symbol: SymbolHandle,
    state: &psi_symbol_resolved_trees::state::State,
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
    machine: &'program psi_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    program
        .data_definitions
        .iter()
        .find(|data_definition| Some(&data_definition.name) == machine.attached_data.as_ref())
        .into_iter()
        .flat_map(|data_definition| program.data_members(data_definition.members).iter())
        .filter_map(move |member| match member {
            psi_symbol_resolved_trees::data::DataMember::Field(field) => {
                Some(symbol_seed(SymbolKind::Field, &field.name, has_sources))
            }
            psi_symbol_resolved_trees::data::DataMember::Variant(_) => None,
        })
}

fn local_symbol_seeds<'program>(
    statements: &'program [psi_symbol_resolved_trees::statement::Statement],
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    statements
        .iter()
        .filter_map(move |statement| match statement {
            psi_symbol_resolved_trees::statement::Statement::LocalData(local_data) => Some(
                symbol_seed(SymbolKind::Local, &local_data.name, has_sources),
            ),
            _ => None,
        })
}
