use omega_core::symbols::{
    BuiltinType, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder,
    builtin_type_member_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::symbol_table::names::{SymbolSeed, operator_symbol_name, symbol_seed};

pub(super) fn insert_builtin_type_symbol_children(
    builder: &mut SymbolTableBuilder,
    builtin_symbol: SymbolHandle,
    builtin_type: (SymbolKind, SymbolNameRef<'static>),
) {
    let SymbolNameRef::Static(name) = builtin_type.1 else {
        return;
    };
    let Some(builtin_type) = BuiltinType::from_name(name) else {
        return;
    };

    builder.insert_children(builtin_symbol, builtin_type_member_symbols(builtin_type));
}

pub(super) fn insert_data_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &omega_symbol_resolved_trees::data::DataDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        data_symbol,
        program
            .data_type_parameters(data_definition.type_parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(program.data_members(data_definition.members).iter().map(
                |member| match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        symbol_seed(SymbolKind::Field, &field.name, has_sources)
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        symbol_seed(SymbolKind::Variant, &variant.name, has_sources)
                    }
                },
            )),
    );
}

pub(super) fn insert_domain_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
    domain: &omega_symbol_resolved_trees::domain::DomainDefinition,
    has_sources: bool,
) {
    let operator_names = program
        .operator_definitions(domain.operators)
        .iter()
        .map(|operator| operator_symbol_name(program, operator))
        .collect::<Vec<_>>();
    let domain_children = builder.insert_children(
        domain_symbol,
        operator_names
            .iter()
            .map(|name| (SymbolKind::Operator, SymbolNameRef::Borrowed(name.as_str()))),
    );

    for (operator_symbol, operator) in SymbolTableBuilder::child_handles(domain_children)
        .zip(program.operator_definitions(domain.operators).iter())
    {
        insert_operator_symbol_children(builder, program, operator_symbol, operator, has_sources);
    }
}

pub(super) fn insert_operator_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    operator_symbol: SymbolHandle,
    operator: &omega_symbol_resolved_trees::operator::OperatorDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        operator_symbol,
        program
            .data_type_parameters(operator.type_parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(
                program
                    .state_parameters(operator.parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            ),
    );
}

pub(super) fn insert_machine_symbol_children(
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

pub(super) fn insert_platform_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    platform_symbol: SymbolHandle,
    platform: &omega_symbol_resolved_trees::platform::Platform,
    has_sources: bool,
) {
    let platform_children = builder.insert_children(
        platform_symbol,
        program
            .platform_state_signatures(platform.states)
            .iter()
            .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
    );

    for (state_symbol, state) in SymbolTableBuilder::child_handles(platform_children)
        .zip(program.platform_state_signatures(platform.states).iter())
    {
        builder.insert_children(
            state_symbol,
            program
                .state_parameters(state.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
}

pub(super) fn insert_trait_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    trait_definition: &omega_symbol_resolved_trees::trait_definition::TraitDefinition,
    has_sources: bool,
) {
    let trait_children = builder.insert_children(
        trait_symbol,
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .map(|machine| symbol_seed(SymbolKind::State, &machine.name, has_sources)),
    );

    for (machine_symbol, machine) in SymbolTableBuilder::child_handles(trait_children).zip(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    ) {
        builder.insert_children(
            machine_symbol,
            program
                .state_parameters(machine.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
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
