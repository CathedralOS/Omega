use omega_core::symbols::{
    SymbolDefinition, SymbolHandle, SymbolKind, SymbolTable, builtin_type_symbol_definitions,
};
use omega_resolved_trees::Program;

pub(crate) fn assign_symbols(program: &mut Program) {
    program.symbols = build_symbol_table(program);
    let symbols = program.symbols.clone();
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
}

fn build_symbol_table(program: &Program) -> SymbolTable {
    let mut children = builtin_type_symbol_definitions().to_vec();

    children.extend(
        program
            .invariant_definitions
            .iter()
            .map(|invariant| SymbolDefinition::named(SymbolKind::Invariant, invariant.name.as_str())),
    );
    children.extend(program.data_definitions.iter().map(data_symbol_definition));
    children.extend(program.machines.iter().map(|machine| machine_symbol_definition(program, machine)));
    children.extend(program.platforms.iter().map(platform_symbol_definition));

    SymbolTable::from_definition(SymbolDefinition::with_children(SymbolKind::Root, "root", children))
}

fn data_symbol_definition<'program>(
    data_definition: &'program omega_resolved_trees::data::DataDefinition,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::Data,
        data_definition.name.as_str(),
        data_definition.members.iter().map(|member| match member {
            omega_resolved_trees::data::DataMember::Field(field) => {
                SymbolDefinition::named(SymbolKind::Field, field.name.as_str())
            }
            omega_resolved_trees::data::DataMember::Variant(variant) => {
                SymbolDefinition::named(SymbolKind::Variant, variant.name.as_str())
            }
        }),
    )
}

fn machine_symbol_definition<'program>(
    program: &'program Program,
    machine: &'program omega_resolved_trees::machine::Machine,
) -> SymbolDefinition<'program> {
    let inherited_data_members = program
        .data_definitions
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
        .into_iter()
        .flat_map(|data_definition| data_definition.members.iter())
        .filter_map(|member| match member {
            omega_resolved_trees::data::DataMember::Field(field) => {
                Some(SymbolDefinition::named(SymbolKind::Field, field.name.as_str()))
            }
            omega_resolved_trees::data::DataMember::Variant(_) => None,
        });
    let contained_objects = machine
        .contains
        .iter()
        .map(|contained_object| SymbolDefinition::named(SymbolKind::Object, contained_object.name.as_str()));
    let owned_data = machine
        .owned_data
        .iter()
        .map(|owned_data| SymbolDefinition::named(SymbolKind::Field, owned_data.name.as_str()));
    let states = machine.states.iter().map(state_symbol_definition);

    SymbolDefinition::with_children(
        SymbolKind::Machine,
        machine.name.as_str(),
        inherited_data_members
            .chain(contained_objects)
            .chain(owned_data)
            .chain(states),
    )
}

fn state_symbol_definition<'program>(
    state: &'program omega_resolved_trees::state::State,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::State,
        state.name.as_str(),
        state
            .parameters
            .iter()
            .map(|parameter| SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str())),
    )
}

fn platform_symbol_definition<'program>(
    platform: &'program omega_resolved_trees::platform::Platform,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::Platform,
        platform.name.as_str(),
        platform.states.iter().map(|state| {
            SymbolDefinition::with_children(
                SymbolKind::State,
                state.name.as_str(),
                state
                    .parameters
                    .iter()
                    .map(|parameter| {
                        SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str())
                    }),
            )
        }),
    )
}

fn assign_top_level_symbols(program: &mut Program, symbols: &SymbolTable) {
    for invariant in &mut program.invariant_definitions {
        invariant.symbol = top_level_symbol(symbols, invariant.name.as_str());
    }

    for data_definition in &mut program.data_definitions {
        data_definition.symbol = top_level_symbol(symbols, data_definition.name.as_str());

        for member in &mut data_definition.members {
            match member {
                omega_resolved_trees::data::DataMember::Field(field) => {
                    field.symbol = child_symbol(symbols, data_definition.symbol, field.name.as_str());
                }
                omega_resolved_trees::data::DataMember::Variant(variant) => {
                    variant.symbol =
                        child_symbol(symbols, data_definition.symbol, variant.name.as_str());
                }
            }
        }
    }

    for machine in &mut program.machines {
        machine.symbol = top_level_symbol(symbols, machine.name.as_str());

        for contained_object in &mut machine.contains {
            contained_object.symbol = child_symbol(symbols, machine.symbol, contained_object.name.as_str());
            contained_object.type_symbol = top_level_symbol(symbols, contained_object.type_name.as_str());
        }

        for owned_data in &mut machine.owned_data {
            owned_data.symbol = child_symbol(symbols, machine.symbol, owned_data.name.as_str());
            assign_type_reference_symbol(symbols, &mut owned_data.type_reference);
        }

        for state in &mut machine.states {
            state.symbol = child_symbol(symbols, machine.symbol, state.name.as_str());

            for parameter in &mut state.parameters {
                parameter.symbol = child_symbol(symbols, state.symbol, parameter.name.as_str());
                assign_type_reference_symbol(symbols, &mut parameter.type_reference);
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol(symbols, return_type);
            }
        }
    }

    for platform in &mut program.platforms {
        platform.symbol = top_level_symbol(symbols, platform.name.as_str());

        for state in &mut platform.states {
            state.symbol = child_symbol(symbols, platform.symbol, state.name.as_str());

            for parameter in &mut state.parameters {
                parameter.symbol = child_symbol(symbols, state.symbol, parameter.name.as_str());
                assign_type_reference_symbol(symbols, &mut parameter.type_reference);
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol(symbols, return_type);
            }
        }
    }
}

fn assign_type_reference_symbols(
    program: &mut Program,
    symbols: &SymbolTable,
) {
    for data_definition in &mut program.data_definitions {
        for member in &mut data_definition.members {
            if let omega_resolved_trees::data::DataMember::Field(field) = member {
                assign_type_reference_symbol(symbols, &mut field.type_reference);
            }
        }
    }
}

fn assign_type_reference_symbol(
    symbols: &SymbolTable,
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Constrained { base_type, .. } => {
            assign_type_reference_symbol(symbols, base_type);
        }
        omega_resolved_trees::types::TypeReference::FixedArray { element_type, .. } => {
            assign_type_reference_symbol(symbols, element_type);
        }
        omega_resolved_trees::types::TypeReference::Slice { element_type } => {
            assign_type_reference_symbol(symbols, element_type);
        }
        omega_resolved_trees::types::TypeReference::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            *base_symbol = top_level_symbol(symbols, base_name.as_str());

            for argument in arguments {
                assign_type_reference_symbol(symbols, argument);
            }
        }
        omega_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = top_level_symbol(symbols, name.as_str());
        }
        omega_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn top_level_symbol(symbols: &SymbolTable, name: &str) -> SymbolHandle {
    child_symbol(symbols, symbols.root(), name)
}

fn child_symbol(symbols: &SymbolTable, parent: SymbolHandle, name: &str) -> SymbolHandle {
    symbols
        .find_child_by_name(parent, name)
        .unwrap_or_else(SymbolHandle::invalid)
}
